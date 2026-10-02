//! `Per_Cell_Process`, vtable slot `+0x18C`: what an object runs on reaching a
//! cell. Every native arrival pushes reason 2: Drive (`0x004B1CFD`,
//! `0x004B220F`), Ship (`0x006A1340`, `0x006A1852`), Walk (`0x0075BE3C`),
//! Hover (`0x005146CA`, `0x00515A1C`), the Unit and Infantry tube exits
//! (`0x0073603F`, `0x0051BA9B`), Jumpjet touchdown (`0x0054C8F0`), a
//! parachute landing in `ObjectClass::AI` (`0x005F3F8D`) and Teleport
//! relocation (`0x0071971C`, `0x00719ADE`). Drive and Ship turn completion
//! pass another reason.
//!
//! The class overrides (`UnitClass` `0x00739EC0`, `InfantryClass`
//! `0x00519630`) run their own work, then `FootClass` `0x004D85D0`, which
//! ends in the Techno tail `0x006F5090`. Fly has no caller.

use super::ground_pose::position_world_coord;
use super::locomotor::MovementLayer;
use crate::map::entities::EntityCategory;
use crate::map::overlay_types::OverlayTypeRegistry;
use crate::rules::ruleset::RuleSet;
use crate::sim::lifecycle_request::{LifecycleRequest, UninitReason};
use crate::sim::world::{FrameAdvanceError, Simulation};

/// The reason a `Per_Cell_Process` call passes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PerCellReason {
    /// Drive/Ship turn completion: the Foot body and Techno tail skip it
    /// (`0x004D85DE`, `0x006F509D`).
    TurnComplete,
    /// Reason 2, every cell arrival.
    Arrival,
}

impl Simulation {
    /// The virtual call: the Unit or Infantry override, else the Foot body.
    /// Returns whether the Infantry override changed a bridge.
    pub(crate) fn per_cell_process(
        &mut self,
        id: u64,
        reason: PerCellReason,
        rules: Option<&RuleSet>,
        registry: Option<&OverlayTypeRegistry>,
    ) -> Result<bool, FrameAdvanceError> {
        match self
            .substrate
            .entities
            .get(id)
            .map(|entity| entity.category)
        {
            Some(EntityCategory::Unit) => {
                self.unit_per_cell_process(id, reason, rules, registry);
                Ok(false)
            }
            Some(EntityCategory::Infantry) => {
                self.infantry_per_cell_process(id, reason, rules, registry)
            }
            Some(_) => {
                self.foot_per_cell_process(id, reason, rules, registry);
                Ok(false)
            }
            None => Ok(false),
        }
    }

