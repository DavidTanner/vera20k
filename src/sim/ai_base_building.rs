//! The computer's building choice, the BasePlan node it builds from and the
//! Construction Yard's placement of the finished building.
//!
//! Native owners: `HouseClass` holds the production mode (`+0x1E4`), the
//! BuildingType its Construction Yard makes next (`+0x564C`) and the naval
//! latch (`+0x1F0`); its update picks the building every eight frames
//! (`HouseClass::Update @ 0x004F9038..0x004F9265` → `AI_Choose_Building @
//! 0x004FE3E0`) and the exit of a produced object steps the mode
//! (`EconomyStateMachine @ 0x00509700`). The node lookup is `BaseClass`'s
//! (`0x0042EB50` with `0x0042E780`, `0x0042E820` and `0x0050CAD0`); a `-1`
//! node becomes a base defense (`sim::ai_base_defense`). The yard
//! (`production::factory_ai`) makes the choice (`Suggest_New_Object @
//! 0x004FBD80`), places it (`BuildingClass::Exit_Object @ 0x00443C60`'s
//! building case, with the site search in `sim::ai_base_site` and the site
//! clearing in `sim::build_site`) and records it (`Record_Last_Built @
//! 0x004FB6B0`, owned by `production::factory_lifecycle`, which also takes
//! the player's placements and every delivered unit).
//!
//! Evidence: instruction reading of the bodies named on each function; the
//! draws and the mode table are executed by `tools/ai_base_building_oracle.py`
//! (see the tests in `ai_base_building_tests.rs`), whose chooser rows answer
//! the walls and the defense choice with failure.
//!
//! RESIDUALS:
//! - `HouseClass::AI_Building_Strategy @ 0x004FD500` is not scheduled
//!   (`house_strategy`'s module doc). Its `AI_Manage_Build_Queue @ 0x004FDD10`
//!   also writes the mode (2 at `0x004FE109`, 1 at `0x004FE3AB`), clears the
//!   four choices, abandons every factory of the house when its planned
//!   buildings cost more than its budget, splices refinery and war factory
//!   nodes and calls `AI_Choose_Building` directly; the strategy reschedules
//!   itself with `RandomRanged(1,7) + 105` (`0x004FD91C..0x004FD928`).
//!   Trigger: every computer house, every 106..112 frames. Effect: the mode
//!   changes only at exits, a computer short of money never cancels its
//!   production, it rebuilds a lost refinery or war factory only when its
//!   original node is replanned, and those draws are missing from the
//!   Scenario RNG sequence.
//! - The Unit, Infantry and Aircraft choosers (`0x004FEA60`, `0x004FEEE0`,
//!   `0x004FF210`) are not ported: their choices (`+0x5650/+0x5654/+0x5658`)
//!   stay -1 and their draws are missing. `sim::ai`'s unit queue stands in.
//!   Trigger: every computer house, every eighth frame. Effect: the chooser
//!   block runs the building choice in modes 0, 1 and 2 and nothing else. In
//!   mode 2 (entered at a building exit below `[General]
//!   AIAlternateProductionCreditCutoff=`) native runs those choosers first
//!   and chooses a building only when all three choices are -1 or a chosen
//!   type has no factory (`0x004F90F0..0x004F9247`, FindFactory `vt+0x94`), so
//!   a native computer below the cutoff pauses its structures while unit
//!   choices are pending; VERA's keeps choosing buildings.
//! - The unit exits of `BuildingClass::Exit_Object` also step
//!   `EconomyStateMachine` (an aircraft with kind 2 at `0x00443CBC`, a unit or
//!   infantry leaving a factory that is neither `Hospital=` nor `Armory=`
//!   with its RTTI at `0x00444102`) and clear that kind's choice; VERA's unit
//!   deliveries (`production::production_queue`, the stand-in's path) do not
//!   call it. Trigger: every computer unit that leaves a factory. Effect: the
//!   mode changes only at building exits, and the state-2 draw
//!   (`0x00509863`) is missing at unit exits. The computer's unit production
//!   (Factory_AI's unit factories and their exits) is a later chain.
//! - A `-1` defense node (and a WallTower node without a cell) takes the wall
//!   draw (`RandomRanged(0,99)` at `0x004FE59E`); below the difficulty's
//!   `AIPickWallDefensePercent=` native first tries the walls of
//!   `0x0050C340`, which are not ported: VERA takes their failure and goes on
//!   to the defense choice (`sim::ai_base_defense`). Trigger: every skirmish
//!   plan, which Recalc seeds with defense sentinels after its fourth node,
//!   on a draw below the percent. Effect: the computer builds no walls.
//!   Where native walls a `ProtectWithWall=` building instead (wall nodes
//!   after it, the `-1` node removed, no further draw), VERA chooses a
//!   defense and makes that choice's draws (three threat draws with an
//!   enemy, one pick draw over several candidates).
//! - A `-3` node is removed without `0x005082C0`'s perimeter scan; only a map
//!   plan can hold one.
//! - Dormant in retail data: the `PowersUpBuilding=` arms of `0x0042E820` and
//!   `AI_Choose_Building` (`0x004FE953..0x004FEA3A`); no retail type sets the
//!   key and VERA reads no upgrade (as `production_sell` and
//!   `building_missions` record). The exit's `FirestormWall=` placement
//!   after Unlimbo (`0x00445355..0x004453AE`, `0x00588570`); retail
//!   `rulesmd.ini` sets the key on no type.
//! - `Record_Last_Built` also stores the last built type of the object's kind
//!   (`+0x26C` for buildings), which only the "built building type" trigger
//!   event reads (`TriggerCondition::Evaluate`, `0x0071EFEB`; VERA's trigger
//!   runtime has no such event); sets `+0x246`, which only restarts a
//!   90-frame House timer nothing else reads (`0x004F8DF6..0x004F8E21`); sets
//!   `+0x1FC`, whose revoke and grant pass on the next House update
//!   (`0x004F92E9..0x004F9302`) VERA runs at the placement; and plays the
//!   type's `CreateSound=` (`+0x534`) or, for a unit, infantry or aircraft,
//!   a Rules default (`+0x178`, `+0x17C`, `+0x180`); presentation, not
//!   parsed.
//! - Native crash or out-of-range reads that VERA defines: a node index past
//!   the BuildingType array or a negative control other than -1/-2/-3 reads
//!   memory past the array (`0x004FE537`, `0x004FE6F6`); a null power plant
//!   slot is dereferenced (`0x004FE8A9`, `0x00505368`). VERA makes no choice
//!   for such a node and splices nothing for a missing plant.

