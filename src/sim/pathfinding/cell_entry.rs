//! Cell entry: the terrain arm, the wall arm and shared Can_Enter_Cell leaves.
//!
//! The original RA2 engine returns 8 distinct codes when a unit
//! tries to enter a cell. Each code triggers a different movement response.
//! The classes' own `Can_Enter_Cell` ports are `Simulation::foot_can_enter`
//! and `aircraft_can_enter` (`world/object_entry.rs`). This module keeps the
//! terrain-only `evaluate_can_enter_cell`, the search's wall arm and the
//! building-entry and crush leaves those ports call.
//!
//! Bridge legality is now driven by A*'s `path_layers` (set per-step by `astar_search`
//! with the Ground→Bridge gates verified against the reference predicate), which
//! approximates the post-switch output of the original two-pass `Can_Enter_Cell`. See
//! docs/plans/2026-05-11-bridge-locomotor-layer-correctness-design.md §"Known Parity Boundary".
//!
//! ## The native shape, and what is not modelled
//!
//! `UnitClass::Can_Enter_Cell` @ `0x0073F0A0` and `InfantryClass::Can_Enter_Cell`
//! @ `0x0051BF90` are the `FootClass` `+0x1AC` slot (`0x007F5E1C` and
//! `0x007EB204`). Both are **accumulators**: a running code later occupants may
//! only raise, punctuated by hard `return 7` / `return 0` exits. `AStar_main_loop`
//! @ `0x00429A90` expands a neighbour iff the code is below 7 and
//! `AStar_compute_edge_cost` @ `0x00429830` indexes it into the float table at
//! `0x0081870C` — `[1.0, 1000.0, 1.0, 1.0, 60.0, 20.0, 8.0, 10000.0]`, whose only
//! reader is `0x00429848`. So codes 3, 4, 5 and 6 all still expand, at 1×, 60×,
//! 20× and 8× the base step.
//!
//! Pre-flight, in order, before any code accumulates: the bridge-deck select from
//! `Cell->Flags & 0x100`; the occupier/occupation snapshot; **Unit only** the
//! `MovementRestrictedTo=` gate; the direction-8 tube endpoint test; the
//! tube-direction consistency tests at this cell and at `(dir-4)&7`; **Infantry
//! only** an unconditional admit when `level - Cell->Level > 4`; the `+0x1B0`
//! slot (`CheckBridgeTraversal` @ `0x004D9C60`, FootClass-level — it is the same
//! entry in both vtables); the deck swap; the playfield gate; and **Unit only**
//! `FootClass::LocomotorPassabilityCheck` @ `0x004D9C10`, whose result **seeds**
//! the running code (Infantry seeds a literal 0).
//!
//! What VERA does not reproduce, recorded rather than guessed and ordered by
//! ordinary-skirmish impact:
//!
//! - **Gate identity corrected (2026-09-22).** `BuildingType+16B7` is `Gate=`:
//!   ReadINI4609EA..460A13 reads literal81AA8C. `4525F0` is gate passability,
//!   requiring Open mission24 and stable-open animation state. Unit73F6E8
//!   skips an open gate; a closed allied gate raises3, armed enemy raises5,
//!   unarmed enemy returns7. The old garrison interpretation was false.
//!   `+16C0` is the separate FirestormWall flag. Complete Unit-call evidence:
//!   `tools/spatial_oracle/unit_entry`. This establishes classification, not
//!   the downstream gate-opening/scatter response for every caller.
//! - **The wall arm produces the wrong code, not no code.** `cell_rect`'s
//!   `is_wall_overlay` / `WallBlocked` path is live and MovementZone-keyed, and
//!   its Destroyer-class escape set matches native's `{2, 3, 8, 0xC}` at
//!   `0x004835BB`. But it answers a **hard block** where native answers **4** for
//!   a friendly wall and **5** for an enemy one (Unit: `OverlayTypeClass+0x2A8`
//!   at `0x0073F420` with the `Crushable=` gate `+0x22D` at `0x0073F42E`;
//!   Infantry: `5 - isAlly`), and 4 and 5 both still expand in the A*. Retail
//!   therefore routes *through* a wall line at 60×/20× cost and stops at it.
//!   Trigger: any expansion into a `Wall=yes` overlay cell. Player
//!   effect: a move order whose destination is enclosed by walls is refused
//!   outright instead of routing to the wall and stopping. Frequency: pre-placed
//!   civilian fences appear on most stock maps, so this fires many times a match
//!   even against players who never build walls. Downstream risk: codes 4 and 5
//!   feed the blocked-step Override arm, whose wall case targets a *cell* rather
//!   than an object and has no Restore path; a producer must land together with
//!   that arm. **Do not add a second wall gate
//!   on top of the existing one.**
//! - **Crushable walls admit crushers and CrusherAll; every other mover is
//!   hard-blocked where native answers 4/5 or 7.** `OverlayTypeClass+0x22D` =
//!   `Crushable=` is parsed (`OverlayTypeFlags::crushable`) and
//!   `overlay_reduced_zone_type` reduces such an overlay to zone class
//!   `CRUSHABLE` (1), as `CellClass::RecalcZoneType` @ `0x00483CB5` does. The
//!   class arm below keys on that class: a `Crusher=` type (`+0xD28`; 29 stock
//!   types, the battle tanks among them) or a `MovementZone=CrusherAll` type
//!   (`[BFRT]`) enters, matching the crusher route of `UnitClass::
//!   Can_Enter_Cell` (`0x0073F42E..F46C`), and `UnitClass::PerCellProcess`
//!   then flattens the wall on arrival (I4, `apply_wall_crush_on_driveover`).
//!   Infantry (`0x0051BF90` has no crusher route) and non-crusher vehicles are
//!   refused, where native's weapon/warhead route answers 4 (allied) or 5
//!   (enemy) for a primary warhead with `Wall=yes` (or `Wood=yes` on a wooden
//!   overlay, Unit only) and 7 otherwise; an allied crushable wall answers 4
//!   even to a crusher; ability 0x11 also takes the crusher route. Those three
//!   are the wall-arm port (the previous entry). The four stock `Crushable=
//!   yes` overlays (`[GASAND]`, `[CAFNCB]`, `[CAFNCP]`, `[CAFNCW]`) are all
//!   `Wall=yes`; a modded crushable non-wall overlay would take this arm where
//!   native applies none. Trigger: a unit with a wall-capable warhead ordered
//!   across a sandbag or fence line, or a crusher crossing its own side's.
//!   Player effect: VERA routes such a unit around the line or refuses where
//!   retail routes through at wall cost and has it shoot; a crusher on an
//!   allied line pays nothing where retail pays 60x. Stock scope of that
//!   gap: the land non-crushers whose primary warhead carries `Wall=yes` are
//!   the IFV (`[FV]`, `HoverMissile` → `HE`) and the Brute (`[BRUTE]`,
//!   `Punch` → `Battering`); every infantry rifle (`M60` → `SA`) and the
//!   other non-crusher vehicles answer 7 natively, which this arm matches.
//!   Frequency: pre-placed fences and sandbags are map dressing on several
//!   stock maps.
//! - **`MovementRestrictedTo=`** (`UnitTypeClass+0xDFC`, Unit only): when set,
//!   the cell's land type must equal it. LandType 10 (Tunnel) is exempt from the
//!   equality test but carries its own rule — `g_IsometricTileTypeClass_Array`
//!   entries with `(+0x2E4, +0x2E8)` of `(5,3)` or `(4,3)` are impassable unless
//!   `bIsoSubTileIndex == 2`, and `(3,4)` or `(3,5)` unless it is `6`. The
//!   overlay window `0xED..=0xEE` escapes the return-7 only when the mover's
//!   level does **not** match the cell's (`0x0073F1FD JNZ`). Trigger: stock
//!   `rulesmd.ini` sets the key on `[HYD]`, `[SQD]`, `[ASW]` and `[HORNET]`,
//!   always `=Water`. Player effect: none observed — the two naval types are
//!   already covered by the water-mover path and the two carrier aircraft use
//!   `AircraftClass`'s own predicate. Frequency: effectively zero for this
//!   owner. Downstream risk: none.
//! - **The tube gates** (`0x0073F211` to the `RET 0x14` at `0x0073F2C6`):
//!   direction 8 — the sentinel
//!   edge `AStar_main_loop` emits as its ninth neighbour — requires a tube at the
//!   cell and then **returns 0 immediately**, skipping the whole rest of the
//!   predicate; Unit tests `tube+0x28 == 0` while **Infantry tests
//!   `tube+0x28 == tube+0x24`**. Separately, any direction whose delta from the
//!   tube's own direction falls in `3..=5` is impassable, tested both at this
//!   cell and at the back-step cell `(dir-4)&7`. Trigger: pathing into or along a
//!   tube cell. Player effect: VERA admits tube-adjacent steps native refuses,
//!   and misses the direction-8 fast admit. Frequency: tube maps only.
//!   Downstream risk: none; it is a leaf predicate.
//! - **The AI-only overlay arm** (`0x0073F3EC`-`0x0073F41D`):
//!   `OverlayTypeClass+0x2AA != 0` and the mover's house not human-controlled
//!   and `g_GameMode == 0` → 7. Infantry has the same arm without the game-mode
//!   term. Trigger: an AI mover in campaign. Player effect: none in skirmish.
//!   Frequency: zero until an AI opponent exists. Downstream risk: none.
//! - **The end-of-list land-row test.** Native checks
//!   `LandTypeSpeedBuildabilityRows[Cell->LandType][speed] == 0.0` **after** the
//!   object walk and only on the ground list; VERA applies it in the terrain
//!   head. Trigger: a bridge-deck step over a zero-row ground cell — water under
//!   a bridge. Player effect: native can still return an object code there;
//!   VERA answers impassable from the head. Frequency: every bridge crossing
//!   over water. Downstream risk: the deck plane suppresses the test in native,
//!   which VERA's `is_elevated_bridge_cell` arm already approximates in
//!   `TerrainCostGrid`, so the observable outcome usually agrees.
//!
//! ## Dependency rules
//! - Part of sim/ — depends on sim/bump_crush, sim/locomotor, sim/pathfinding,
//!   map/entities, map/houses, rules/locomotor_type.