    /// `UnitClass::Per_Cell_Process @ 0x00739EC0`: the Unit's own arrival
    /// work, then the Foot body (`0x0073A4FB`, `0x0073B0A0`).
    pub(super) fn unit_per_cell_process(
        &mut self,
        id: u64,
        reason: PerCellReason,
        rules: Option<&RuleSet>,
        registry: Option<&OverlayTypeRegistry>,
    ) {
        // Unit739EC0 invokes the MCV receiver before normal crush/Foot tail.
        if let Some(rules) = rules {
            crate::sim::mcv_deploy::per_cell_process(self, id, rules, registry);
        }
        if !self.track_survives(id) {
            return;
        }
        // 0x0073A31F..0x0073A5EA, before the Ready/Commence below: a tethered
        // unit on Enter arriving north-adjacent to its dock sends DOCK_NOW.
        if let Some(rules) = rules
            && reason == PerCellReason::Arrival
        {
            crate::sim::miner::per_cell_dock_now(self, rules, id);
        }
        // Unit PerCell2 739EC0: after MCV retry, +6D1==0 admits
        // Ready(+200)73ACC2 -> Commence(+1EC)73ACD1, BEFORE full-cell
        // crush73B089 and Foot sensor/playfield tail73B0A0. The existing
        // miner.unload_active owns +6D1 (ctor7353FE, unload73DFDA).
        // This promotes only: it does not dispatch a mission handler or
        // repeat the object AI prefix. mission_host_promote retains its
        // documented unavailable locomotor/height fallback for other inputs.
        let promote = reason == PerCellReason::Arrival
            && self.substrate.entities.get(id).is_some_and(|entity| {
                !entity
                    .miner
                    .as_ref()
                    .is_some_and(|miner| miner.unload_active)
            });
        if let Some(rules) = rules.filter(|_| promote) {
            self.mission_host_promote(id, self.session.binary_frame, rules);
        }
        // 0x0073ACD7..0x0073ADC4: a harvester (or weeder) off Enter/Unload
        // drops a refinery (weeder) contact at every track end.
        if let Some(rules) = rules
            && reason == PerCellReason::Arrival
        {
            crate::sim::miner::per_cell_release_dock_contact(self, rules, id);
        }
        let Some(entity) = self.substrate.entities.get(id) else {
            return;
        };
        let at = (entity.position.rx, entity.position.ry);
        let layer = if entity.on_bridge {
            MovementLayer::Bridge
        } else {
            MovementLayer::Ground
        };
        let mut cursor = self
            .substrate
            .occupancy
            .get(at.0, at.1)
            .and_then(|list| list.first_on_layer(layer));
        while let Some(victim) = cursor {
            // Save successor BEFORE lifecycle can remove the current object.
            cursor = self
                .substrate
                .occupancy
                .get(at.0, at.1)
                .and_then(|list| list.next_on_layer(layer, victim));
            if victim == id {
                continue;
            }
            let Some(crusher) = self.substrate.entities.get(id) else {
                return;
            };
            let coord = position_world_coord(&crusher.position);
            let capability = super::bump_crush::CrushCapability::new(
                crusher.regular_crusher,
                crusher.omni_crusher,
            );
            let kills = super::bump_crush::select_crush_victims(
                &[victim],
                &self.substrate.entities,
                id,
                &self.house_alliances,
                &self.interner,
                (coord.x, coord.y),
                capability,
                self.session.binary_frame,
            );
            if !kills.contains(&victim) {
                continue;
            }
            if let Some(rules) = rules {
                if let Some(entity) = self.substrate.entities.get(victim) {
                    super::bump_crush::emit_crush_kill_sounds_at(
                        entity,
                        (i32::from(at.0), i32::from(at.1)),
                        rules,
                        &mut self.interner,
                        &mut self.sound_events,
                    );
                }
                let owner = self.substrate.entities.get(id).map(|entity| entity.owner());
                if let Some(entity) = self.substrate.entities.get_mut(victim) {
                    entity.health.current = 0;
                }
                self.record_the_kill(
                    victim,
                    Some(id),
                    owner,
                    crate::sim::combat::KillCallback::Terminal,
                    rules,
                );
                self.apply_lifecycle_request_with_rules(
                    LifecycleRequest::Uninit {
                        stable_id: victim,
                        reason: UninitReason::Crush,
                    },
                    rules,
                );
            } else {
                self.apply_lifecycle_request(LifecycleRequest::Uninit {
                    stable_id: victim,
                    reason: UninitReason::Crush,
                });
            }
        }
        // 0x0073B08F tests only IsAlive (`+0x90`) before the Foot body.
        if self.per_cell_owner_alive(id) {
            self.foot_per_cell_process(id, reason, rules, registry);
        }
    }

    /// `InfantryClass::Per_Cell_Process @ 0x00519630`: for reason 2 its
    /// Engineer building receiver (`0x00519948..0x0051A02E`), then the Foot body for
    /// a live owner (`0x0051A9EB` tests only IsAlive, `+0x90`).
    ///
    /// RESIDUAL: the override's other reason-2 arms (other Capture branches, Eaten, Enter,
    /// the transport and C4 receivers, `0x00519675..0x0051A9E8`) are not
    /// dispatched from here. Some have ports with their own owners and
    /// callers (`capture_manager`, `passenger`, `world_orders`); native runs
    /// them inside this call, before the Foot body, and several return
    /// without it. Trigger: an infantry arriving on a mission cell. Risk:
    /// same-frame order between those receivers and the Foot body.
    fn infantry_per_cell_process(
        &mut self,
        id: u64,
        reason: PerCellReason,
        rules: Option<&RuleSet>,
        registry: Option<&OverlayTypeRegistry>,
    ) -> Result<bool, FrameAdvanceError> {
        let entry = match rules {
            Some(rules) if reason == PerCellReason::Arrival => {
                self.infantry_per_cell_engineer_entry(id, rules, registry)?
            }
            _ => Default::default(),
        };
        if !entry.return_before_foot && self.per_cell_owner_alive(id) {
            self.foot_per_cell_process(id, reason, rules, registry);
        }
        Ok(entry.bridge_state_changed)
    }