use crate::map::overlay_types::OverlayTypeRegistry;
use crate::rules::object_type::{FactoryType, ObjectType};
use crate::rules::ruleset::RuleSet;
use crate::sim::ai_base_site::{SiteKey, find_base_building_site, reserved_near};
use crate::sim::base_plan::{BasePlanNode, pack_base_plan_cell, unpack_base_plan_cell};
use crate::sim::build_site::{Flush, can_place_building_at, flush_for_placement};
use crate::sim::components::BuildingUp;
use crate::sim::intern::InternedId;
use crate::sim::movement::locomotor::MovementLayer;
use crate::sim::pathfinding::PathGrid;
use crate::sim::world::{PlacementEvidence, Simulation};

/// The computer's production state on its House.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct HouseAiProduction {
    /// `HouseClass+0x1E4`, the mode `EconomyStateMachine` steps and the
    /// chooser block dispatches on.
    mode: i32,
    /// `HouseClass+0x564C`, the BuildingType index the yard's next factory
    /// makes (`Suggest_New_Object @ 0x004FBD80`, `0x004FBDDC`); -1 is none.
    building_choice: i32,
    /// `HouseClass+0x1F0`: while clear, the choice skips naval nodes. A naval
    /// building the yard could not place clears it (`0x00450219..0x00450220`);
    /// nothing sets it again (the other clearing writer, `0x005048A0`, has no
    /// caller).
    naval_allowed: bool,
}

impl Default for HouseAiProduction {
    /// `HouseClass::Constructor`: mode 0 (`0x004F56DF`, EBX), choice -1
    /// (`0x004F5BC0`, ESI), naval 1 (`0x004F56FD`).
    fn default() -> Self {
        Self {
            mode: 0,
            building_choice: -1,
            naval_allowed: true,
        }
    }
}

impl HouseAiProduction {
    #[cfg(test)]
    pub(crate) const fn mode(&self) -> i32 {
        self.mode
    }

    #[cfg(test)]
    pub(crate) const fn building_choice(&self) -> i32 {
        self.building_choice
    }

    /// The building exit (`0x00444F3F`, `0x0044531F`) and a computer house's
    /// abandoned building (`FactoryClass::AbandonProduction`, `0x004CA0DD`).
    pub(crate) fn clear_building_choice(&mut self) {
        self.building_choice = -1;
    }

    /// `BuildingClass::Factory_AI`'s refused naval building (`0x00450220`).
    pub(crate) fn forbid_naval(&mut self) {
        self.naval_allowed = false;
    }

    #[cfg(test)]
    pub(crate) fn set_for_test(&mut self, mode: i32, building_choice: i32, naval_allowed: bool) {
        self.mode = mode;
        self.building_choice = building_choice;
        self.naval_allowed = naval_allowed;
    }
}