use super::PathGrid;
use super::terrain_cost::TerrainCostGrid;
use crate::map::entities::EntityCategory;
use crate::map::resolved_terrain::{ResolvedTerrainCell, ResolvedTerrainGrid, zone_class};
use crate::rules::locomotor_type::{MovementZone, SpeedType};
use crate::sim::cell_rect::{
    IsClearToMoveResult, LiveCellPassabilityQuery, evaluate_live_cell_passability,
};
use crate::sim::game_entity::GameEntity;
use crate::sim::movement::bump_crush;
use crate::sim::movement::locomotor::MovementLayer;

// ---------------------------------------------------------------------------
// Result enums
// ---------------------------------------------------------------------------

/// Terrain-only result for native-shaped cell-entry checks above `PathGrid`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanEnterCellResult {
    Clear,
    HardBlocked,
    /// The wall arm answered 4 or 5: the mover cannot step here, but the search
    /// may still expand through it at the class's cost multiplier.
    ///
    /// gamemd-derived: `UnitClass::Can_Enter_Cell @ 0x0073F0A0` accumulates
    /// `max(code, 4)` for an allied wall (`0x0073F4EB`) and `max(code, 5)` for a
    /// non-allied one (`0x0073F50E`), after `HouseClass::Is_Ally_ByIndex
    /// @ 0x004F9A10` on the wall owner at `cell+0x50`. `InfantryClass::
    /// Can_Enter_Cell @ 0x0051BF90` computes the same pair as `5 - is_ally`.
    /// `AStar_compute_edge_cost @ 0x00429830` then prices them at 60x and 20x
    /// from the class table at `0x0081870C`.
    WallBlocked {
        cost_class: u8,
    },
}

/// Search-time interpretation of the YR `FootClass` cell predicate result.
///
/// This is deliberately not a terrain-speed percentage. `TerrainCostGrid` remains
/// responsible for SpeedType movement rates; this value is the small native
/// classification `AStar_compute_edge_cost` @ `0x00429830` consumes during A*
/// expansion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchCellCostDecision {
    /// The raw value returned by the per-Foot predicate.
    pub raw_cost_class: u8,
    /// The class supplied to the neighbor-cost routine when expansion continues.
    pub effective_cost_class: Option<u8>,
    /// Whether this neighbor may be expanded.
    pub expands: bool,
    /// Whether the normal edge-cost path is reachable.
    pub should_call_edge_cost: bool,
}

/// Apply the search-only cost-class gate used after YR's `FootClass` +0x1AC call.
///
/// Original: `AStar_main_loop` @ `0x00429A90`, immediately after the `+0x1AC`
/// call — `if (gate && class < 7) class = 0;` then reject `class >= 7`.
///
/// The gate is neither bridge nor coercion: it is `TechnoTypeClass+0xC94`, read
/// at `0x00429B64` and `0x00429C79`, which `TechnoTypeClass::ReadINI` binds at
/// `0x00712284` to the key string at `0x008444BC` = **`IsTrain`**. No stock
/// `rulesmd.ini` entry sets it, so this arm is a correctly-shaped model of a
/// mechanism nothing in stock YR enables — latent, not live.
pub fn search_cell_cost_decision(
    raw_cost_class: u8,
    mover_is_train: bool,
) -> SearchCellCostDecision {
    let effective_cost_class = if mover_is_train && raw_cost_class < 7 {
        0
    } else {
        raw_cost_class
    };
    let expands = effective_cost_class < 7;

    SearchCellCostDecision {
        raw_cost_class,
        effective_cost_class: expands.then_some(effective_cost_class),
        expands,
        should_call_edge_cost: expands,
    }
}

impl CanEnterCellResult {
    pub fn is_clear(self) -> bool {
        matches!(self, Self::Clear)
    }
}

/// Native-shaped known-input context for the terrain/layer portion of cell entry.
///
/// This deliberately stops before the unresolved search-only cost class and the
/// runtime-only blocker response. The original evaluates those with different
/// caller state; only the shared terrain/layer admission belongs here.
#[derive(Debug, Clone, Copy)]
pub struct CanEnterCellContext<'a> {
    pub target: (u16, u16),
    pub terrain_layer: MovementLayer,
    pub movement_zone: Option<MovementZone>,
    pub speed_type: Option<SpeedType>,
    pub path_grid: Option<&'a PathGrid>,
    pub resolved_terrain: Option<&'a ResolvedTerrainGrid>,
    pub terrain_costs: Option<&'a TerrainCostGrid>,
    pub bypass_grid: bool,
    /// Selects the infantry view of terrain-object occupation. Retail terrain
    /// objects occupy sub-cells, and only the infantry entry gate reads that
    /// mask; vehicles stay blocked by the whole cell.
    pub is_infantry: bool,
    /// The mover's `Crusher=` type flag (`UnitTypeClass+0xD28`), the key of
    /// the crusher route of the Unit wall arm (`0x0073F438`). Infantry never
    /// takes that route; pass `false` where the mover is unknown.
    pub mover_is_crusher: bool,
    /// Wall-arm inputs. `None` keeps the coarse pre-I9b answer (a wall is a
    /// hard block), which is what every caller without a resolved mover wants.
    pub wall: Option<WallArmContext<'a>>,
}

/// The map-global tables the wall arm needs, carried on the pathfinding context.
///
/// Split from [`WallArmContext`] deliberately, and the split is the whole point
/// of the seam. These three are map-global — identical for every mover — so they
/// ride on `PathfindingContext` and no caller ever supplies them. The *mover*
/// facts (`owner`, `is_armed`, the two warhead bools) have a different lifetime,
/// one per mover, and are resolved by exactly two authorities: `snapshot_mover`
/// on the tick path and `resolve_move_info` on the order path.
///
/// Ledger row I9c is the counter-example this shape exists to avoid: a mover
/// fact (`mover_is_crusher`) was derived independently at each call site, and the
/// sites lacking context silently passed `false`, so the same unit behaved
/// differently depending on which function issued its move. Tables that no
/// caller passes cannot diverge that way.
///
/// The interner is deliberately **not** here. [`WallArmContext`] is built per
/// mover at the point of use, where an immutable reborrow is already in scope;
/// holding `&StringInterner` on a context that outlives the whole pass would
/// collide with the `&mut` the pass still needs (movement_tick.rs:4023, :4049,
/// :4114, :3855).
#[derive(Clone, Copy)]
pub struct WallArmTables<'a> {
    pub overlay_grid: Option<&'a crate::sim::overlay_grid::OverlayGrid>,
    pub overlay_registry: Option<&'a crate::rules::overlay_types::OverlayTypeRegistry>,
    pub alliances: Option<&'a crate::map::houses::HouseAllianceMap>,
    /// Map-global like the other three: the ally test resolves the wall's owner
    /// name through it before `HouseClass::Is_Ally_ByIndex @ 0x004F9A10`. It
    /// used to be supplied separately at the one runtime site that built a
    /// `WallArmContext`, which left the search site unable to build one at all
    /// without threading a second value; it belongs with the tables.
    pub interner: Option<&'a crate::sim::intern::StringInterner>,
}

