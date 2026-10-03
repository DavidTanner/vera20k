//! Ground move orders: the path-search inputs an order's move reads.
//!
//! Command orders, pursuing and resumed orders, miners, ejected passengers
//! and the factory rally give a ground mover its destination through
//! [`Simulation::issue_ground_move`]: the blocker plane and, where the site
//! asks for them, the mover's owner block sets, then
//! `issue_move_command_with_destination`. Both come from the products the
//! movement pass keeps current through the entity touch log
//! (`MovementPassCache`). Each equals a whole-world build from the entities,
//! terrain, overlays, alliances and rules at this point of the frame; debug
//! builds compare the two on every read. Scatter, air and other direct moves
//! do not come through here. A mover whose active locomotor is Teleport takes
//! its class setter instead of a route: no pass moves a Teleport owner. A
//! Jumpjet's cell order likewise takes Foot's setter, whose `Move_To` flies
//! it; an object order still takes the route, and the Jumpjet's `Process`
//! hands its goal to `Move_To` (`apply_jumpjet_adapter_order`).
//!
//! Residual, carried over unchanged: the sites differ in what they hand the
//! search, with no recorded native reason. The resumed-order, both miner and
//! the factory-rally moves search without owner block sets; the resumed-order
//! and idle-miner moves also search without terrain costs. Only a mover that
//! searches when the order is given reads these inputs: not Drive, Ship or a
//! Walk mover taking a fresh destination, which accept first and search in
//! their own turn. Aligning the sites changes paths and needs native evidence
//! per site.

use super::Simulation;
use crate::map::entities::EntityCategory;
use crate::rules::locomotor_type::{LocomotorKind, SpeedType};
use crate::rules::ruleset::RuleSet;
use crate::sim::components::{DriveCoord, NavTargetRef};
use crate::sim::movement::{self, DestinationTiming};
use crate::util::fixed_math::SimFixed;

/// One ground move order.
pub(crate) struct GroundMove {
    pub(crate) entity_id: u64,
    pub(crate) target: (u16, u16),
    pub(crate) speed: SimFixed,
    pub(crate) queue: bool,
    /// Terrain costs for this speed type; `None` searches without them.
    pub(crate) speed_type: Option<SpeedType>,
    /// Whether the search reads the block sets as the mover's owner sees
    /// them (friendly movers passable); otherwise it reads none.
    pub(crate) owner_blocks: bool,
    /// An object order's captured coordinate (see
    /// `issue_move_command_with_destination`).
    pub(crate) object_destination: Option<(NavTargetRef, DriveCoord)>,
}

impl Simulation {
    /// Issue `order` with the kept block sets and blocker plane, against the
    /// canonical path grid. Returns whether the mover accepted the
    /// destination; without a published grid nothing is issued. A mover on
    /// Teleport takes its class setter ([`Self::teleport_destination`]), and
    /// a Jumpjet's cell order takes Foot's
    /// ([`Self::jumpjet_cell_destination`]), which ignores `queue`.
    pub(crate) fn issue_ground_move(&mut self, order: GroundMove, rules: Option<&RuleSet>) -> bool {
        if let Some(accepted) = self.teleport_destination(order.entity_id, order.target, rules) {
            return accepted;
        }
        if order.object_destination.is_none()
            && let Some(accepted) =
                self.jumpjet_cell_destination(order.entity_id, order.target, order.speed, rules)
        {
            return accepted;
        }
        let Some(grid) = self.path_grid_snapshot() else {
            return false;
        };
        let grid = grid.as_ref();
        let block_owner = order
            .owner_blocks
            .then(|| self.substrate.entities.get(order.entity_id))
            .flatten()
            .map(|entity| entity.owner());
        let lent = block_owner.map(|owner| {
            let sets = self.movement_pass_cache.lend_block_set(
                owner,
                &mut self.substrate.entities,
                &self.house_alliances,
                &self.interner,
                rules,
            );
            (owner, sets)
        });
        let blocker_neighbor_counts = self.movement_pass_cache.blocker_plane(
            &mut self.substrate.entities,
            grid,
            self.resolved_terrain.as_ref(),
            self.overlay_grid.as_ref(),
            &self.interner,
            rules,
        );
        let issued = movement::issue_move_command_with_destination(
            &mut self.substrate.entities,
            grid,
            order.entity_id,
            order.target,
            order.speed,
            order.queue,
            order
                .speed_type
                .and_then(|speed_type| self.terrain_costs.get(&speed_type)),
            lent.as_ref().map(|(_, lent)| &lent.sets.0),
            self.resolved_terrain.as_ref(),
            self.zone_grid.as_ref(),
            lent.as_ref().map(|(_, lent)| &lent.sets.1),
            Some(blocker_neighbor_counts),
            self.playfield_bounds,
            Some(&mut self.substrate.cell_occupation),
            order.object_destination,
            DestinationTiming::from_rules(self.session.binary_frame, rules),
        );
        if let Some((owner, lent)) = lent {
            self.movement_pass_cache.give_back(owner, lent);
        }
        issued
    }

    /// `vt+0x480(cell, 1)` for a mover whose active locomotor is Teleport: its
    /// class setter, whose Foot tail reaches Teleport Move_To (`0x00718100`).
    /// The Teleport Process warps; it follows no route.
    /// - Infantry: the Infantry setter (`0x0051AA40`), which never reads
    ///   `Teleporter=`.
    /// - A `Teleporter=` Unit (the Chrono Miner): the Unit setter
    ///   (`0x00741970`), whose Teleporter arm drives it except onto a dock.
    /// - Another Unit: Teleport Move_To. The retail ones are CMON and SMON,
    ///   `UnloadingClass=` harvesters.
    ///
    /// RESIDUALS:
    /// - The last arm skips the rest of the Unit setter, which does not
    ///   represent a Unit on Teleport without `Teleporter=`: its same-NavCom
    ///   return and NavCom write. Trigger: an order to a Chrono Miner while it
    ///   unloads. Effect: a repeated order re-arms the warp, and NavCom keeps
    ///   the previous order. Frequency: rare.
    /// - The order's queue flag and object destination are not carried: a
    ///   queued waypoint replaces the order, and an object order warps to the
    ///   object's cell. Trigger: a queued or object order to a Chrono unit.
    ///   Frequency: uncommon.
    pub(crate) fn teleport_destination(
        &mut self,
        id: u64,
        cell: (u16, u16),
        rules: Option<&RuleSet>,
    ) -> Option<bool> {
        let entity = self.substrate.entities.get(id)?;
        if entity.locomotor.as_ref()?.active_kind() != LocomotorKind::Teleport {
            return None;
        }
        let category = entity.category;
        let Some(rules) = rules else {
            return Some(false);
        };
        Some(match category {
            EntityCategory::Infantry => self
                .set_infantry_destination(id, NavTargetRef::cell(cell.0, cell.1), rules, None)
                .unwrap_or_else(|error| {
                    log::debug!("Teleport infantry order {id} refused: {error}");
                    false
                }),
            EntityCategory::Unit if self.unit_setter_receiver(id, Some(rules)) => {
                self.set_unit_destination(id, NavTargetRef::cell(cell.0, cell.1), rules, true)
            }
            EntityCategory::Unit => {
                let harvester = self
                    .object_type(entity.type_ref(), rules)
                    .is_some_and(|object| object.harvester);
                self.teleport_move_to(id, cell, rules, harvester, None)
                    .unwrap_or_else(|error| {
                        log::debug!("Unit Teleport order {id}: {error}");
                        false
                    })
            }
            EntityCategory::Aircraft | EntityCategory::Structure => return None,
        })
    }
}