/// `HouseClass::AI_EconomyStateMachine @ 0x00509700`, from a produced
/// object's exit: `building_exit` is the Building exit's kind 6
/// (`0x00444F34`); the unit and infantry exit passes the object's kind
/// (`0x00444102`), the aircraft exit 2 (`0x00443CBC`).
///
/// A house no human controls in a skirmish steps its mode against
/// `AIAlternateProductionCreditCutoff=`: below it, mode 0 moves to 2 for a
/// building and 1 otherwise, mode 1 moves to 2 for a building; mode 2 stays
/// only if it owns a barracks and a war factory, has enough power and draws
/// 1 from `RandomRanged(0,1)` (`0x00509863`), else a non-building exit moves
/// it to 1; at or above it, modes 1 and 2 return to 0, and mode 3 only above.
pub(crate) fn economy_state_machine(
    sim: &mut Simulation,
    rules: &RuleSet,
    owner: InternedId,
    building_exit: bool,
) {
    let game_mode_nonzero = sim.session.game_mode_nonzero;
    let Some(house) = sim.houses.get(&owner) else {
        return;
    };
    if house.is_controlled_by_human(game_mode_nonzero) || !game_mode_nonzero {
        return;
    }
    let mode = house.ai_production.mode;
    let money = crate::sim::credit_income::available_money(sim, owner);
    let cutoff = rules.general.ai_alternate_production_credit_cutoff;
    let next = match mode {
        0 => (money < cutoff).then_some(if building_exit { 2 } else { 1 }),
        1 if money >= cutoff => Some(0),
        1 => building_exit.then_some(2),
        2 if money >= cutoff => Some(0),
        2 => {
            // `0x005097A2..0x0050983A`: one owned (on-map, `+0x5550`) type
            // from each list.
            let owns_any = |types: &[String]| {
                types.iter().any(|name| {
                    sim.interner
                        .get(name)
                        .is_some_and(|id| house.tracking.active_building_count(id) > 0)
                })
            };
            let (output, drain) = sim
                .power_states
                .get(&owner)
                .map_or((0, 0), |power| (power.total_output, power.total_drain));
            let may_stay = owns_any(&rules.build_barracks_types)
                && owns_any(&rules.build_weapons_types)
                && drain <= output;
            if may_stay && sim.scenario_rng.next_range_i32_inclusive(0, 1) != 0 {
                None
            } else {
                (!building_exit).then_some(1)
            }
        }
        3 => (money > cutoff).then_some(0),
        // Modes above 3 (unsigned, `0x00509742`) change nothing.
        _ => None,
    };
    if let Some(next) = next
        && let Some(house) = sim.houses.get_mut(&owner)
    {
        house.ai_production.mode = next;
    }
}

/// The building choice of `HouseClass::Update`'s chooser block
/// (`0x004F9038..0x004F9265`): a house no human controls, whose HouseType is
/// not `MultiplayPassive=`, on every eighth (signed) frame. Mode 0 and a
/// campaign run the building choice first, mode 1 runs it first and mode 2
/// runs it after the unit choosers when they all hold no choice or a chosen
/// type has no factory; other modes run none. The unit choosers are not
/// ported (module residual), so their choices stay -1 and the building
/// choice runs in modes 0, 1 and 2.
pub(crate) fn update_building_choice(
    sim: &mut Simulation,
    rules: &RuleSet,
    owner: InternedId,
    path_grid: Option<&PathGrid>,
    registry: Option<&OverlayTypeRegistry>,
) {
    let game_mode_nonzero = sim.session.game_mode_nonzero;
    let Some(house) = sim.houses.get(&owner) else {
        return;
    };
    if house.is_controlled_by_human(game_mode_nonzero) || house.multiplay_passive {
        return;
    }
    if (sim.session.binary_frame as i32) % 8 != 0 {
        return;
    }
    let runs_building_choice = !game_mode_nonzero || matches!(house.ai_production.mode, 0..=2);
    if runs_building_choice {
        choose_building(sim, rules, owner, path_grid, registry);
    }
}