/// Everything the wall arm of `Can_Enter_Cell` reads that terrain alone cannot
/// supply: the overlay at the target cell, the house that owns it, and whether
/// the mover can shoot a wall at all.
///
/// gamemd-derived: `UnitClass::Can_Enter_Cell @ 0x0073F0A0` wall arm
/// (`0x0073F3D0..0x0073F51F`) and `InfantryClass::Can_Enter_Cell @ 0x0051BF90`.
#[derive(Clone, Copy)]
pub struct WallArmContext<'a> {
    pub overlay_grid: Option<&'a crate::sim::overlay_grid::OverlayGrid>,
    pub overlay_registry: Option<&'a crate::rules::overlay_types::OverlayTypeRegistry>,
    pub alliances: Option<&'a crate::map::houses::HouseAllianceMap>,
    pub interner: Option<&'a crate::sim::intern::StringInterner>,
    /// The mover's owning house, compared with the wall's owner through
    /// `HouseClass::Is_Ally_ByIndex @ 0x004F9A10`.
    pub mover_owner: Option<crate::sim::intern::InternedId>,
    /// `TechnoClass::Is_Armed @ 0x00701120` (vtable `+0x2AC`, primary weapon
    /// slot non-null). False answers 7 at `0x0073F48F`.
    pub is_armed: bool,
    /// Primary warhead `Wall=` (`WarheadTypeClass+0x144`, tested at
    /// `0x0073F4A9`).
    pub warhead_wall: bool,
    /// Primary warhead `Wood=` (`+0x147`, tested at `0x0073F4B3`), which only
    /// admits an overlay whose `Armor=` is wood (`0x0073F4BD` compares 6).
    pub warhead_wood: bool,
}

// `OverlayTypeRegistry` carries no `Debug`, and adding one there would touch a
// rules type for a pathfinding convenience. `CanEnterCellContext` derives
// `Debug`, so this prints the mover-side facts and elides the borrowed tables.
impl std::fmt::Debug for WallArmContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WallArmContext")
            .field("mover_owner", &self.mover_owner)
            .field("is_armed", &self.is_armed)
            .field("warhead_wall", &self.warhead_wall)
            .field("warhead_wood", &self.warhead_wood)
            .field("overlay_grid", &self.overlay_grid.is_some())
            .field("overlay_registry", &self.overlay_registry.is_some())
            .field("alliances", &self.alliances.is_some())
            .finish()
    }
}

impl WallArmContext<'_> {
    /// The wall overlay at `cell`, as `(is_wall, armor_is_wood, owner)`.
    fn overlay_at(
        &self,
        cell: (u16, u16),
    ) -> Option<(bool, bool, Option<crate::sim::intern::InternedId>)> {
        let grid = self.overlay_grid?;
        let registry = self.overlay_registry?;
        let overlay = grid.cell(cell.0, cell.1);
        let flags = overlay.overlay_id.and_then(|id| registry.flags(id))?;
        Some((flags.wall, flags.armor_is_wood, overlay.wall_owner))
    }

    /// `HouseClass::Is_Ally_ByIndex @ 0x004F9A10`: true for the mover's own
    /// house, false for an unowned wall (index -1), else the ally bitfield.
    fn owner_is_ally(&self, wall_owner: Option<crate::sim::intern::InternedId>) -> bool {
        let (Some(alliances), Some(interner), Some(mover), Some(wall)) =
            (self.alliances, self.interner, self.mover_owner, wall_owner)
        else {
            return false;
        };
        // RESIDUAL (ledger I9b): `is_allied_with` normalizes both names, i.e. two
        // String allocations per query, and this runs once per refused neighbour
        // inside the A* loop. Pre-existing, not introduced here. An id-keyed
        // lookup is the right fix but must be built once per *frame* and cached
        // (`MovementPassCache`) — a first attempt rebuilt it per object per frame,
        // which is strictly worse, and silently dropped alliance rows for house
        // names carrying whitespace because the interner does not trim.
        crate::map::houses::is_allied_with(
            alliances,
            interner.resolve(mover),
            interner.resolve(wall),
        )
    }

    /// The accumulated wall code, or `None` when the arm answers 7.
    ///
    /// `0x0073F483..0x0073F51F`: an unarmed mover exits through the shared
    /// epilogue at `0x0073FCD0` (branch `JZ` at `0x0073F48F`); one whose primary
    /// warhead is neither `Wall=` nor `Wood=`-against-wood returns 7 at its own
    /// exit, `0x0073F4C9`. Otherwise the ally test picks `max(code, 4)` or
    /// `max(code, 5)`.
    ///
    /// `is_infantry` gates the `Wood=` clause, which is **Unit-only**. The
    /// infantry arm (`InfantryClass::Can_Enter_Cell 0x0051BF90`) resolves its
    /// warhead through `FUN_00772AC0`, whose whole body is one test of
    /// `+0x144` (`Wall=`) — no `+0x147`, no `Armor == 6` compare — and then
    /// takes `5 - Is_Ally_ByIndex`. Stock-reachable: `[SHK]` (Primary
    /// `ElectricBolt` -> warhead `[Shock]`, `Wood=yes` with no `Wall=`) against
    /// `[CAKRMW]` (`Armor=wood`, `Crushable=no`).
    fn weapon_route_code(
        &self,
        armor_is_wood: bool,
        wall_owner: Option<crate::sim::intern::InternedId>,
        is_infantry: bool,
    ) -> Option<u8> {
        if !self.is_armed {
            return None;
        }
        let wood_route = !is_infantry && self.warhead_wood && armor_is_wood;
        if !(self.warhead_wall || wood_route) {
            return None;
        }
        // Fail closed on a wiring gap rather than guessing "enemy". Native
        // always has a house to ask, so absent alliance tables here are a VERA
        // wiring mistake, not a game state — and answering 5 would be
        // indistinguishable from a genuinely unowned wall while quietly pricing
        // it at 20x. Declining keeps the pre-I9b hard block, which is the same
        // "absent context => pre-I9b behaviour" rule the rest of the arm
        // follows, and it matters because the producer has to reach many call
        // sites: a site wired without tables then refuses walls instead of
        // silently mis-pricing them. An unowned wall (`wall_owner: None` with
        // the tables present) still takes 5 — native's index -1.
        if self.alliances.is_none() || self.interner.is_none() || self.mover_owner.is_none() {
            return None;
        }
        Some(if self.owner_is_ally(wall_owner) { 4 } else { 5 })
    }
}

/// Prices a candidate cell for A* expansion through the wall arm.
///
/// gamemd-derived: `AStar_main_loop @ 0x00429A90` calls the Foot `+0x1AC` slot
/// for each neighbour and hands the returned code to
/// `AStar_compute_edge_cost @ 0x00429830`, which indexes the class base table
/// at `0x0081870C` — `[1.0, 1000.0, 1.0, 1.0, 60.0, 20.0, 8.0, 10000.0]`, so an
/// allied wall (4) expands at 60x and an enemy one (5) at 20x. A code of 7 stops
/// the expansion in `search_cell_cost_decision`.
///
/// This is the production producer for the `search_cost_classifier` seam: it
/// carries the mover facts the pure pathfinding layer cannot resolve on its own.
pub struct WallSearchCostClassifier<'a> {
    pub wall: WallArmContext<'a>,
    pub path_grid: Option<&'a PathGrid>,
    pub resolved_terrain: Option<&'a ResolvedTerrainGrid>,
    pub terrain_costs: Option<&'a TerrainCostGrid>,
    pub movement_zone: Option<MovementZone>,
    pub speed_type: Option<SpeedType>,
    pub is_infantry: bool,
    pub mover_is_crusher: bool,
}

impl crate::sim::pathfinding::SearchCellCostClassifier for WallSearchCostClassifier<'_> {
    fn classify(&self, _from: (u16, u16), candidate: (u16, u16), bridge: bool) -> u8 {
        let terrain_layer = if bridge {
            MovementLayer::Bridge
        } else {
            MovementLayer::Ground
        };
        match evaluate_can_enter_cell(CanEnterCellContext {
            wall: Some(self.wall),
            target: candidate,
            terrain_layer,
            movement_zone: self.movement_zone,
            speed_type: self.speed_type,
            path_grid: self.path_grid,
            resolved_terrain: self.resolved_terrain,
            terrain_costs: self.terrain_costs,
            bypass_grid: false,
            is_infantry: self.is_infantry,
            mover_is_crusher: self.mover_is_crusher,
        }) {
            // `Clear` is NOT class 0 here. `astar_search` consults this
            // classifier only after its own `neighbor_passable` has already
            // refused the cell, and that check carries terms this cell-scoped
            // evaluation cannot see — the ground/bridge layer split, and the
            // `neighbor_cell.transition` (`0x200`) gate a ground->bridge entry
            // must pass. Answering 0 would re-admit a refused neighbour at 1x:
            // a passable deck whose `transition` is clear would be entered
            // anyway. Only a wall this mover may shoot changes the outcome.
            CanEnterCellResult::Clear => 7,
            CanEnterCellResult::WallBlocked { cost_class } => cost_class,
            CanEnterCellResult::HardBlocked => 7,
        }
    }
}