    fn per_cell_owner_alive(&self, id: u64) -> bool {
        self.substrate
            .entities
            .get(id)
            .is_some_and(|entity| entity.lifecycle.object_alive)
    }

    /// `FootClass::Per_Cell_Process @ 0x004D85D0` for reason 2, ending in the
    /// Techno tail `0x006F5090`; both return at once for any other reason.
    ///
    /// Ported, in native order: the sensor deposit move (`0x004D8611`,
    /// `0x004D8621`), the neighbour history (`0x004D8627..0x004D8759`), the
    /// `Sensors=` uncloak scan (`0x004D8760..0x004D882F`), the range stop
    /// (`0x004D882F..0x004D896E`) and the Techno tail's Temporal release
    /// (`0x006F50A3..0x006F50B4`) and playfield promote (`0x006F511A`).
    ///
    /// RESIDUAL: the cell tag events (`0x004D8978..0x004D8D99`), the
    /// `+0x83` / `0x00586360` step (`0x004D8DA7..0x004D8DF5`), the
    /// planning-waypoint upkeep (`0x004D8DFF..0x004D8F1D`) and the Techno
    /// tail's `+0x420` call, tag event 0x22, `+0x198` call and cell
    /// `0x00486920` are not ported. Trigger: every arrival. Effect: cell-tag
    /// triggers and planning waypoints do not fire from movement. Risk: maps
    /// with cell triggers.
    pub(super) fn foot_per_cell_process(
        &mut self,
        id: u64,
        reason: PerCellReason,
        rules: Option<&RuleSet>,
        registry: Option<&OverlayTypeRegistry>,
    ) {
        if reason != PerCellReason::Arrival {
            return;
        }
        if let Some(rules) = rules {
            self.refresh_unit_sensor_at_per_cell(id, rules);
        }
        //4D8657..4D868D compares the two coarse threat buckets and migrates
        //the cached contribution BEFORE4D870E replaces retained Foot+55C.
        if let Some(rules) = rules {
            self.spatial_threat_at_per_cell(id, rules);
        }
        self.foot_neighbors_at_per_cell(id);
        if let Some(rules) = rules {
            crate::sim::world::techno_ai_cloak::uncloak_on_sensor_neighbour_after_cell_entry(
                self, id, rules,
            );
            self.per_cell_range_stop(id, rules, registry);
        }
        // `0x006F5090`'s head lets a held Temporal target go.
        self.temporal_release_if_warping(id);
        self.promote_entity_playfield_membership_after_move(id);
    }

    /// The range stop (`0x004D882F..0x004D896E`): the class
    /// `Set_Destination(NULL, 1)` (vt `+0x480`), then `+0x5E0 = -1`, which
    /// runs even when the class refuses. See
    /// tools/spatial_oracle/walk_percell_stop.{py,json,meta.json}.
    ///
    /// A Unit takes only the `OpenTopped=` arm here; the pursuit stage's
    /// in-range halt (`world_orders.rs`) stands in for its InRange arm, tested
    /// each frame. Every infantryman takes Infantry `0x0051AA40`, a Rocketeer
    /// touching down in range included.
    fn per_cell_range_stop(
        &mut self,
        id: u64,
        rules: &RuleSet,
        registry: Option<&OverlayTypeRegistry>,
    ) {
        let Some(category) = self
            .substrate
            .entities
            .get(id)
            .map(|entity| entity.category)
        else {
            return;
        };
        if category == EntityCategory::Unit
            && !self
                .substrate
                .entities
                .get(id)
                .and_then(|entity| self.object_type(entity.type_ref(), rules))
                .is_some_and(|obj| obj.open_topped)
        {
            return;
        }
        if !self.foot_per_cell_range_stop(id, rules, registry) {
            return;
        }
        match category {
            EntityCategory::Unit => {
                self.set_unit_null_destination(id, Some(rules), registry);
            }
            EntityCategory::Infantry => {
                // With no head and no destination left, the next Walk
                // Process takes its idle tail and retires the adapter.
                self.set_infantry_null_destination(id, Some(rules), registry);
            }
            _ => {}
        }
        if let Some(entity) = self.substrate.entities.get_mut(id) {
            // Keep the backing suffix/cursor/reference; this is one DWORD.
            entity.navigation.path_replay.clear_live_head();
        }
    }
}