/// `HouseClass::AI_Choose_Building @ 0x004FE3E0`.
fn choose_building(
    sim: &mut Simulation,
    rules: &RuleSet,
    owner: InternedId,
    path_grid: Option<&PathGrid>,
    registry: Option<&OverlayTypeRegistry>,
) {
    let Some(house) = sim.houses.get(&owner) else {
        return;
    };
    // `0x004FE3E9..0x004FE416`: a choice already waits, the house has no
    // Construction Yard (`+0x60`, the ConYards count), or no node is open.
    if house.ai_production.building_choice != -1 || house.build_const_order.is_empty() {
        return;
    }
    let naval_allowed = house.ai_production.naval_allowed;
    let Some(mut index) = find_node(sim, rules, owner, -1, registry) else {
        return;
    };
    // `0x004FE41C..0x004FE4A4`: a naval node while naval building is off is
    // removed, and the lookup runs once more.
    let node_type = plan_node(sim, owner, index).type_or_control;
    if !naval_allowed && rules.building_type_at(node_type).is_some_and(|ty| ty.naval) {
        remove_node(sim, owner, index);
        let Some(next) = find_node(sim, rules, owner, -1, registry) else {
            return;
        };
        index = next;
    }
    let mut node = plan_node(sim, owner, index);
    // `0x004FE4B0..0x004FE518`: remove a perimeter node (its `0x005082C0`
    // scan is a residual).
    if node.type_or_control == -3 {
        remove_node(sim, owner, index);
        return;
    }
    let wall_tower = rules.building_type_at(node.type_or_control).filter(|ty| {
        rules
            .general
            .building_types
            .wall_tower
            .as_deref()
            .is_some_and(|name| name.eq_ignore_ascii_case(&ty.id))
    });
    if node.type_or_control == -1 || (wall_tower.is_some() && node.packed_cell == 0) {
        // `0x004FE59E`: the wall draw. The walls (`0x0050C340`) are a
        // residual whose failure leads here too.
        let _ = sim.scenario_rng.next_range_i32_inclusive(0, 99);
        if !crate::sim::ai_base_defense::choose_next_production(
            sim, rules, owner, index, path_grid, registry,
        ) {
            // `0x004FE640..0x004FE6D2`: a WallTower node goes first, then
            // the node at its index.
            if wall_tower.is_some() {
                remove_node(sim, owner, index);
            }
            remove_node(sim, owner, index);
            return;
        }
        // `0x004FE6E1`: the node now holds the defense (or the WallTower its
        // cell) and takes the ordinary path.
        node = plan_node(sim, owner, index);
    }
    if node.type_or_control == -2 {
        return;
    }
    let Some(ty) = rules.building_type_at(node.type_or_control) else {
        return;
    };
    if let Some(plant) = power_plant_to_splice(sim, rules, owner, ty) {
        // `0x004FE893..0x004FE943`: the plant takes the node's index, the
        // node and the rest shift up.
        if let Some(house) = sim.houses.get_mut(&owner) {
            house.base_plan.nodes.insert(
                index,
                BasePlanNode {
                    type_or_control: plant,
                    packed_cell: 0,
                    filled: false,
                    retry_count: 0,
                },
            );
        }
        return;
    }
    // `0x004FEA3C..0x004FEA42`.
    if let Some(house) = sim.houses.get_mut(&owner) {
        house.ai_production.building_choice = node.type_or_control;
    }
}

/// The power splice gates of `AI_Choose_Building` (`0x004FE6F6..0x004FE8A6`):
/// the building's drain would exceed the house's output (less `+0x160B4`,
/// which only the constructor writes, zero), it is not a `BuildConst=` type,
/// it drains power, no blackout remains and no powered building is being
/// drained (`+0x577B`). Returns the plant's BuildingType index: the side 0
/// house's `GDIPowerPlant=`, the side 2 house's `ThirdPowerPlant=`, any other
/// side's `NodAdvancedPower=` when `0x00505360` admits it, else
/// `NodRegularPower=`.
fn power_plant_to_splice(
    sim: &Simulation,
    rules: &RuleSet,
    owner: InternedId,
    ty: &ObjectType,
) -> Option<i32> {
    let house = sim.houses.get(&owner)?;
    let power = sim.power_states.get(&owner).cloned().unwrap_or_default();
    let drain = crate::sim::power_system::native_building_power_drain(ty.power);
    let short = power.total_drain.wrapping_add(drain) > power.total_output;
    if !short
        || ty.build_const_eligible
        || drain <= 0
        || power.power_blackout_remaining != 0
        || power.has_drained_power_source
    {
        return None;
    }
    let plants = &rules.general.building_types;
    let index = |name: Option<&str>| name.and_then(|name| rules.building_type_index(name));
    // A missing plant is dereferenced natively (module residual): no splice.
    match house.side_index {
        0 => index(plants.gdi_power_plant.as_deref()),
        2 => index(plants.third_power_plant.as_deref()),
        // `0x004FE7A7..0x004FE85D`: the list handed to `0x00505360` holds the
        // house's BuildingClass objects (`+0x6C`, reversed) where the helper
        // compares BuildingTypes, so a prerequisite never matches and the
        // advanced plant passes only when it has no prerequisite (`+0x648`,
        // `0x00505368..0x00505379`). Retail NANRCT needs NATECH and NACNST,
        // so every such house splices NAPOWR.
        _ => index(plants.nod_advanced_power.as_deref())
            .filter(|&advanced| {
                rules
                    .building_type_at(advanced)
                    .is_some_and(|advanced| advanced.prerequisite.is_empty())
            })
            .or_else(|| index(plants.nod_regular_power.as_deref())),
    }
}