/// The Foot `+0x1AC` cost class for a slave: `InfantryClass::Can_Enter_Cell`
/// (`0x0051C29E..0x0051C2CA`) clears the master Building's occupation on the
/// slave's own deposit Cells (`SlaveManagerClass 0x006B0880`), so
/// `AStar_main_loop @ 0x00429A90` can end a path inside the refinery. The
/// ground Cell's terrain still decides; every other Cell goes to `inner`.
pub struct SlaveDepositSearchClassifier<'a> {
    pub cells: [Option<(u16, u16)>; 2],
    pub inner: Option<&'a dyn crate::sim::pathfinding::SearchCellCostClassifier>,
    pub path_grid: Option<&'a PathGrid>,
    pub resolved_terrain: Option<&'a ResolvedTerrainGrid>,
    pub terrain_costs: Option<&'a TerrainCostGrid>,
    pub movement_zone: Option<MovementZone>,
    pub speed_type: Option<SpeedType>,
}

impl crate::sim::pathfinding::SearchCellCostClassifier for SlaveDepositSearchClassifier<'_> {
    fn classify(&self, from: (u16, u16), candidate: (u16, u16), bridge: bool) -> u8 {
        if !bridge && self.cells.contains(&Some(candidate)) {
            return match evaluate_can_enter_cell(CanEnterCellContext {
                wall: None,
                target: candidate,
                terrain_layer: MovementLayer::Ground,
                movement_zone: self.movement_zone,
                speed_type: self.speed_type,
                path_grid: self.path_grid,
                resolved_terrain: self.resolved_terrain,
                terrain_costs: self.terrain_costs,
                bypass_grid: true,
                is_infantry: true,
                mover_is_crusher: false,
            }) {
                CanEnterCellResult::Clear => 0,
                CanEnterCellResult::WallBlocked { cost_class } => cost_class,
                CanEnterCellResult::HardBlocked => 7,
            };
        }
        self.inner
            .map_or(7, |inner| inner.classify(from, candidate, bridge))
    }
}

/// Evaluate the shared terrain/layer slice of Can_Enter_Cell.
///
/// `PathGrid` is a coarse structural filter. Final terrain legality must also
/// consult the mover's SpeedType against the resolved target LandType/speed row
/// so a PathGrid-walkable water cell is still illegal for ordinary ground movers.
// Original: the `FootClass` `+0x1AC` slot — `UnitClass::Can_Enter_Cell` @
// `0x0073F0A0` (`0x007F5E1C`) and `InfantryClass::Can_Enter_Cell` @
// `0x0051BF90` (`0x007EB204`). There is no `EvaluateCellEnterabilityOrCost`
// symbol in this program; the earlier name here was invented.
pub fn evaluate_can_enter_cell(ctx: CanEnterCellContext<'_>) -> CanEnterCellResult {
    match ctx.terrain_layer {
        MovementLayer::Ground => evaluate_ground_cell_entry(ctx),
        MovementLayer::Bridge => {
            let bridge_walkable = ctx.path_grid.is_some_and(|grid| {
                grid.is_walkable_on_layer(ctx.target.0, ctx.target.1, MovementLayer::Bridge)
            });
            // The deck branch never reaches the land row: `0x0073FA92` tests the
            // deck flag and jumps past the read (`JNZ 0x0073FC24`), so the row
            // cannot refuse a bridge-layer entry.
            evaluate_shared_cell_leaf(ctx, bridge_walkable, true)
        }
        // Air and underground locomotors are admitted by their dedicated
        // locomotion state machines, not this ground/bridge terrain slice.
        MovementLayer::Air | MovementLayer::Underground => CanEnterCellResult::Clear,
    }
}

fn evaluate_ground_cell_entry(ctx: CanEnterCellContext<'_>) -> CanEnterCellResult {
    let (x, y) = ctx.target;

    if let Some(movement_zone) = ctx.movement_zone.filter(|zone| zone.is_water_mover()) {
        let land_passable = ctx
            .resolved_terrain
            .and_then(|terrain| terrain.cell(x, y))
            .is_some_and(|cell| is_water_surface_cell_passable(cell, movement_zone));
        // A water mover's surface test above already stands in for the row on
        // this VERA-internal branch; native has one path and would read the row
        // here too. UNCHECKED, and inert in practice (walls are not placed on
        // open water), so the row is not made to refuse anything extra.
        return evaluate_shared_cell_leaf(ctx, land_passable, true);
    }

    let grid_ok = ctx.path_grid.map_or(true, |grid| {
        ctx.bypass_grid
            || if ctx.is_infantry {
                grid.is_walkable_for_infantry(x, y)
            } else {
                grid.is_walkable(x, y)
            }
    });
    let speed_type = ctx.speed_type.or_else(|| {
        if ctx.terrain_costs.is_some() && !ctx.is_infantry {
            None
        } else {
            // InfantryClass::Can_Enter_Cell @ 0x0051C750 reads the ground
            // LandType/SpeedType row even beneath an intact bridge. The coarse
            // cost grid also represents the deck and can contain 100 over a
            // ground Foot row of zero, so it cannot replace this input.
            ctx.movement_zone.map(|zone| zone.speed_type())
        }
    });
    let speed_passable = speed_type.is_none_or(|speed_type| {
        ctx.resolved_terrain
            .and_then(|terrain| terrain.cell(x, y))
            .is_none_or(|cell| speed_type_allows_cell(cell, speed_type))
    });
    // A mover on the ground plane of a stamped cell reads the land row of the
    // terrain itself, not the deck's override: `UnitClass::Can_Enter_Cell`
    // @ `0x0073F0A0` clears its deck flag for a path height within one of the
    // cell's signed level (`0x0073F0B7..F0E8`), walks the ground list `+0xE4`
    // (`0x0073F51A`) and then tests the row at `0x0073FAB5` only on that
    // branch (`0x0073FA92`); Infantry does the same at `0x0051C750`. The planner's cost
    // grid carries the deck answer on those cells, so consult its ground row.
    let terrain_cost_passable = match ctx.terrain_costs {
        Some(costs) if target_has_structural_bridge(ctx) => costs.ground_cost_at(x, y) != 0,
        Some(costs) => costs.cost_at(x, y) != 0,
        None => true,
    };

    evaluate_shared_cell_leaf(
        ctx,
        grid_ok && speed_passable && terrain_cost_passable,
        speed_passable,
    )
}

/// Whether the target carries the native `CellClass+0x140 & 0x100` stamp.
fn target_has_structural_bridge(ctx: CanEnterCellContext<'_>) -> bool {
    ctx.path_grid
        .and_then(|grid| grid.cell(ctx.target.0, ctx.target.1))
        .is_some_and(|cell| cell.has_structural_bridge())
        || ctx
            .resolved_terrain
            .and_then(|terrain| terrain.cell(ctx.target.0, ctx.target.1))
            .is_some_and(|cell| cell.bridge_facts.has_structural_bridge())
}