/// What the Construction Yard's exit answers `Factory_AI`
/// (`BuildingClass::Exit_Object @ 0x00443C60`'s return value).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BuildingExit {
    /// 0: nothing was placed; the yard abandons the object.
    Failed,
    /// 1: units were told to clear the site; the yard tries again later.
    TryLater,
    /// 2: the building stands, or its wall is stamped.
    Placed,
}

/// The building case of `BuildingClass::Exit_Object @ 0x00443C60`
/// (`0x00444F19..0x00445696`): yard `factory` places its finished building
/// `product`.
///
/// A human's yard places nothing. Otherwise the exit steps the production
/// mode, forgets the building choice and looks up the product's open node
/// ([`find_node`]). The site is the node's cell while a reservation of the
/// house lies near it, else a new one that the node keeps ([`building_site`]).
/// Units in the way are told to leave (`Flush_For_Placement`): the yard tries
/// later and the node counts a failure. Anything else in the way, or a refused
/// placement, fails ([`forget_failed_site`]).
pub(crate) fn exit_building(
    sim: &mut Simulation,
    rules: &RuleSet,
    factory: u64,
    product: u64,
    path_grid: Option<&PathGrid>,
    registry: Option<&OverlayTypeRegistry>,
) -> BuildingExit {
    let Some(owner) = sim.substrate.entities.get(factory).map(|yard| yard.owner()) else {
        return BuildingExit::Failed;
    };
    // The prelude marks the leaving object a playfield member whatever the
    // outcome (`0x00443C81`); a placement's Unlimbo then sets it exactly.
    if let Some(object) = sim.substrate.entities.get_mut(product) {
        object.in_playfield = true;
    }
    let game_mode_nonzero = sim.session.game_mode_nonzero;
    // `0x00444F19..0x00444F2A` (`0x0050B730`).
    if sim
        .houses
        .get(&owner)
        .is_none_or(|house| house.is_controlled_by_human(game_mode_nonzero))
    {
        return BuildingExit::Failed;
    }
    let Some(ty) = sim
        .substrate
        .entities
        .get(product)
        .and_then(|object| sim.object_type(object.type_ref(), rules))
    else {
        return BuildingExit::Failed;
    };
    economy_state_machine(sim, rules, owner, true);
    if let Some(house) = sim.houses.get_mut(&owner) {
        house.ai_production.clear_building_choice();
    }
    // `0x0042EB20` with the product type's index (`+0xDF8`): the node or none.
    let node = rules
        .building_type_index(&ty.id)
        .and_then(|index| find_node(sim, rules, owner, index, registry));
    let site = building_site(sim, rules, owner, ty, node, path_grid, registry);
    match flush_for_placement(sim, rules, ty, site, owner, path_grid) {
        Flush::Scattered => {
            // `0x00445237..0x004452C3`.
            if let Some(index) = node
                && let Some(house) = sim.houses.get_mut(&owner)
            {
                house.base_plan.count_placement_failure(
                    index,
                    game_mode_nonzero,
                    rules.general.maximum_building_placement_failures,
                );
            }
            BuildingExit::TryLater
        }
        Flush::Blocked => {
            forget_failed_site(sim, rules, owner, node, site);
            BuildingExit::Failed
        }
        Flush::Clear => {
            if !place_building(sim, rules, owner, ty, product, site, registry) {
                forget_failed_site(sim, rules, owner, node, site);
                return BuildingExit::Failed;
            }
            carry_wall_tower_site(sim, rules, owner, ty, node, site);
            log::info!(
                "{} placed {} at ({}, {})",
                sim.interner.resolve(owner),
                ty.id,
                site.0,
                site.1
            );
            BuildingExit::Placed
        }
    }
}

/// `0x00444F65..0x004451AA`: the node's cell when it has one and
/// [`reserved_near`] admits it (`0x00444F9B`; the `PowersUpBuilding=` arm is
/// dormant), else a new site (`0x00444FE1`, `0x004450BD`) that the node, if
/// any, keeps (`0x00445055`, `0x0044519F`). The site's coordinate is its
/// centre at height 0; no ordinary site answers the null coordinate that
/// would take the upgrade arm at `0x00445614`, so a site of (0, 0) goes on to
/// be refused.
#[allow(clippy::too_many_arguments)]
fn building_site(
    sim: &mut Simulation,
    rules: &RuleSet,
    owner: InternedId,
    ty: &ObjectType,
    node: Option<usize>,
    path_grid: Option<&PathGrid>,
    registry: Option<&OverlayTypeRegistry>,
) -> (i16, i16) {
    let node_cell = node
        .map(|index| plan_node(sim, owner, index).packed_cell)
        .filter(|&packed| packed != 0);
    if let Some(packed) = node_cell {
        let cell = unpack_base_plan_cell(packed);
        if reserved_near(sim, rules, owner, ty, cell) {
            return cell;
        }
    }
    let site = find_base_building_site(
        sim,
        rules,
        owner,
        ty,
        SiteKey::Ordinary,
        path_grid,
        registry,
    );
    if let Some(index) = node
        && let Some(house) = sim.houses.get_mut(&owner)
    {
        house.base_plan.nodes[index].packed_cell =
            pack_base_plan_cell(i32::from(site.0), i32::from(site.1));
    }
    site
}

/// The product's `Unlimbo(coord, 0)` at `site` (`0x004452D7`) and what follows
/// it up to the wall fill. `BuildingClass::Unlimbo` admits the site through
/// its `Can_Enter_Cell` (`sim::build_site`). A wall stamps its overlay and
/// deletes the object (`0x00440774..0x00440865`), then fills towards the
/// house's walls (`0x00445408`). Any other building stands and runs its
/// Unlimbo's grant pass (`0x0050B1D0`); its slave manager takes the hand-off
/// (`0x004452F0..0x004452FA`) and its queued Construction mission commences
/// (`0x00445329..0x0044533F`). A computer house's yard stands in the Logic
/// vector, which is re-read, so the new building's first Update follows
/// later in the same frame ([`BuildingUp::placed_by_computer`]).
fn place_building(
    sim: &mut Simulation,
    rules: &RuleSet,
    owner: InternedId,
    ty: &ObjectType,
    product: u64,
    site: (i16, i16),
    registry: Option<&OverlayTypeRegistry>,
) -> bool {
    if !can_place_building_at(sim, rules, registry, ty, site, Some(owner)) {
        return false;
    }
    let (Ok(rx), Ok(ry)) = (u16::try_from(site.0), u16::try_from(site.1)) else {
        return false;
    };
    if ty.wall {
        let Some(registry) = registry else {
            return false;
        };
        if !crate::sim::production::stamp_wall_with_autofill(
            sim,
            rules,
            registry,
            ty,
            (rx, ry),
            owner,
        ) {
            return false;
        }
        let _ = sim.discard_constructed_limbo(product);
        return true;
    }
    let z = sim
        .resolved_terrain
        .as_ref()
        .and_then(|terrain| terrain.cell(rx, ry))
        .map_or(0, |cell| cell.level);
    if sim
        .unlimbo_held_production_object(
            product,
            rx,
            ry,
            0,
            z,
            PlacementEvidence::EvaluateMark,
            rules,
        )
        .is_none()
    {
        return false;
    }
    sim.mission_spawned_entities = true;
    if sim.session.game_options.super_weapons {
        crate::sim::superweapon::refresh_super_weapons_for_owner(sim, rules, owner);
    }
    sim.slave_manager_hand_off(product, rules);
    // The choice clear at `0x0044531F` compares the product with the choice
    // the exit cleared at its start; nothing in between sets it.
    let control = rules.buildup_control(&ty.id);
    let now = sim.session.binary_frame as i32;
    if let Some(building) = sim.substrate.entities.get_mut(product) {
        building.building_up = Some(BuildingUp::placed_by_computer(control, now));
    }
    true
}

/// `0x004454E6..0x00445611`: after a failure, a wall or gate node
/// (`+0x1571`, `+0x16B7`) is removed; any other node's site is forgotten by
/// every node that holds it. Without a node nothing changes.
fn forget_failed_site(
    sim: &mut Simulation,
    rules: &RuleSet,
    owner: InternedId,
    node: Option<usize>,
    site: (i16, i16),
) {
    let Some(index) = node else {
        return;
    };
    let node_type = plan_node(sim, owner, index).type_or_control;
    if rules
        .building_type_at(node_type)
        .is_some_and(|ty| ty.wall || ty.gate)
    {
        remove_node(sim, owner, index);
    } else if let Some(house) = sim.houses.get_mut(&owner) {
        house
            .base_plan
            .clear_failed_site(pack_base_plan_cell(i32::from(site.0), i32::from(site.1)));
    }
}