/// The shared tail of both `Can_Enter_Cell` implementations.
///
/// `land_passable` is VERA's coarse "may this mover stand here at all" answer:
/// the path grid, the speed row and the cost grid folded together.
///
/// `land_row_passable` is the cell's **land row** — `speed_type_allows_cell`,
/// the analogue of native's `FLD [ECX*4 + 0x89EA40]` / `FCOMP 0.0` at
/// `0x0073FAB5`.
///
/// It is a separate parameter because the wall arm needs the row without the
/// two *blocking* terms the coarse answer folds in: `grid_ok` and the cost grid
/// both go false on `overlay_blocks`, which `ResolvedTerrainGrid` sets for every
/// `zone_class::WALL` overlay, so gating the wall classes on `land_passable`
/// would refuse the very cells the arm exists to price.
///
/// The row itself is *not* overlay-free, and the distinction matters:
/// `apply_overlay_land` writes `cell.speed_costs` from the overlay's own `Land=`
/// row, exactly as native does — `CellClass::RecalcAttributes @ 0x0047D2B0`
/// opens with `this->LandType = ot->Land` (`+0x298`) and early-returns on
/// `Land == 4`/`9` or `NoUseTileLandType` (`+0x2AC`), which
/// `uses_early_recalc_land_branch` ports. So reading the post-overlay row is the
/// faithful analogue, not an accident.
///
/// In stock data this gate never fires: no `Wall=yes` overlay declares `Land=`,
/// so each inherits `LandType::Clear` and its passable row rather than the
/// all-zero `[Wall]` row. It bites only where an overlay declares a land whose
/// row is zero for the mover's SpeedType.
fn evaluate_shared_cell_leaf(
    ctx: CanEnterCellContext<'_>,
    land_passable: bool,
    land_row_passable: bool,
) -> CanEnterCellResult {
    let structural_bridge = target_has_structural_bridge(ctx);
    let bridge_transition = ctx
        .path_grid
        .and_then(|grid| grid.cell(ctx.target.0, ctx.target.1))
        .is_some_and(|cell| cell.is_bridge_transition_cell())
        || ctx
            .resolved_terrain
            .and_then(|terrain| terrain.cell(ctx.target.0, ctx.target.1))
            .is_some_and(|cell| cell.is_bridge_transition_cell());
    if bridge_transition || (ctx.terrain_layer == MovementLayer::Bridge && !structural_bridge) {
        // Native `IsClearToMove` receives an integer level, not the engine's
        // path-layer enum. A bridgehead can carry Ground while an already-on-
        // bridge mover remains at deck height, so guessing base/base+4 here
        // rejects the proved Body->Ramp->Ground transition. Until +0x1AC
        // threads its numeric path height, retain the prior structural gate.
        return if land_passable {
            CanEnterCellResult::Clear
        } else {
            CanEnterCellResult::HardBlocked
        };
    }

    let Some(speed_type) = ctx
        .speed_type
        .or_else(|| ctx.movement_zone.map(|zone| zone.speed_type()))
    else {
        return if land_passable {
            CanEnterCellResult::Clear
        } else {
            CanEnterCellResult::HardBlocked
        };
    };
    let movement_zone = ctx.movement_zone.unwrap_or(MovementZone::Normal);
    if ctx.terrain_layer == MovementLayer::Ground && speed_type != SpeedType::Winged {
        // Neither Foot +0x1AC implementation calls Cell::CheckCellPassability
        // @ 0x004834A0 — its callers are CellRect::CheckPassability, threat
        // scans, placement, paradrop, overlay Mark and Jumpjet touchdown, none of
        // them a Foot entry test. Infantry +0x1AC @ 0x0051BF90 and Unit +0x1AC
        // @ 0x0073F0A0 (UnitClass vtable 0x007F5C70 + 0x1AC = 0x007F5E1C) both
        // defer numeric height legality to the shared +0x1B0 traversal @
        // 0x004D9C60, whose equal-level arm admits base-height ground beneath a
        // span; ground near the candidate's level then selects the ground list
        // (+0xE4/+0x124) even when the cell carries the 0x100 stamp. A* @
        // 0x00429F54, Walk @ 0x0075B690 and the Drive crossing admission reach
        // this class contract through the virtual slot. Keep the existing
        // coarse wall result here (the native 4/5 wall accumulator is a separate
        // recorded gap), without importing the unrelated Cell leaf's level
        // rejection. See RAMP_UNIT_HEIGHT_GHIDRA_REPORT.md, under-span admission.
        let terrain_cell = ctx
            .resolved_terrain
            .and_then(|terrain| terrain.cell(ctx.target.0, ctx.target.1));
        if terrain_cell.is_none()
            && ctx
                .path_grid
                .and_then(|grid| grid.cell(ctx.target.0, ctx.target.1))
                .is_none()
        {
            // Retain the prior live-adapter missing-target rejection, including
            // bypass_grid callers. This does not model native dummy-cell access.
            return CanEnterCellResult::HardBlocked;
        }
        let wall = terrain_cell.is_some_and(|cell| cell.zone_type == zone_class::WALL);
        let wall_cleared = wall
            && matches!(
                movement_zone,
                MovementZone::Destroyer
                    | MovementZone::AmphibiousDestroyer
                    | MovementZone::InfantryDestroyer
                    | MovementZone::CrusherAll
            );
        // A `Crushable=` overlay reduces to zone class 1 in `RecalcZoneType`
        // (`overlay_reduced_zone_type`), never to `WALL`, so the wall test
        // above cannot see a sandbag or fence. The four stock `Crushable=yes`
        // overlays (`[GASAND]`, `[CAFNCB]`, `[CAFNCP]`, `[CAFNCW]`) are all
        // `Wall=yes`, so class 1 stands for "crushable wall" here; a modded
        // crushable non-wall overlay would take this arm where native applies
        // none. Native `UnitClass::Can_Enter_Cell` wall arm `0x0073F42E..F46E`:
        // `Crushable=` (+0x22D) with the type's `Crusher=` (+0xD28) or ability
        // 0x11 enters (code unchanged when the wall is not allied, 4 when it
        // is); `0x0073F455..F46C`: `MovementZone=CrusherAll` (+0x5B4 == 0xC)
        // enters any `Wall=`; everything else, and every infantryman
        // (`0x0051BF90` has no crusher route), takes the weapon/warhead route
        // that answers 4/5 or 7. The runtime crossing produces 4/5 since
        // 2026-09-16; the A* search does only for Walk orders
        // (`walk_path.rs` builds the classifier; the order path and the Drive
        // tick contexts leave it off, ledger I9b), so elsewhere that route is
        // the hard block below; the allied-wall 4 and ability 0x11 are
        // likewise unmodelled.
        let crushable_wall =
            terrain_cell.is_some_and(|cell| cell.zone_type == zone_class::CRUSHABLE);
        let crushable_wall_admitted =
            !ctx.is_infantry && (ctx.mover_is_crusher || movement_zone == MovementZone::CrusherAll);

        // The wall arm's weapon route (`0x0073F483..0x0073F51F`): an armed mover
        // whose primary warhead is `Wall=`, or `Wood=` against an `Armor=wood`
        // overlay (Unit only — the infantry arm's `FUN_00772AC0` tests `Wall=`
        // alone), accumulates 4 against an allied wall and 5 against any other
        // — an unowned wall is index -1, which `Is_Ally_ByIndex` rejects, so it
        // takes 5. An unarmed mover leaves through the shared epilogue at
        // `0x0073FCD0`; a warhead miss returns 7 at `0x0073F4C9`.
        //
        // With `ctx.wall` absent this stays `None` and the coarse pre-I9b hard
        // block is returned unchanged, which is what every caller that has not
        // resolved a mover wants.
        //
        // Residual: the crusher route's allied-wall `max(code, 4)` (`0x0073F481`
        // jumps into the same accumulator at `0x0073F4EB`) is not modelled, so a
        // crusher still enters a crushable wall freely whoever owns it. Changing
        // that also moves I4's wall-crush admission, so it is recorded rather
        // than folded in here.
        let wall_attack_code: Option<u8> = ctx.wall.and_then(|wall_ctx| {
            let (is_wall, armor_is_wood, owner) = wall_ctx.overlay_at(ctx.target)?;
            if !is_wall {
                return None;
            }
            wall_ctx.weapon_route_code(armor_is_wood, owner, ctx.is_infantry)
        });

        // The wall arm does NOT return in native. It accumulates 4/5 into the
        // running code, falls through the occupant walk, and then reads the
        // ground land row at `0x0073FAB5` (`FLD [ECX*4 + 0x89EA40]`, `FCOMP
        // 0.0`); a zero row returns 7 at `0x0073FAD0` whatever the arm
        // accumulated, and `InfantryClass` does the same at `0x0051C7D0`. So a
        // wall overlay on terrain whose speed row refuses this mover answers 7,
        // not 4/5 - the classes survive only where the LAND ROW admits, which
        // is not the same as the terrain beneath. Review corrected this:
        // `OverlayTypeFlags::no_use_tile_land_type` defaults **true**, so a
        // stock wall overlay writes its own `Clear` row into the cell and a
        // wall over water or rock reads as passable here. Native reads the
        // same stored LandType at `0x0073FAB5`, so this is plausibly parity
        // rather than a defect, and stock maps do not place walls on water -
        // but what the code checks is the stored row, not the ground.
        //
        // `land_row_passable` is that row alone, deliberately not
        // `land_passable`: see this function's doc for why the wider term would
        // refuse every wall and leave the arm dead.
        if crushable_wall && !crushable_wall_admitted {
            return match wall_attack_code.filter(|_| land_row_passable) {
                Some(cost_class) => CanEnterCellResult::WallBlocked { cost_class },
                None => CanEnterCellResult::HardBlocked,
            };
        }
        return if !wall_cleared && (wall || !land_passable) {
            match wall_attack_code.filter(|_| wall && land_row_passable) {
                Some(cost_class) => CanEnterCellResult::WallBlocked { cost_class },
                None => CanEnterCellResult::HardBlocked,
            }
        } else {
            CanEnterCellResult::Clear
        };
    }
    let result = evaluate_live_cell_passability(LiveCellPassabilityQuery {
        target: ctx.target,
        speed_type,
        movement_zone,
        // FootClass +0x1AC owns zone calculation outside the shared Cell leaf.
        requested_zone: None,
        actual_zone: 0,
        requested_layer: Some(ctx.terrain_layer),
        ignore_infantry: false,
        ignore_vehicles: false,
        land_passable,
        path_grid: ctx.path_grid,
        resolved_terrain: ctx.resolved_terrain,
        // Object-list and raw occupation classification remain the later
        // class-specific +0x1AC arms and must not be collapsed into terrain.
        raw_occupation: None,
    });
    if matches!(
        result,
        IsClearToMoveResult::Clear { .. } | IsClearToMoveResult::ClearWinged
    ) {
        CanEnterCellResult::Clear
    } else {
        CanEnterCellResult::HardBlocked
    }
}

/// Ship movement must use the reduced ZoneType matrix rather than PathGrid's
/// ground walkability, with the confirmed coastal-water compatibility fallback.
pub(crate) fn is_water_surface_cell_passable(
    cell: &ResolvedTerrainCell,
    movement_zone: MovementZone,
) -> bool {
    let matrix_ok = super::passability::is_passable_for_zone(cell.zone_type, movement_zone);
    if matrix_ok || cell.is_water {
        return true;
    }
    movement_zone == MovementZone::WaterBeach && cell.zone_type == zone_class::BEACH
}