/// `0x0044540D..0x004454D4`: a placed `WallTower=` building hands its cell to
/// the first base defense node (`+0x1706`) after its own.
fn carry_wall_tower_site(
    sim: &mut Simulation,
    rules: &RuleSet,
    owner: InternedId,
    ty: &ObjectType,
    node: Option<usize>,
    site: (i16, i16),
) {
    let wall_tower = rules
        .general
        .building_types
        .wall_tower
        .as_deref()
        .is_some_and(|name| name.eq_ignore_ascii_case(&ty.id));
    // Without a node the index lookup (`vt+0x14`, `0x0042F3D0`) maps the null
    // pointer to `(0 - items) >> 4`, past the count, so no node takes the
    // cell.
    let (true, Some(index)) = (wall_tower, node) else {
        return;
    };
    let Some(house) = sim.houses.get_mut(&owner) else {
        return;
    };
    let defense = house
        .base_plan
        .nodes
        .iter_mut()
        .skip(index + 1)
        .find(|next| {
            rules
                .building_type_at(next.type_or_control)
                .is_some_and(|ty| ty.is_base_defense)
        });
    if let Some(defense) = defense {
        defense.packed_cell = pack_base_plan_cell(i32::from(site.0), i32::from(site.1));
    }
}

/// `HouseClass::Suggest_New_Object @ 0x004FBD80` for a factory that makes
/// `factory_type`: the house's choice of that kind, unchecked. Only the
/// building choice (`+0x564C`, `0x004FBDDC`) is ever made (module residual).
pub(crate) fn suggest_new_object<'r>(
    sim: &Simulation,
    rules: &'r RuleSet,
    owner: InternedId,
    factory_type: FactoryType,
) -> Option<&'r ObjectType> {
    let house = sim.houses.get(&owner)?;
    match factory_type {
        FactoryType::BuildingType => rules.building_type_at(house.ai_production.building_choice),
        FactoryType::UnitType | FactoryType::InfantryType | FactoryType::AircraftType => None,
    }
}

/// `BaseClass::Find_Node @ 0x0042EB50` for BuildingType index `wanted`, or
/// the first open node when `wanted` is -1. A node is open when
/// [`node_satisfied`] rejects it; for -1 in a skirmish a filled node is open
/// only if [`node_may_replan`] admits it, and then its cell is cleared
/// (`0x0042EBC3..0x0042EBD1`, the zeroed static `0x0089C310`).
pub(crate) fn find_node(
    sim: &mut Simulation,
    rules: &RuleSet,
    owner: InternedId,
    wanted: i32,
    registry: Option<&OverlayTypeRegistry>,
) -> Option<usize> {
    let game_mode_nonzero = sim.session.game_mode_nonzero;
    let mut index = 0;
    // The count is re-read every step (`0x0042EBAE`).
    while index < sim.houses.get(&owner)?.base_plan.nodes.len() {
        let node = plan_node(sim, owner, index);
        if wanted == -1 {
            if !node_satisfied(sim, rules, owner, &node, registry) {
                if !game_mode_nonzero || !node.filled {
                    return Some(index);
                }
                if node_may_replan(sim, rules, owner, &node) {
                    if let Some(house) = sim.houses.get_mut(&owner) {
                        house.base_plan.nodes[index].packed_cell = 0;
                    }
                    return Some(index);
                }
            }
        } else if node.type_or_control == wanted
            && !node_satisfied(sim, rules, owner, &node, registry)
        {
            return Some(index);
        }
        index += 1;
    }
    None
}

/// `BaseClass @ 0x0042E780`: the node's building stands ([`node_building`]),
/// or a wall node's cell holds its wall overlay or any building.
fn node_satisfied(
    sim: &Simulation,
    rules: &RuleSet,
    owner: InternedId,
    node: &BasePlanNode,
    registry: Option<&OverlayTypeRegistry>,
) -> bool {
    if node_building(sim, owner, node).is_some() {
        return true;
    }
    let Some(ty) = rules.building_type_at(node.type_or_control) else {
        return false;
    };
    if !ty.wall {
        return false;
    }
    let Some((rx, ry)) = map_cell(node.packed_cell) else {
        return false;
    };
    // `cell+0x44 == ToOverlay(+0xE54)->ArrayIndex` (`0x0042E7CA..0x0042E7D9`).
    let wall_overlay = ty
        .to_overlay
        .as_deref()
        .and_then(|name| registry?.id_for_name(name));
    let overlay = sim
        .overlay_grid
        .as_ref()
        .and_then(|grid| grid.cell(rx, ry).overlay_id);
    (wall_overlay.is_some() && overlay == wall_overlay)
        || sim
            .substrate
            .occupancy
            .first_building_on_layer(rx, ry, MovementLayer::Ground)
            .is_some()
}

/// `BaseClass @ 0x0042E820`: the house's building of the node's type whose
/// top-left cell is the node's cell. The `PowersUpBuilding=` arm is dormant
/// (module residual).
fn node_building(sim: &Simulation, owner: InternedId, node: &BasePlanNode) -> Option<u64> {
    if node.packed_cell == 0 || node.type_or_control < 0 {
        return None;
    }
    let (rx, ry) = map_cell(node.packed_cell)?;
    let id = sim
        .substrate
        .occupancy
        .first_building_on_layer(rx, ry, MovementLayer::Ground)?;
    let building = sim.substrate.entities.get(id)?;
    ((building.position.rx, building.position.ry) == (rx, ry)
        && building.owner() == owner
        && building.base_plan_type_index == node.type_or_control)
        .then_some(id)
}

/// `HouseClass @ 0x0050CAD0`: whether a filled node whose building is gone
/// may be planned again. A control node may; a refinery that gathers (a
/// deployed Slave Miner) only while the house has fewer gatherers on the map
/// (`+0x158`) than `AISlaveMinerNumber=` allows. In a skirmish an armed type,
/// a wall next to one of the house's buildings or a power plant may; any other
/// type only once `AIRestrictReplaceTime=` frames have passed since a
/// building of the house was attacked (`+0x54D8`).
fn node_may_replan(
    sim: &Simulation,
    rules: &RuleSet,
    owner: InternedId,
    node: &BasePlanNode,
) -> bool {
    if node.type_or_control < 0 {
        return true;
    }
    let Some(ty) = rules.building_type_at(node.type_or_control) else {
        return false;
    };
    let Some(house) = sim.houses.get(&owner) else {
        return false;
    };
    if ty.resource_destination && ty.resource_gatherer {
        // The Recalc preflight validated this row before the plan existed.
        return rules
            .ai_slave_miner_number
            .get(house.difficulty.table_index())
            .is_some_and(|&allowed| house.tracking.resource_gatherers() < allowed);
    }
    if !sim.session.game_mode_nonzero {
        return true;
    }
    // `TechnoTypeClass::GetWeapon(0) @ 0x007177C0`: the Primary slot.
    if ty.primary.is_some() {
        return true;
    }
    if ty.wall {
        let Some((rx, ry)) = map_cell(node.packed_cell) else {
            return false;
        };
        // `0x00481810` over the eight directions, `0x0047C520` on each.
        return crate::util::direction_tables::cell::CELL_DELTAS
            .iter()
            .any(|&(dx, dy)| {
                let neighbor = map_cell_offset(rx, ry, dx, dy);
                neighbor
                    .and_then(|(nx, ny)| {
                        sim.substrate.occupancy.first_building_on_layer(
                            nx,
                            ny,
                            MovementLayer::Ground,
                        )
                    })
                    .and_then(|id| sim.substrate.entities.get(id))
                    .is_some_and(|building| building.owner() == owner)
            });
    }
    if ty.power > 0 {
        return true;
    }
    house
        .strategy_emergency
        .last_building_attack_frame
        .wrapping_add(rules.general.ai_restrict_replace_time)
        <= sim.session.binary_frame as i32
}

fn plan_node(sim: &Simulation, owner: InternedId, index: usize) -> BasePlanNode {
    sim.houses[&owner].base_plan.nodes[index]
}

/// The ordered shift-left removal every caller inlines (`DynamicVector`
/// `Delete` of one 16-byte node).
pub(crate) fn remove_node(sim: &mut Simulation, owner: InternedId, index: usize) {
    if let Some(house) = sim.houses.get_mut(&owner)
        && index < house.base_plan.nodes.len()
    {
        house.base_plan.nodes.remove(index);
    }
}

/// A node cell as a map cell. `MapClass::operator[] @ 0x005657A0` answers
/// its empty dummy for a cell off the map, which holds no object and no
/// overlay; a negative word is always off the map.
fn map_cell(packed_cell: u32) -> Option<(u16, u16)> {
    let (x, y) = unpack_base_plan_cell(packed_cell);
    Some((u16::try_from(x).ok()?, u16::try_from(y).ok()?))
}

fn map_cell_offset(rx: u16, ry: u16, dx: i32, dy: i32) -> Option<(u16, u16)> {
    let x = u16::try_from(i32::from(rx) + dx).ok()?;
    let y = u16::try_from(i32::from(ry) + dy).ok()?;
    Some((x, y))
}

#[cfg(test)]
#[path = "ai_base_building_tests.rs"]
mod tests;