/// The cell's land row (`Ground[LandType]`, `0x0089EA40`) for `speed_type`:
/// false where it gives no speed (`FCOMP 0.0`); an unbound row admits.
pub(crate) fn speed_type_allows_cell(cell: &ResolvedTerrainCell, speed_type: SpeedType) -> bool {
    cell.speed_costs
        .cost_for_speed_type(speed_type)
        .is_none_or(|cost| cost > 0)
}

/// Layer selections used by Can_Enter_Cell-style checks.
///
/// The common case uses one layer for all phases. Bridge traversal may select
/// the bridge object list while the post-traversal occupancy bits remain ground.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanEnterLayerContext {
    pub terrain_layer: MovementLayer,
    pub object_list_layer: MovementLayer,
    pub occupancy_bits_layer: MovementLayer,
}

impl CanEnterLayerContext {
    pub fn single(layer: MovementLayer) -> Self {
        Self {
            terrain_layer: layer,
            object_list_layer: layer,
            occupancy_bits_layer: layer,
        }
    }
}

/// Vehicle-only building entry branch that may reach the live row helper.
///
/// InfantryClass::Can_Enter_Cell does not use the radio/contact or
/// UnitRepair/Bunker NumberImpassableRows branches, so callers must not use this
/// as a shared infantry rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VehicleBuildingEntryBranch {
    /// Contact-vector branch. The caller must supply whether this mover has
    /// RadioClass contact with the checked building.
    RadioContact { mover_has_contact: bool },
    /// UnitRepair/Bunker branch. This branch is gated by the checked building's
    /// type flags, not by RadioClass contact.
    UnitRepairOrBunker,
}

/// Decision for a checked building occupant in UnitClass-style cell entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildingOccupantEntryDecision {
    /// Keep the checked building in the ordinary blocker classification path.
    KeepBlocker,
    /// Skip this building occupant and continue scanning later occupants in the
    /// cell's object list.
    SkipBlocker,
}

/// Explicit live facts needed by the UnitClass building row-helper decision.
///
/// Caller responsibilities:
/// - `candidate_building_id` must be the result of a live
///   Look_up_building_in_cell-style lookup for the candidate cell.
/// - `checked_building_id` and type/runtime flags must describe the building
///   occupant currently being inspected.
/// - `mover_category` must be the mover's semantic category; only UnitClass-style
///   vehicle movers use these exceptions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveVehicleBuildingEntry {
    pub mover_category: EntityCategory,
    pub branch: VehicleBuildingEntryBranch,
    pub checked_building_id: u64,
    pub candidate_building_id: Option<u64>,
    pub candidate_x: u16,
    pub building_origin_x: u16,
    pub number_impassable_rows: i32,
    pub is_unit_repair: bool,
    pub is_bunker: bool,
    pub bunker_occupied: bool,
}

/// Decide whether UnitClass-style movement should skip a building occupant.
///
/// This models `FUN_00458A00` at its two UnitClass::Can_Enter_Cell callsites:
/// radio/contact and UnitRepair/Bunker. A `KeepBlocker` result means the caller
/// should continue with the existing Can_Enter_Cell return-code classification;
/// `SkipBlocker` means only this building occupant is ignored.
pub fn decide_live_vehicle_building_entry(
    input: LiveVehicleBuildingEntry,
) -> BuildingOccupantEntryDecision {
    if input.mover_category != EntityCategory::Unit {
        return BuildingOccupantEntryDecision::KeepBlocker;
    }

    let branch_active = match input.branch {
        VehicleBuildingEntryBranch::RadioContact { mover_has_contact } => mover_has_contact,
        VehicleBuildingEntryBranch::UnitRepairOrBunker => input.is_unit_repair || input.is_bunker,
    };
    if !branch_active {
        return BuildingOccupantEntryDecision::KeepBlocker;
    }

    if input.candidate_building_id != Some(input.checked_building_id) {
        //458A00 returns false on identity mismatch. The radio caller73F5A2
        //skips then; UnitRepair/Bunker73F761 has a separate equality gate.
        return match input.branch {
            VehicleBuildingEntryBranch::RadioContact { .. } => {
                BuildingOccupantEntryDecision::SkipBlocker
            }
            VehicleBuildingEntryBranch::UnitRepairOrBunker => {
                BuildingOccupantEntryDecision::KeepBlocker
            }
        };
    }
    if input.number_impassable_rows == -1 {
        return BuildingOccupantEntryDecision::KeepBlocker;
    }
    if input.is_bunker && input.bunker_occupied {
        return BuildingOccupantEntryDecision::KeepBlocker;
    }

    let first_clear_x = i32::from(input.building_origin_x) + input.number_impassable_rows;
    if i32::from(input.candidate_x) >= first_clear_x {
        BuildingOccupantEntryDecision::SkipBlocker
    } else {
        BuildingOccupantEntryDecision::KeepBlocker
    }
}

/// The Unit-target subset of Object::IsCrushableBy5F6CD0, reached by the
/// post-latch GetUnit(false) exception at73FD17. The caller has already supplied
/// a latch; this leaf does not retest Crusher/ability or apply kills. Both native
/// arms test mover-house alliance and the target's +160 invulnerability slot.
/// A rejected Omni arm falls through to ordinary Crushable, which does not
/// read OmniCrushResistant or the mover's regular-crusher flag.
pub(crate) fn unit_tail_is_crushable_by(
    unit: &GameEntity,
    capability: bump_crush::CrushCapability,
    mover_considers_target_allied: bool,
    current_frame: u32,
) -> bool {
    debug_assert_eq!(unit.category, EntityCategory::Unit);
    let target = bump_crush::CrushTarget::from_entity(unit, current_frame);
    // Live Unit entities carry Techno abstract flag1. Deploy crush immunity
    // is written only by Infantry deploy; it is always clear on this subset.
    !mover_considers_target_allied
        && bump_crush::object_is_crushable_by(capability.omni_crusher, target)
}

/// An object's native coordinate triple in leptons: cell origin plus sub-cell
/// offset, and the exact Z when the mover retains one, else the level height.
pub(crate) fn entity_world_leptons(entity: &GameEntity) -> [i32; 3] {
    let coord = crate::sim::movement::ground_pose::position_world_coord(&entity.position);
    [coord.x, coord.y, coord.z]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::houses::HouseAllianceMap;

    fn crushable_wall_grid() -> ResolvedTerrainGrid {
        let mut cells = Vec::with_capacity(9);
        for ry in 0..3u16 {
            for rx in 0..3u16 {
                let mut cell = ResolvedTerrainCell::clear_for_test(rx, ry);
                if (rx, ry) == (1, 1) {
                    // `[GASAND]`: Wall=yes + Crushable=yes -> RecalcZoneType class 1.
                    cell.overlay_zone_type = Some(zone_class::CRUSHABLE);
                    cell.zone_type = zone_class::CRUSHABLE;
                }
                cells.push(cell);
            }
        }
        ResolvedTerrainGrid::from_cells(3, 3, cells)
    }

    fn crushable_wall_entry(
        terrain: &ResolvedTerrainGrid,
        grid: &PathGrid,
        movement_zone: MovementZone,
        is_infantry: bool,
        mover_is_crusher: bool,
    ) -> CanEnterCellResult {
        evaluate_can_enter_cell(CanEnterCellContext {
            wall: None,
            target: (1, 1),
            terrain_layer: MovementLayer::Ground,
            movement_zone: Some(movement_zone),
            speed_type: None,
            path_grid: Some(grid),
            resolved_terrain: Some(terrain),
            terrain_costs: None,
            bypass_grid: false,
            is_infantry,
            mover_is_crusher,
        })
    }

    /// `UnitClass::Can_Enter_Cell 0x0073F42E..F46C`: a `Crushable=` wall admits a
    /// `Crusher=` type or `MovementZone=CrusherAll`; `InfantryClass 0x0051BF90`
    /// has no crusher route. Every other mover is refused here where native's
    /// weapon route answers 4/5/7 (recorded gap). Native established from the
    /// bodies; this is a Rust regression check over the zone-class-1 fixture.
    #[test]
    fn crushable_wall_admits_only_crushers_and_crusher_all() {
        let terrain = crushable_wall_grid();
        let grid = PathGrid::from_resolved_terrain(&terrain);
        assert!(
            grid.is_walkable(1, 1),
            "class 1 does not block the path grid"
        );
        let cases = [
            (
                MovementZone::Normal,
                false,
                false,
                CanEnterCellResult::HardBlocked,
            ),
            (MovementZone::Normal, false, true, CanEnterCellResult::Clear),
            (
                MovementZone::Crusher,
                false,
                true,
                CanEnterCellResult::Clear,
            ),
            (
                MovementZone::CrusherAll,
                false,
                false,
                CanEnterCellResult::Clear,
            ),
            (
                MovementZone::Infantry,
                true,
                false,
                CanEnterCellResult::HardBlocked,
            ),
            // An infantryman never takes the crusher route, whatever its type says.
            (
                MovementZone::Infantry,
                true,
                true,
                CanEnterCellResult::HardBlocked,
            ),
            // Nor does an infantryman take the CrusherAll route.
            (
                MovementZone::CrusherAll,
                true,
                false,
                CanEnterCellResult::HardBlocked,
            ),
            (
                MovementZone::Destroyer,
                false,
                false,
                CanEnterCellResult::HardBlocked,
            ),
        ];
        for (zone, infantry, crusher, expected) in cases {
            assert_eq!(
                crushable_wall_entry(&terrain, &grid, zone, infantry, crusher),
                expected,
                "zone {zone:?} infantry {infantry} crusher {crusher}"
            );
        }
        // The clear neighbour is untouched by the arm.
        let clear = evaluate_can_enter_cell(CanEnterCellContext {
            wall: None,
            target: (0, 0),
            terrain_layer: MovementLayer::Ground,
            movement_zone: Some(MovementZone::Normal),
            speed_type: None,
            path_grid: Some(&grid),
            resolved_terrain: Some(&terrain),
            terrain_costs: None,
            bypass_grid: false,
            is_infantry: false,
            mover_is_crusher: false,
        });
        assert_eq!(clear, CanEnterCellResult::Clear);
    }

    /// The search consumer of the arm: over a sandbag line with one gap, a
    /// non-crusher's route takes the gap and a crusher's route goes straight
    /// through the wall cell (I4's drive-over crush then flattens it).
    #[test]
    fn astar_routes_non_crushers_around_a_sandbag_line_and_crushers_through_it() {
        // Seven by five: sandbags down column 3 with the only gap at (3, 0),
        // so the crusher's straight run along row 4 (6 steps) is strictly
        // cheaper than the non-crusher's detour through the gap (8 steps) and
        // the outcome rests on the arm, not on the direction tiebreak.
        let (w, h) = (7u16, 5u16);
        let mut cells = Vec::with_capacity((w * h) as usize);
        for ry in 0..h {
            for rx in 0..w {
                let mut cell = ResolvedTerrainCell::clear_for_test(rx, ry);
                if rx == 3 && ry != 0 {
                    cell.overlay_zone_type = Some(zone_class::CRUSHABLE);
                    cell.zone_type = zone_class::CRUSHABLE;
                }
                cells.push(cell);
            }
        }
        let terrain = ResolvedTerrainGrid::from_cells(w, h, cells);
        let grid = PathGrid::from_resolved_terrain(&terrain);
        let route = |mover_is_crusher: bool| {
            crate::sim::pathfinding::find_path_with_costs(
                &grid,
                (0, 4),
                (6, 4),
                None,
                None,
                Some(MovementZone::Normal),
                Some(&terrain),
                None,
                0,
                mover_is_crusher,
                false,
            )
            .expect("a route exists on both sides of the arm")
        };
        let detour = route(false);
        assert!(
            detour.contains(&(3, 0)),
            "non-crusher takes the gap: {detour:?}"
        );
        assert!(
            !detour.iter().any(|&(x, y)| x == 3 && y != 0),
            "non-crusher never enters a sandbag cell: {detour:?}"
        );
        let straight = route(true);
        assert!(
            straight.iter().any(|&(x, y)| x == 3 && y != 0),
            "crusher drives through the line: {straight:?}"
        );
        assert!(
            straight.len() < detour.len(),
            "the arm, not the tiebreak, decides: {straight:?} vs {detour:?}"
        );
    }

    fn row_entry_input(
        mover_category: EntityCategory,
        branch: VehicleBuildingEntryBranch,
        candidate_x: u16,
    ) -> LiveVehicleBuildingEntry {
        LiveVehicleBuildingEntry {
            mover_category,
            branch,
            checked_building_id: 100,
            candidate_building_id: Some(100),
            candidate_x,
            building_origin_x: 10,
            number_impassable_rows: 1,
            is_unit_repair: false,
            is_bunker: false,
            bunker_occupied: false,
        }
    }

    #[test]
    fn infantry_does_not_use_vehicle_row_contact_skip() {
        let input = row_entry_input(
            EntityCategory::Infantry,
            VehicleBuildingEntryBranch::RadioContact {
                mover_has_contact: true,
            },
            11,
        );

        assert_eq!(
            decide_live_vehicle_building_entry(input),
            BuildingOccupantEntryDecision::KeepBlocker
        );
    }

    #[test]
    fn contacted_vehicle_row_skip_opens_east_columns_but_keeps_west() {
        let contacted = VehicleBuildingEntryBranch::RadioContact {
            mover_has_contact: true,
        };
        assert_eq!(
            decide_live_vehicle_building_entry(row_entry_input(
                EntityCategory::Unit,
                contacted,
                10,
            )),
            BuildingOccupantEntryDecision::KeepBlocker
        );
        assert_eq!(
            decide_live_vehicle_building_entry(row_entry_input(
                EntityCategory::Unit,
                contacted,
                11,
            )),
            BuildingOccupantEntryDecision::SkipBlocker
        );
        assert_eq!(
            decide_live_vehicle_building_entry(row_entry_input(
                EntityCategory::Unit,
                VehicleBuildingEntryBranch::RadioContact {
                    mover_has_contact: false,
                },
                11,
            )),
            BuildingOccupantEntryDecision::KeepBlocker
        );
    }

    #[test]
    fn empty_vs_occupied_bunker_uses_explicit_runtime_occupant_arg() {
        let mut empty = row_entry_input(
            EntityCategory::Unit,
            VehicleBuildingEntryBranch::UnitRepairOrBunker,
            10,
        );
        empty.number_impassable_rows = 0;
        empty.is_bunker = true;

        assert_eq!(
            decide_live_vehicle_building_entry(empty),
            BuildingOccupantEntryDecision::SkipBlocker
        );

        let occupied = LiveVehicleBuildingEntry {
            bunker_occupied: true,
            ..empty
        };
        assert_eq!(
            decide_live_vehicle_building_entry(occupied),
            BuildingOccupantEntryDecision::KeepBlocker
        );
    }

    #[test]
    fn row_helper_requires_same_candidate_building_and_rows_value() {
        let mut other_building = row_entry_input(
            EntityCategory::Unit,
            VehicleBuildingEntryBranch::UnitRepairOrBunker,
            11,
        );
        other_building.is_unit_repair = true;
        other_building.candidate_building_id = Some(200);
        assert_eq!(
            decide_live_vehicle_building_entry(other_building),
            BuildingOccupantEntryDecision::KeepBlocker
        );

        assert_eq!(
            decide_live_vehicle_building_entry(LiveVehicleBuildingEntry {
                branch: VehicleBuildingEntryBranch::RadioContact {
                    mover_has_contact: true
                },
                ..other_building
            }),
            BuildingOccupantEntryDecision::SkipBlocker,
            "73F5A2 consumes false458A00 as skip even for a different first Building"
        );
        let no_rows = LiveVehicleBuildingEntry {
            candidate_building_id: Some(100),
            number_impassable_rows: -1,
            ..other_building
        };
        assert_eq!(
            decide_live_vehicle_building_entry(no_rows),
            BuildingOccupantEntryDecision::KeepBlocker
        );
    }

    #[test]
    fn yr_search_cost_class_rejects_seven_without_neighbor_cost() {
        assert_eq!(
            search_cell_cost_decision(7, false),
            SearchCellCostDecision {
                raw_cost_class: 7,
                effective_cost_class: None,
                expands: false,
                should_call_edge_cost: false,
            }
        );
    }

    #[test]
    fn yr_search_cost_class_preserves_accepted_class_when_gate_is_off() {
        let decision = search_cell_cost_decision(4, false);
        assert_eq!(decision.effective_cost_class, Some(4));
        assert!(decision.expands);
        assert!(decision.should_call_edge_cost);
    }

    #[test]
    fn yr_search_cost_class_coerces_accepted_class_when_gate_is_on() {
        let decision = search_cell_cost_decision(6, true);
        assert_eq!(decision.effective_cost_class, Some(0));
        assert!(decision.expands);
        assert!(decision.should_call_edge_cost);
    }

    #[test]
    fn yr_search_cost_class_two_remains_an_accepted_special_case_input() {
        let decision = search_cell_cost_decision(2, false);
        assert_eq!(decision.effective_cost_class, Some(2));
        assert!(decision.expands);
        assert!(decision.should_call_edge_cost);
    }

    /// `tables` carries the alliance context the arm needs to answer the ally
    /// test; `None` models a caller that has not wired it, which the arm treats
    /// as a reason to decline rather than to guess.
    type AllianceTables<'a> = (
        &'a HouseAllianceMap,
        &'a crate::sim::intern::StringInterner,
        crate::sim::intern::InternedId,
    );

    fn wall_arm<'a>(
        tables: Option<AllianceTables<'a>>,
        is_armed: bool,
        warhead_wall: bool,
        warhead_wood: bool,
    ) -> WallArmContext<'a> {
        WallArmContext {
            overlay_grid: None,
            overlay_registry: None,
            alliances: tables.map(|(alliances, _, _)| alliances),
            interner: tables.map(|(_, interner, _)| interner),
            mover_owner: tables.map(|(_, _, owner)| owner),
            is_armed,
            warhead_wall,
            warhead_wood,
        }
    }

    /// The `Wood=` clause of the wall arm is **Unit-only**.
    ///
    /// `UnitClass::Can_Enter_Cell 0x0073F0A0` tests `Wall=` (`+0x144`) at
    /// `0x0073F4A9` and then `Wood=` (`+0x147`) at `0x0073F4B3` gated on the
    /// overlay's `Armor` (`+0x9C == 6`) at `0x0073F4BD`. The infantry arm
    /// `InfantryClass::Can_Enter_Cell 0x0051BF90` instead calls `FUN_00772AC0`,
    /// whose entire body is `warhead != 0 && *(warhead + 0x144) != 0` — `Wall=`
    /// only, no `Wood=` and no armor compare (decompiled 2026-09-16).
    ///
    /// Stock-reachable: `[SHK]` Shock Trooper (`Category=Soldier`,
    /// `Primary=ElectricBolt` -> warhead `[Shock]`, which declares `Wood=yes`
    /// and no `Wall=`) against `[CAKRMW]` (`Armor=wood`, `Crushable=no`) — a
    /// non-crushable wooden wall gamemd refuses it.
    #[test]
    fn the_wall_arm_wood_route_is_unit_only() {
        // Intern before cloning the thread-local: the clone must already carry
        // the id, or `resolve` in the ally test finds nothing.
        let mover = crate::sim::intern::test_intern("Americans");
        let interner = crate::sim::intern::test_interner();
        let alliances = HouseAllianceMap::new();
        let tables = Some((&alliances, &interner, mover));
        let arm = |armed, wall, wood| wall_arm(tables, armed, wall, wood);

        // Wood= against a wooden wall: the vehicle routes, the infantryman does not.
        assert_eq!(
            arm(true, false, true).weapon_route_code(true, None, false),
            Some(5)
        );
        assert_eq!(
            arm(true, false, true).weapon_route_code(true, None, true),
            None
        );
        // Wall= routes for both classes.
        assert_eq!(
            arm(true, true, false).weapon_route_code(false, None, false),
            Some(5)
        );
        assert_eq!(
            arm(true, true, false).weapon_route_code(false, None, true),
            Some(5)
        );
        // Wood= against a non-wood overlay never routes, for either class.
        assert_eq!(
            arm(true, false, true).weapon_route_code(false, None, false),
            None
        );
        // An unarmed mover leaves at 0x0073F48F before any warhead is read.
        assert_eq!(
            arm(false, true, true).weapon_route_code(true, None, false),
            None
        );
        // Wiring gap: with no alliance tables the arm declines instead of
        // guessing "enemy" and quietly pricing the wall at 20x. An unowned wall
        // with the tables present still takes 5 — that is the `None` wall_owner
        // in the cases above.
        assert_eq!(
            wall_arm(None, true, true, false).weapon_route_code(false, None, false),
            None
        );
    }

    /// `astar_search` consults this classifier ONLY after its own
    /// `neighbor_passable` has refused the neighbour, so a `Clear` answer is not
    /// new information and must never re-admit the cell.
    ///
    /// Mapping `Clear` to class 0 would drop the terms the classifier cannot
    /// see — the ground/bridge layer split and the `neighbor_cell.transition`
    /// (`0x200`) gate a ground->bridge entry must pass — and silently expand a
    /// refused neighbour at 1x.
    #[test]
    fn the_wall_classifier_never_upgrades_a_refusal_to_passable() {
        use crate::sim::pathfinding::SearchCellCostClassifier as _;
        let terrain = crushable_wall_grid();
        let grid = PathGrid::from_resolved_terrain(&terrain);
        let classifier = WallSearchCostClassifier {
            wall: wall_arm(None, true, true, true),
            path_grid: Some(&grid),
            resolved_terrain: Some(&terrain),
            terrain_costs: None,
            movement_zone: Some(MovementZone::Normal),
            speed_type: None,
            is_infantry: false,
            mover_is_crusher: false,
        };
        // (0, 0) is ordinary clear ground: the arm answers Clear, which must
        // still read as "keep the refusal", never as class 0.
        assert_eq!(classifier.classify((0, 0), (0, 0), false), 7);
        // (1, 1) is the fixture's crushable wall. This arm carries no overlay
        // grid, so `overlay_at` yields nothing and the weapon route never runs;
        // the refusal stands at 7. The overlay-backed 4/5 case is covered by
        // `the_wall_arm_answers_seven_where_the_land_row_refuses` below.
        assert_eq!(classifier.classify((0, 0), (1, 1), false), 7);
    }

    /// A 3x3 board whose centre carries a non-crushable `Wall=yes` overlay.
    ///
    /// `track_row` is the centre cell's `Track` land row: `Some(0)` is a row
    /// that refuses the mover, `Some(100)` one that admits it.
    fn wall_row_fixture(track_row: Option<u8>) -> ResolvedTerrainGrid {
        let mut cells = Vec::with_capacity(9);
        for ry in 0..3u16 {
            for rx in 0..3u16 {
                let mut cell = ResolvedTerrainCell::clear_for_test(rx, ry);
                cell.speed_costs.track = Some(100);
                if (rx, ry) == (1, 1) {
                    // `RecalcZoneType` reduces a non-crushable `Wall=` overlay to
                    // class 2, and `ResolvedTerrainGrid` marks it `overlay_blocks`
                    // — which is exactly why the wall arm cannot key on the wider
                    // `land_passable`.
                    cell.zone_type = zone_class::WALL;
                    cell.overlay_zone_type = Some(zone_class::WALL);
                    cell.overlay_blocks = true;
                    cell.speed_costs.track = track_row;
                }
                cells.push(cell);
            }
        }
        ResolvedTerrainGrid::from_cells(3, 3, cells)
    }

    /// Native's wall arm does **not** return. It accumulates 4/5 into the
    /// running code, falls through the occupant walk, and then reads the ground
    /// land row at `0x0073FAB5` (`FLD [ECX*4 + 0x89EA40]` / `FCOMP 0.0`); a zero
    /// row returns 7 at `0x0073FAD0`, and `InfantryClass` does the same at
    /// `0x0051C7D0`, whatever the arm accumulated. So a wall standing on terrain
    /// this mover's speed row refuses answers 7, not 4/5.
    ///
    /// This is also the first exercise of the overlay-backed producer path:
    /// `WallArmContext::overlay_at` -> `weapon_route_code` with a live
    /// `OverlayGrid` and `OverlayTypeRegistry`.
    #[test]
    fn the_wall_arm_answers_seven_where_the_land_row_refuses() {
        use crate::rules::ini_parser::IniFile;
        let registry = crate::rules::overlay_types::OverlayTypeRegistry::from_ini(
            &IniFile::from_str("[OverlayTypes]\n0=GAWALL\n\n[GAWALL]\nWall=yes\n"),
            None,
        );
        let mut overlays = crate::sim::overlay_grid::OverlayGrid::new(3, 3);
        // Unowned: `Is_Ally_ByIndex` rejects index -1, so the route takes 5.
        overlays.cell_mut(1, 1).overlay_id = Some(0);
        // Intern before cloning the thread-local interner.
        let mover = crate::sim::intern::test_intern("Americans");
        let interner = crate::sim::intern::test_interner();
        let alliances = HouseAllianceMap::new();

        let entry = |track_row: Option<u8>| {
            let terrain = wall_row_fixture(track_row);
            let grid = PathGrid::from_resolved_terrain(&terrain);
            evaluate_can_enter_cell(CanEnterCellContext {
                wall: Some(WallArmContext {
                    overlay_grid: Some(&overlays),
                    overlay_registry: Some(&registry),
                    alliances: Some(&alliances),
                    interner: Some(&interner),
                    mover_owner: Some(mover),
                    is_armed: true,
                    warhead_wall: true,
                    warhead_wood: false,
                }),
                target: (1, 1),
                terrain_layer: MovementLayer::Ground,
                movement_zone: Some(MovementZone::Normal),
                speed_type: Some(SpeedType::Track),
                path_grid: Some(&grid),
                resolved_terrain: Some(&terrain),
                terrain_costs: None,
                bypass_grid: false,
                is_infantry: false,
                mover_is_crusher: false,
            })
        };

        // Row admits the mover: the weapon route survives as the enemy class 5.
        assert_eq!(
            entry(Some(100)),
            CanEnterCellResult::WallBlocked { cost_class: 5 }
        );
        // Row refuses it: native's post-arm read returns 7 regardless of the
        // accumulated 5, so the wall class must not escape.
        assert_eq!(entry(Some(0)), CanEnterCellResult::HardBlocked);
    }
}
