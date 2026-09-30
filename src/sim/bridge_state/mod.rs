//! Mutable bridge runtime state layered on top of resolved terrain.
//!
//! Bridges are modeled as terrain, not spawned entities. This module owns the
//! destroyable runtime state used by combat, layered pathing, and bridge-deck
//! fallout handling.
//!
//! Ordinary wooden74..101 and concrete205..232 overlays share scalar
//! `ordinary_damage` / `ordinary_repair` controllers and synchronous world
//! publication. Raw Cell overlay fields own those states; this module's
//! per-cell overlay is a derived mirror. Structural ramp/body state machines
//! retain their separate native flags, anchors and ordered fallout.

//!
//! ## Tagged natives with no counterpart in this crate
//!
//! Four members of the Ghidra BRIDGE_HIGH / BRIDGE_LOW tag sets have no Rust
//! behaviour to compare against, each for a different and verified reason:
//!
//! - `MapClass::RecalcBridgeShroudFlags` 0x00578100 — **not bridge code.**
//!   Body decompiled 2026-08-19: two whole-map cell iterations that recompute
//!   shroud edge bitmasks through `Shroud_EdgeBitmask_Calculator` into
//!   `cell+0x120` and enqueue tactical redraws. No bridge field, flag or
//!   tileset appears in it, and the `+0x140` bit 0x20 its first pass gates on
//!   is not a bridge bit — `SetBridgeDirection`'s clear mask 0xFFFEE07F
//!   preserves it. The tag is wrong; removing it needs a Ghidra write.
//! - `FUN_0056A080` — bridge code with zero xrefs. Whether that is genuine
//!   dead code or a lost indirect reference is unresolved; either way nothing
//!   reaches it at runtime, so there is nothing to be faithful to.
//! - `MapClass::IncrementBridgeCounter` 0x00578AC0 — single caller
//!   `FUN_004F42F0`, itself untraced, so no trigger is established. No
//!   counterpart here.
//! - `ShipLocomotionClass::Compute_BridgeZOffset` 0x0069EBB0 — no CALL xrefs.
//!   Its Drive-side analogue `DriveLocomotionClass::ComputeBridgeZOffset`
//!   0x004AF4A0 is a one-shot constant initializer and is bound in
//!   `sim::movement::movement_bridge`; this one cannot be bound without a
//!   reference.
pub(crate) mod damage_dispatch;
mod damaged_variant;
pub(crate) mod gap_restamp;
pub(crate) mod occupants;
pub(crate) mod ordinary;
pub(crate) mod ordinary_damage;
pub(crate) mod ordinary_repair;
pub(crate) mod publication;
pub(crate) mod ramp_repair;
mod record_scan;
pub(crate) mod rim;
mod zone_activation;

use crate::map::resolved_terrain::ResolvedTerrainGrid;
use damaged_variant::extend_unique_cells;
use std::collections::{BTreeMap, VecDeque};

/// Sentinel `overlay_byte` value meaning "no bridge overlay" (the original
/// engine's -1 / 0xFF). A cell carrying this byte has no own overlay sprite.
/// It may still be a structural deck cell stamped by a neighboring anchor;
/// native structural consumers read live BridgeCellFacts bit0x100 instead.
/// Also written by the orchestrator's `update_adjacent_bridges` stub reset.
const OVERLAY_BYTE_NONE: u8 = 0xFF;

/// High-bridge theater slots used by the map-load bridge-record walk.
/// A negative entry is a deliberately unused slot.
const HIGH_BRIDGE_START_SUBTILE: [i32; 16] = [7, 7, -1, 7, 7, -1, 4, 4, 4, 4, 4, 2, 2, 2, 2, 2];
const HIGH_BRIDGE_WALK_DIRECTION: [i32; 16] = [2, 2, -1, 4, 4, -1, 2, 2, 2, 2, 2, 4, 4, 4, 4, 4];
const HIGH_BRIDGE_END_SUBTILE: [i32; 16] = [-1, -1, 4, -1, -1, 2, 4, 4, 4, 4, 4, 2, 2, 2, 2, 2];
// Static bridge axis/anchor vocabulary is map-owned (map::bridge_facts, F05);
// sim re-exports so runtime and serialized consumers keep their paths.
pub use crate::map::bridge_facts::{Axis, BridgeheadAnchorClass};

/// Per-cell damage state encoding all 18 state-byte values.
///
/// Body cells transition Healthy → Damaged → Destroyed under repeated
/// damage (per axis). Partial-collapse states are reached only via
/// bridgehead final-step cascade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum DamageState {
    /// Healthy body — `variant` carries the 6-frame jitter (0..=5 per axis,
    /// map-load-deterministic, never advances during gameplay).
    /// Maps to state byte 0–5 (NS) or 9–14 (EW).
    Healthy { variant: u8 },
    /// Damaged body — next hit collapses. State byte 6 (NS) / 15 (EW).
    Damaged,
    /// Partial collapse: ramp B already collapsed; this cell will fire
    /// CollapseA. State byte 7 (NS) / 17 (EW).
    PartialCollapseA,
    /// Partial collapse: ramp A already collapsed; this cell will fire
    /// CollapseB. State byte 8 (NS) / 16 (EW).
    PartialCollapseB,
    /// Fully destroyed.
    Destroyed,
}

impl DamageState {
    /// Encode to binary state byte (`CellClass+0x11E`).
    ///
    /// Per HIGH §3.1 / `apply_ramp_transition` docstring:
    /// - NS axis: Healthy{variant: 0..=5} → 0..=5; Damaged → 6;
    ///   PartialCollapseA → 7; PartialCollapseB → 8; Destroyed → 0.
    /// - EW axis: Healthy{variant: 0..=5} → 9..=14; Damaged → 0xF;
    ///   PartialCollapseA → 0x11; PartialCollapseB → 0x10; Destroyed → 0.
    ///
    /// **Note:** `Destroyed` always maps to byte 0, which is also the encoding
    /// for `Healthy{variant: 0}` initial state. Callers must use context
    /// (phase + prior state) to disambiguate after a `from_state_byte(0)` decode.
    /// `to_state_byte` is unambiguous (every variant has exactly one encoding).
    pub fn to_state_byte(self, axis: Axis) -> u8 {
        let ns_base: u8 = 0;
        let ew_base: u8 = 9;
        let base = match axis {
            Axis::NS => ns_base,
            Axis::EW => ew_base,
        };
        match self {
            DamageState::Healthy { variant } => base + variant.min(5),
            DamageState::Damaged => match axis {
                Axis::NS => 6,
                Axis::EW => 0xF,
            },
            DamageState::PartialCollapseA => match axis {
                Axis::NS => 7,
                Axis::EW => 0x11,
            },
            DamageState::PartialCollapseB => match axis {
                Axis::NS => 8,
                Axis::EW => 0x10,
            },
            DamageState::Destroyed => 0,
        }
    }

    /// Render-side state byte. Returns the *base* byte for `Healthy { variant }`
    /// (`0` for NS, `9` for EW) regardless of the stored variant. The renderer
    /// re-derives Latin-square jitter from cell `(x, y)` per the binary
    /// `DrawOverlay_Body` path (RE doc §3.3.1, ledger #4).
    #[cfg(test)]
    pub fn render_state_byte(self, axis: Axis) -> u8 {
        match self {
            DamageState::Healthy { .. } => match axis {
                Axis::NS => 0,
                Axis::EW => 9,
            },
            other => other.to_state_byte(axis),
        }
    }

    /// Decode from binary state byte. Returns `None` for bytes outside the
    /// defined ranges (NS: 0..=8; EW: 9..=0x11).
    ///
    /// **State 0 ambiguity:** byte 0 always decodes to `Healthy{variant: 0}`.
    /// Post-collapse `Destroyed` cells also have byte 0 in the binary, but the
    /// caller (body driver) writes `Destroyed` directly without round-tripping
    /// through `from_state_byte`. Test fixtures and snapshot consistency checks
    /// should not rely on this method to recover `Destroyed`.
    pub fn from_state_byte(byte: u8) -> Option<Self> {
        match byte {
            0..=5 => Some(DamageState::Healthy { variant: byte }),
            6 => Some(DamageState::Damaged),
            7 => Some(DamageState::PartialCollapseA),
            8 => Some(DamageState::PartialCollapseB),
            9..=14 => Some(DamageState::Healthy { variant: byte - 9 }),
            0xF => Some(DamageState::Damaged),
            0x10 => Some(DamageState::PartialCollapseB),
            0x11 => Some(DamageState::PartialCollapseA),
            _ => None,
        }
    }
}

/// Cell role within an `AnchorSpan`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum BridgeCellRole {
    /// Anchor cell: primary cell of an anchor span; carries the canonical state byte.
    Anchor,
    /// Body cell: non-anchor structural cell; follows `anchor_span_id` for state-machine processing.
    Body,
    /// Bridgehead cell: ramp connection-piece off the body.
    Bridgehead,
    /// Tail cell (cell 5 of anchor pattern, walked in `–direction` from anchor).
    Tail,
}

/// Compass-direction enum.
///
/// Discriminant values must match the binary's table indices because
/// `set_bridge_direction` uses them to index into the offsets table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum Direction {
    N = 0,
    NE = 1,
    E = 2,
    SE = 3,
    S = 4,
    SW = 5,
    W = 6,
    NW = 7,
}

impl Direction {
    /// Cell-coord offset `(dx, dy)`. Signed because directions can decrement.
    pub const fn offset(self) -> (i32, i32) {
        match self {
            Direction::N => (0, -1),
            Direction::NE => (1, -1),
            Direction::E => (1, 0),
            Direction::SE => (1, 1),
            Direction::S => (0, 1),
            Direction::SW => (-1, 1),
            Direction::W => (-1, 0),
            Direction::NW => (-1, -1),
        }
    }

    /// `(self - 4) & 7` — opposite direction. Used by `set_bridge_direction`
    /// to compute cell 5 (walked in –direction from anchor).
    pub const fn opposite(self) -> Direction {
        match self {
            Direction::N => Direction::S,
            Direction::NE => Direction::SW,
            Direction::E => Direction::W,
            Direction::SE => Direction::NW,
            Direction::S => Direction::N,
            Direction::SW => Direction::NE,
            Direction::W => Direction::E,
            Direction::NW => Direction::SE,
        }
    }
}

/// `apply_ramp_transition` phase. Maps to one of the 16 ramp transition helpers
/// (NS/EW × DamageA/DamageB/CollapseA/CollapseB).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Phase {
    DamageA,
    DamageB,
    CollapseA,
    CollapseB,
}

/// First-class anchor-span representation. One span per anchor cell.
///
/// Walker pattern: up to 6 cells (anchor + 3 walked +dir + 1 walked –dir +
/// optional fixed-offset cell when direction == W). Per-cell action
/// (BlowUpBridge vs flag-only) is determined by slot index.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct AnchorSpan {
    /// Stable ID, matches `BridgeRuntimeCell.anchor_span_id`.
    pub id: u16,
    /// The anchor cell. Slot 0.
    pub anchor: (u16, u16),
    /// All cells in walker order:
    /// `[0]=anchor, [1..=3]=+direction × 1/2/3, [4]=-direction × 1, [5]=fixed-offset (only when direction == W)`.
    /// `None` for unused slots when the optional fixed-offset cell isn't present.
    pub cells: [Option<(u16, u16)>; 6],
    /// Body axis (NS or EW). Determined from `bridge_layer.direction`.
    pub axis: Axis,
    /// Walk direction (compass index 0–7). Used to compute walked cells.
    pub direction: Direction,
}

impl AnchorSpan {
    /// Cells receiving `BlowUpBridge` on destruction path: slots 0, 1, 2, 4.
    /// Slot 3 (cell 4) and slot 5 (cell 6) are flag-only.
    pub const BLOW_UP_SLOTS: [usize; 4] = [0, 1, 2, 4];

    /// Iterate `(slot, cell)` for present cells (skips `None`).
    pub fn iter_cells(&self) -> impl Iterator<Item = (usize, (u16, u16))> + '_ {
        self.cells
            .iter()
            .enumerate()
            .filter_map(|(i, c)| c.map(|cell| (i, cell)))
    }

    /// Cells that get `BlowUpBridge` on destruction. Skips slots 3 and 5
    /// which are flag-only.
    pub fn blow_up_cells(&self) -> impl Iterator<Item = (u16, u16)> + '_ {
        Self::BLOW_UP_SLOTS
            .iter()
            .filter_map(|&slot| self.cells[slot])
    }
}

/// One area's bridge-damage input. Combat calls the world orchestrator
/// synchronously after that area's receivers; it is never queued across the
/// bullet's animation/cluster tail. The orchestrator owns native admission,
/// strength RNG, driver retries, publication and target release.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct BridgeDamageEvent {
    pub rx: u16,
    pub ry: u16,
    pub damage: i32,
    /// Interned warhead ID — used for IonCannon identity check (combat
    /// boundary pre-resolves `is_ion_cannon`) and for InfDeath selection in
    /// the C4Warhead ground-kill cascade.
    pub warhead_ref: crate::sim::intern::InternedId,
    /// Pre-resolved at combat: `warhead_ref == rules.ion_cannon_warhead_id()`.
    /// Bypasses the BridgeStrength RNG gate; enables the 3-retry loop on
    /// state-machine paths only (direct-overlay paths are single-shot).
    pub is_ion_cannon: bool,
    /// Exact signed world height in leptons, retained from the detonation.
    /// Structural state-machine paths admit (ground + 208, ground + 520];
    /// nonstructural tiles and direct-overlay paths have no height gate.
    pub impact_z_leptons: i32,
}

/// Path discriminator for the bridge-damage 4-path dispatcher.
/// Order matches the binary's outer-dispatch evaluation order
/// (HighSM → LowSM → LowDirect → HighDirect).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchPath {
    /// HIGH state-machine: anchor / body / tail / bridgehead cell whose
    /// overlay byte has already transitioned out of the raw body range.
    /// Includes Z-height range gate.
    HighStateMachine,
    /// LOW state-machine: same shape as `HighStateMachine` for low bridges.
    /// Includes Z-height range gate.
    LowStateMachine,
    /// LOW direct-overlay: `cell.overlay_byte ∈ [0x4A..=0x63]`. Single-shot;
    /// no Z-gate.
    LowDirect,
    /// HIGH direct-overlay: `cell.overlay_byte ∈ [0xCD..=0xE6]`. Single-shot;
    /// no Z-gate.
    HighDirect,
}

impl DispatchPath {
    /// State-machine paths support the IonCannon 3-retry loop. Direct-overlay
    /// paths are single-shot regardless of warhead.
    pub fn is_state_machine(self) -> bool {
        matches!(
            self,
            DispatchPath::HighStateMachine | DispatchPath::LowStateMachine
        )
    }
}

/// Outcome of one `body_cell_advance_state` invocation. Mirrors the return
/// codes of binary `ProcessBridgeDamageStateMachine_High @ 0x576BA0` body
/// branch (0 = absorbed, 1 = collapse), with structured fallout for the
/// orchestrator to dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateOutcome {
    /// Damage absorbed — anchor advanced from `Healthy` to `Damaged`. Bridge
    /// still passable. Renderer should redraw.
    Absorbed {
        /// Cells whose `ToggleBridgePavement @ 0x0056E990` equivalent changed
        /// the current TMP damage selector. Native marks each cell before its
        /// direction-0..7 recursion, so order is presentation-significant.
        damaged_variant_cells: Vec<(u16, u16)>,
    },
    /// Anchor collapsed — `damage_state` became `Destroyed`. Cascade actions
    /// for orchestrator follow.
    Collapsed {
        /// Boolean returned by the underlying gamemd bridge damage helper.
        /// Usually true for collapse, except low bridgehead slot `+3`: that
        /// branch performs collapse side effects but returns false.
        binary_success: bool,
        /// Cells whose `damage_state` was set to `Destroyed` in this call
        /// (typically just the anchor; perpendicular targets that hit
        /// collapse-final via `update_ramp_perpendicular` also appear here).
        destroyed_cells: Vec<(u16, u16)>,
        /// `BlowUpBridge` cascade actions emitted by `set_bridge_direction`.
        /// Orchestrator dispatches these (kill ground occupants, Limbo
        /// bridge-deck, spawn debris).
        set_bridge_direction: crate::sim::bridge_specs::SetBridgeDirectionResult,
        /// Exact native setter call order for the represented 0x1180 subset.
        /// Perpendicular ramp-helper setters precede the parent span setter;
        /// bridgehead collapse can carry a helper setter even when the legacy
        /// action result above has no setter header. Execution-only: snapshot
        /// persistence stores final real-cell values, never this transcript.
        setter_transcript: Vec<crate::map::bridge_facts::BridgeFlagStamp>,
        /// Cells where `UpdateAdjacentBridges_High` should run for rim
        /// re-evaluation. Orchestrator (Phase F Task 27) runs the actual
        /// rim helper.
        adjacent_bridges_dirty: Vec<(u16, u16)>,
        /// The cell both machines pass to `MapClass::InvalidateBridgeZones`
        /// (`0x0056DAE0`) after the rim update: the anchor for a body collapse
        /// (High `0x005778CE`, Low `0x005721D1`), the blow-up row center for a
        /// bridgehead collapse. Orchestrator dispatches.
        zone_query: (i16, i16),
        /// Cells whose visible terrain changed and must be marked dirty on the
        /// minimap. On a collapse this is the collapsed triple PLUS every
        /// cascade-leaf cell touched — including intermediate `Damaged`
        /// perpendicular neighbors, not only the finals — so a partially-
        /// damaged neighbor's minimap variant does not go stale. The
        /// orchestrator feeds these into `mark_radar_terrain_dirty_cells`,
        /// the same channel the engineer-repair path uses.
        radar_cells: Vec<(u16, u16)>,
        /// Ordered cells changed by the ramp helpers' nested
        /// `ToggleBridgePavement @ 0x0056E990` calls. Kept separate so the
        /// existing collapse dirty-set ordering remains unchanged.
        damaged_variant_cells: Vec<(u16, u16)>,
    },
    /// Cell is not a body-bridge cell, anchor span lookup failed, or anchor
    /// is already `Destroyed`. No-op.
    NoChange,
}

impl StateOutcome {
    /// Whether this invocation produced any local state/cascade side effect.
    pub fn has_effect(&self) -> bool {
        !matches!(self, StateOutcome::NoChange)
    }

    /// Boolean success value returned by gamemd's bridge damage helper.
    ///
    /// This is separate from `has_effect`: low bridgehead slot `+3` performs
    /// collapse side effects while returning false.
    pub fn apply_damage_success(&self) -> bool {
        matches!(
            self,
            StateOutcome::Collapsed {
                binary_success: true,
                ..
            }
        )
    }

    pub fn damaged_variant_cells(&self) -> &[(u16, u16)] {
        match self {
            StateOutcome::Absorbed {
                damaged_variant_cells,
            }
            | StateOutcome::Collapsed {
                damaged_variant_cells,
                ..
            } => damaged_variant_cells,
            StateOutcome::NoChange => &[],
        }
    }

    pub fn setter_transcript(&self) -> &[crate::map::bridge_facts::BridgeFlagStamp] {
        match self {
            StateOutcome::Collapsed {
                setter_transcript, ..
            } => setter_transcript,
            StateOutcome::Absorbed { .. } | StateOutcome::NoChange => &[],
        }
    }
}

/// One ordered CellClass bridge-overlay projection operation.
///
/// Native low-bridge walkers write a complete three-cell identity strip before
/// calling `RecalcAttributes` on any member. A flat write-only queue cannot
/// represent that boundary, so the transient stream carries both operations.
/// Repeated writes and recalculations are retained verbatim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BridgeOverlayProjectionOp {
    Write { rx: u16, ry: u16, overlay_byte: u8 },
    Recalc { rx: u16, ry: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct BridgeRuntimeCell {
    pub deck_present: bool,
    pub deck_level: u8,

    /// Per-cell damage state. Drives state-machine progression and renderer
    /// display-tile selection. Replaces the old `destroyed: bool`.
    pub damage_state: DamageState,

    /// Bridge body axis (NS or EW). `None` for cells where axis is not
    /// meaningful (orphan body cells, edge cases). Filled by Task 7 anchor walker.
    pub axis: Option<Axis>,

    /// Cell role within its anchor span. Drives state-machine branch dispatch.
    /// Filled by Task 7 anchor walker.
    pub role: BridgeCellRole,

    /// Stable ID of containing `AnchorSpan` (for body cells); `None` for
    /// bridgehead cells.
    pub anchor_span_id: Option<u16>,

    /// Per-cell visible overlay byte (mirrors binary `CellClass+0x44`).
    /// Populated at map-load from `ResolvedTerrainCell.bridge_layer.overlay_id`;
    /// mutated at runtime by the body-cell state machine and (future) perpendicular
    /// overlay-write branch. Renderer queries this to pick the visible tile.
    pub overlay_byte: u8,

    /// Anchor tile-class mirror written by the bridgehead state machine when
    /// damage lands on a bridgehead-class cell. Carries the visual variant
    /// of the anchor (or neighbor bridgehead progressed via `DamageB`).
    /// Defaults to `Variant0` at map load. The renderer follow-up will read
    /// this to pick the anchor's TMP tile variant; G3 lands the sim-side
    /// write only.
    #[serde(default)]
    pub bridgehead_anchor_class: BridgeheadAnchorClass,
}

/// Binary bridge record kind (`BridgeRecord+0x0C`).
///
/// Verified against `MapClass__ComputeBridgeZones @ 0x0056D6E0`:
/// high bridges write `0`, accepted Tube endpoints write `1`. The historical
/// `Low` Rust name is retained for compatibility. `MapClass__FindBridgeRecord`
/// skips non-zero kinds, so callers must choose high-only vs all-record use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum BridgeRecordKind {
    High,
    Low,
}

impl Default for BridgeRecordKind {
    fn default() -> Self {
        Self::High
    }
}

/// A bridge's map-load endpoint pair for zone connectivity.
/// High records retain the matching theater tiles found by the map-load walk;
/// low records retain their tube-span endpoints.
/// Mirrors gamemd.exe BridgeRecord at MapClass+0x54 (16 bytes each).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct BridgeEndpointRecord {
    /// First endpoint discovered by the record builder.
    pub endpoint_a: (u16, u16),
    /// Matching far endpoint discovered by the record builder.
    pub endpoint_b: (u16, u16),
    /// Whether the bridge is traversable (false = destroyed).
    pub active: bool,
    /// High vs low bridge record kind.
    #[serde(default)]
    pub bridge_kind: BridgeRecordKind,
}

impl BridgeEndpointRecord {
    pub fn is_high(&self) -> bool {
        self.bridge_kind == BridgeRecordKind::High
    }
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct BridgeRuntimeState {
    width: u16,
    height: u16,
    cells: Vec<Option<BridgeRuntimeCell>>,
    /// Strength constant from `[CombatDamage] BridgeStrength=` (default 1000).
    /// Used by the dispatcher's per-path BridgeStrength RNG gate.
    bridge_strength: i32,
    endpoint_records: Vec<BridgeEndpointRecord>,
    /// Source Map Size paired with this derived record set. Only construction
    /// writes it; this receipt is not an independently editable map authority.
    /// Native base-zone endpoint lookup uses stride W+H+1, not terrain width.
    #[serde(default)]
    native_zone_source_size: Option<(i32, i32)>,
    /// First-class anchor spans (one per anchor cell). Replaces emergent
    /// flag-bit detection.
    anchor_spans: BTreeMap<u16, AnchorSpan>,
    /// Active `SpecialFlags::DestroyableBridges` bit. Read by the weapon AoE
    /// bridge-damage outer gate.
    bridge_destroyable_flag: bool,
    /// Ordered bridge-overlay identity writes waiting for the world-owned
    /// CellClass/terrain projection. Runtime cache only: snapshots serialize
    /// the resulting bridge and OverlayGrid identities, never this queue.
    #[serde(skip, default)]
    overlay_projection_ops: Vec<BridgeOverlayProjectionOp>,
}

impl BridgeRuntimeState {
    /// Production entry with the raw Map Size authority, not backing dimensions.
    pub(crate) fn from_resolved_terrain_with_map_size(
        terrain: &ResolvedTerrainGrid,
        destroyable: bool,
        bridge_strength: i32,
        size: (i32, i32),
    ) -> Self {
        Self::build_from_terrain(terrain, destroyable, bridge_strength, Some(size))
    }

    #[cfg(test)]
    pub fn from_resolved_terrain(
        terrain: &ResolvedTerrainGrid,
        destroyable: bool,
        bridge_strength: i32,
    ) -> Self {
        Self::build_from_terrain(terrain, destroyable, bridge_strength, None)
    }

    fn build_from_terrain(
        terrain: &ResolvedTerrainGrid,
        destroyable: bool,
        bridge_strength: i32,
        size: Option<(i32, i32)>,
    ) -> Self {
        let width = terrain.width();
        let height = terrain.height();
        let mut cells = vec![None; width as usize * height as usize];
        let mut groups: Vec<Vec<(u16, u16)>> = Vec::new();
        let mut anchor_spans: BTreeMap<u16, AnchorSpan> = BTreeMap::new();
        let mut visited = vec![false; cells.len()];
        let mut next_span_id: u16 = 1;

        // Pass 1: BFS-group structural bridge cells. High bridges use the
        // authoritative SetBridgeDirection-equivalent facts; low bridges and
        // existing test fixtures keep the legacy deck fallback.
        for cell in terrain.iter() {
            let Some(index) = index_of(width, height, cell.rx, cell.ry) else {
                continue;
            };
            if visited[index] || !resolved_cell_has_runtime_deck(cell) {
                continue;
            }
            let mut queue = VecDeque::from([(cell.rx, cell.ry)]);
            let mut members = Vec::new();
            while let Some((rx, ry)) = queue.pop_front() {
                let Some(idx) = index_of(width, height, rx, ry) else {
                    continue;
                };
                if visited[idx] {
                    continue;
                }
                let Some(resolved) = terrain.cell(rx, ry) else {
                    continue;
                };
                if !resolved_cell_has_runtime_deck(resolved) {
                    continue;
                }
                visited[idx] = true;
                members.push((rx, ry));
                cells[idx] = Some(BridgeRuntimeCell {
                    deck_present: true,
                    deck_level: resolved.bridge_deck_level,
                    damage_state: initial_bridge_damage_state(resolved),
                    axis: bridge_fact_axis(resolved)
                        .or_else(|| bridge_layer_to_axis(resolved.bridge_layer.as_ref())),
                    role: BridgeCellRole::Body, // overwritten in pass 2
                    anchor_span_id: None,
                    overlay_byte: resolved
                        .bridge_facts
                        .overlay_id
                        .or_else(|| resolved.bridge_layer.as_ref().map(|bl| bl.overlay_id))
                        .unwrap_or(0),
                    bridgehead_anchor_class: resolved
                        .bridgehead_anchor_class_at_load
                        .unwrap_or(BridgeheadAnchorClass::Variant0),
                });
                for (nx, ny) in cardinal_neighbors(rx, ry, width, height) {
                    if let Some(neighbor) = terrain.cell(nx, ny) {
                        if resolved_cell_has_runtime_deck(neighbor) {
                            queue.push_back((nx, ny));
                        }
                    }
                }
            }
            if !members.is_empty() {
                groups.push(members);
            }
        }

        // Pass 2: walk anchor patterns. High bridges trust the 0x80 anchor
        // fact. Low/legacy bridges keep the previous bridge_layer fallback.
        for members in &groups {
            for &(rx, ry) in members {
                let Some(resolved) = terrain.cell(rx, ry) else {
                    continue;
                };
                let fact_anchor = resolved.bridge_facts.is_anchor_self();
                let legacy_anchor = resolved.bridge_facts.family
                    == crate::map::bridge_facts::BridgeStampFamily::None
                    && resolved
                        .bridge_layer
                        .as_ref()
                        .is_some_and(|bl| is_anchor_overlay(bl.overlay_id));
                if !fact_anchor && !legacy_anchor {
                    continue;
                }
                let (axis, direction) = if fact_anchor {
                    let stamp_direction = resolved.bridge_facts.direction.unwrap_or(0);
                    (
                        bridge_stamp_direction_to_axis(stamp_direction),
                        bridge_stamp_direction_to_direction(stamp_direction),
                    )
                } else {
                    let bl = resolved
                        .bridge_layer
                        .as_ref()
                        .expect("legacy anchor checked bridge_layer above");
                    let axis = bridge_direction_to_axis(bl.direction);
                    (axis, anchor_walk_direction(axis))
                };
                let span_id = next_span_id;
                next_span_id = next_span_id.saturating_add(1);
                let span = walk_anchor_pattern(span_id, (rx, ry), axis, direction, width, height);
                // Tag each cell in span.
                for (slot, cell_pos) in span.iter_cells() {
                    if let Some(idx) = index_of(width, height, cell_pos.0, cell_pos.1) {
                        if let Some(c) = cells[idx].as_mut() {
                            c.role = if slot == 0 {
                                BridgeCellRole::Anchor
                            } else if slot == 4 {
                                BridgeCellRole::Tail
                            } else {
                                BridgeCellRole::Body
                            };
                            c.anchor_span_id = Some(span_id);
                            c.axis = Some(axis);
                        }
                    }
                }
                anchor_spans.insert(span_id, span);
            }
        }

        // Pass 3: classify bridgehead cells (have bridge_layer but not
        // anchor-overlay; not part of an AnchorSpan).
        for cell in terrain.iter() {
            let Some(idx) = index_of(width, height, cell.rx, cell.ry) else {
                continue;
            };
            let Some(resolved) = terrain.cell(cell.rx, cell.ry) else {
                continue;
            };
            let Some(bl) = resolved.bridge_layer.as_ref() else {
                continue;
            };
            if is_anchor_overlay(bl.overlay_id) {
                continue;
            }
            // Bridgehead cells: ramp/connection cells. May not have deck_present
            // if treated purely as ground transition. Mark role only when
            // a BridgeRuntimeCell already exists.
            if let Some(c) = cells[idx].as_mut() {
                c.role = BridgeCellRole::Bridgehead;
                c.anchor_span_id = None;
                c.axis = Some(bridge_direction_to_axis(bl.direction));
            }
        }

        // Pass 4: register bridgehead cells. ResolvedTerrainCell sets
        // bridge_walkable=true and has_bridge_deck=false at every bridgehead
        // (see resolved_terrain.rs bridgehead pass). Bridgeheads are NOT
        // created in pass 1 (no deck) and NOT touched by pass 3 (no
        // bridge_layer). Without this pass the rebuild silently flips
        // PathCell.bridge_walkable to false on every rebuild_dynamic_path_grid.
        //
        // Contract: deck_present=true permanently, damage_state=Healthy
        // permanently, bridge_group_id=None, anchor_span_id=None, axis=None,
        // overlay_byte=0. The dispatcher (path_matches_cell HighSM/LowSM)
        // rejects Bridgehead+axis.is_none() so no damage-event RNG fires on
        // these cells. Pass-3 bridgeheads (axis=Some) stay in the allowed set.
        for cell in terrain.iter() {
            if !cell.bridge_walkable || cell.has_bridge_deck {
                continue;
            }
            let Some(idx) = index_of(width, height, cell.rx, cell.ry) else {
                continue;
            };
            if cells[idx].is_some() {
                // Defensive: pass 1 already registered a cell here. The
                // condition (bw && !has_deck) should be mutually exclusive
                // with pass 1's has_deck, so this branch is unreachable
                // unless the resolved terrain is internally inconsistent.
                continue;
            }
            cells[idx] = Some(BridgeRuntimeCell {
                deck_present: true,
                deck_level: cell.bridge_deck_level,
                damage_state: DamageState::Healthy { variant: 0 },
                axis: None,
                role: BridgeCellRole::Bridgehead,
                anchor_span_id: None,
                overlay_byte: 0,
                bridgehead_anchor_class: BridgeheadAnchorClass::Variant0,
            });
        }

        let endpoint_records = record_scan::compute_bridge_endpoints(terrain, size);

        Self {
            width,
            height,
            cells,
            bridge_strength,
            endpoint_records,
            native_zone_source_size: size,
            anchor_spans,
            bridge_destroyable_flag: destroyable,
            overlay_projection_ops: Vec::new(),
        }
    }

    /// Look up an anchor span by ID.
    pub fn anchor_span(&self, id: u16) -> Option<&AnchorSpan> {
        self.anchor_spans.get(&id)
    }

    /// All anchor spans, sorted by ID (BTreeMap iteration order).
    pub fn anchor_spans(&self) -> &BTreeMap<u16, AnchorSpan> {
        &self.anchor_spans
    }

    pub fn cell(&self, rx: u16, ry: u16) -> Option<&BridgeRuntimeCell> {
        index_of(self.width, self.height, rx, ry)
            .and_then(|idx| self.cells.get(idx))
            .and_then(|cell| cell.as_ref())
    }

    /// Mutable cell access. Returns `None` if `(rx, ry)` is out of bounds or
    /// the cell is not a bridge runtime cell.
    pub fn cell_mut(&mut self, rx: u16, ry: u16) -> Option<&mut BridgeRuntimeCell> {
        index_of(self.width, self.height, rx, ry)
            .and_then(move |idx| self.cells.get_mut(idx))
            .and_then(|cell| cell.as_mut())
    }

    /// Write one live bridge-overlay byte as a complete one-cell native
    /// transaction. Multi-cell walkers use the deferred form below so they can
    /// place every identity before queueing their ordered recalculations.
    pub(crate) fn write_overlay_byte(&mut self, rx: u16, ry: u16, overlay_byte: u8) -> bool {
        let changed = self.write_overlay_byte_deferred_recalc(rx, ry, overlay_byte);
        if self.cell(rx, ry).is_some() {
            self.queue_overlay_recalc(rx, ry);
        }
        changed
    }

    pub(crate) fn write_overlay_byte_deferred_recalc(
        &mut self,
        rx: u16,
        ry: u16,
        overlay_byte: u8,
    ) -> bool {
        let changed = match self.cell_mut(rx, ry) {
            Some(cell) => {
                let changed = cell.overlay_byte != overlay_byte;
                cell.overlay_byte = overlay_byte;
                changed
            }
            None => return false,
        };
        self.overlay_projection_ops
            .push(BridgeOverlayProjectionOp::Write {
                rx,
                ry,
                overlay_byte,
            });
        changed
    }

    pub(crate) fn queue_overlay_recalc(&mut self, rx: u16, ry: u16) {
        if self.cell(rx, ry).is_some() {
            self.overlay_projection_ops
                .push(BridgeOverlayProjectionOp::Recalc { rx, ry });
        }
    }

    pub(crate) fn take_overlay_projection_ops(&mut self) -> Vec<BridgeOverlayProjectionOp> {
        std::mem::take(&mut self.overlay_projection_ops)
    }

    /// Map width in cells. Needed by walker code in the `walker` submodule
    /// (Rust privacy: child modules can't read parent's private fields
    /// without a getter or `pub(super)`).
    pub fn width(&self) -> u16 {
        self.width
    }

    /// Map height in cells. See `width()` rationale.
    pub fn height(&self) -> u16 {
        self.height
    }

    /// `[CombatDamage] BridgeStrength=` value used by the per-path RNG gate
    /// in the bridge-damage dispatcher. Read-only; set at construction.
    pub fn bridge_strength(&self) -> i32 {
        self.bridge_strength
    }

    /// Whether the global `SpecialFlags::DestroyableBridges` is set. Outer
    /// gate of the bridge-damage dispatcher; if false, bridges are immune.
    pub fn is_destroyable(&self) -> bool {
        self.bridge_destroyable_flag
    }

    /// Unmigrated bridgehead continuation. The shared damage dispatcher owns
    /// 587180's overlay-first selection, and the live publication host owns
    /// the structural body switch, before this continuation is called.
    pub(crate) fn advance_damage_state(
        &mut self,
        rx: u16,
        ry: u16,
        is_high: bool,
        terrain: &mut crate::map::resolved_terrain::ResolvedTerrainGrid,
    ) -> StateOutcome {
        match self.cell(rx, ry).map(|c| c.role) {
            Some(BridgeCellRole::Bridgehead) => {
                self.bridgehead_advance_state(rx, ry, is_high, terrain)
            }
            _ => StateOutcome::NoChange,
        }
    }

    /// Test-only: insert a `BridgeRuntimeCell` at `(rx, ry)`, growing the
    /// internal `cells` Vec and `width`/`height` to fit if needed. Used by
    /// unit tests that need precise control over cell placement and state
    /// without going through `from_resolved_terrain`.
    #[cfg(test)]
    pub(crate) fn test_seed_cell(&mut self, rx: u16, ry: u16, cell: BridgeRuntimeCell) {
        let needed_w = (rx + 1).max(self.width);
        let needed_h = (ry + 1).max(self.height);
        if needed_w != self.width || needed_h != self.height {
            // Resize while preserving existing (rx, ry) → cell mappings.
            let mut new_cells = vec![None; needed_w as usize * needed_h as usize];
            for old_ry in 0..self.height {
                for old_rx in 0..self.width {
                    let old_idx = old_ry as usize * self.width as usize + old_rx as usize;
                    let new_idx = old_ry as usize * needed_w as usize + old_rx as usize;
                    new_cells[new_idx] = self.cells[old_idx];
                }
            }
            self.cells = new_cells;
            self.width = needed_w;
            self.height = needed_h;
        }
        let idx = ry as usize * self.width as usize + rx as usize;
        self.cells[idx] = Some(cell);
    }

    /// Test-only: insert an `AnchorSpan` directly into the registry.
    #[cfg(test)]
    pub(crate) fn test_seed_anchor_span(&mut self, span: AnchorSpan) {
        self.anchor_spans.insert(span.id, span);
    }

    #[cfg(test)]
    pub(crate) fn test_set_endpoint_records(&mut self, records: Vec<BridgeEndpointRecord>) {
        self.endpoint_records = records;
    }

    /// Visible overlay state, also retained by legacy overlay controllers.
    /// This is not a structural-presence query: native constructor5FC380
    /// leaves nonanchor structural cells at overlay-1 (constructor corpus
    /// cases8..11), and the anchor supplies their bridge sprite.
    pub fn effective_render_state(cell: &BridgeRuntimeCell) -> Option<DamageState> {
        let state_from_overlay = match cell.overlay_byte {
            0x4A..=0x4D => Some(DamageState::Healthy {
                variant: cell.overlay_byte - 0x4A,
            }),
            0x4E..=0x52 => Some(DamageState::Damaged),
            0x53..=0x56 => Some(DamageState::Healthy {
                variant: cell.overlay_byte - 0x53,
            }),
            0x57..=0x5B => Some(DamageState::Damaged),
            0x64 | 0x65 => None,
            0xCD..=0xD0 => Some(DamageState::Healthy {
                variant: cell.overlay_byte - 0xCD,
            }),
            0xD1..=0xD5 => Some(DamageState::Damaged),
            0xD6..=0xD9 => Some(DamageState::Healthy {
                variant: cell.overlay_byte - 0xD6,
            }),
            0xDA..=0xDE => Some(DamageState::Damaged),
            0xE7 | 0xE8 => None,
            OVERLAY_BYTE_NONE => None,
            _ => Some(cell.damage_state),
        };
        match state_from_overlay {
            Some(DamageState::Destroyed) | None => None,
            other => other,
        }
    }

    /// Legacy overlay/runtime walkability only. Native Cell+140 bit0x100
    /// readers must use live `BridgeCellFacts::has_structural_bridge` instead:
    /// this owner has no flag authority, and absent side-cell overlays do not
    /// remove a stamped deck. PathGrid limits this fallback to legacy cells.
    pub fn is_bridge_walkable(&self, rx: u16, ry: u16) -> bool {
        self.cell(rx, ry)
            .is_some_and(|cell| cell.deck_present && Self::effective_render_state(cell).is_some())
    }

    /// Bridgehead-cell state-machine driver.
    ///
    /// Sparse-by-design: most bridgehead cells absorb damage via the per-axis
    /// start-cell gate inside `bridgehead_walk_to_anchor` (NS rejects odd
    /// sub-tiles; EW rejects sub-tiles > 4). Only the small subset that passes
    /// the gate reaches the anchor-write path.
    ///
    /// On a successful walk:
    /// - Writes `bridgehead_anchor_class = AboutToFall` on the anchor cell.
    ///   This is the **most-damaged variant** (4th slot in the enum, matching
    ///   the reference engine's anchor-tile write target). A later hit that
    ///   resolves this slot enters the collapse path below.
    /// - Fires `update_ramp_perpendicular(DamageA)` and `DamageB` on the
    ///   anchor's perpendicular neighbors. These do both the existing
    ///   state-byte bump (on Anchor targets) AND the asymmetric A/B
    ///   tile-class progression (on Anchor and Bridgehead targets) —
    ///   `Variant0 → Variant1 → Damaged` via DamageB; DamageA preserves.
    /// - The hit bridgehead cell's own `damage_state` is NEVER modified.
    ///
    /// Returns:
    /// - `StateOutcome::Absorbed` on a successful walk + anchor write.
    /// - `StateOutcome::NoChange` on role mismatch, missing axis, gated
    ///   start cell, or walk-off-map.
    /// - `StateOutcome::Collapsed` when the resolved bridgehead/anchor class
    ///   is already `AboutToFall` (binary slot `+3`).
    ///
    /// `is_high_bridge` selects the slot `+3` binary return value
    /// (high true; low false after collapse side effects).
    ///
    /// Both walks read `CellClass+0x11A`, the iso sub-tile (`0x00576C5F`,
    /// `0x0057722E`, `0x0057727C`), through the live cell lookup.
    pub fn bridgehead_advance_state(
        &mut self,
        rx: u16,
        ry: u16,
        is_high_bridge: bool,
        terrain: &mut crate::map::resolved_terrain::ResolvedTerrainGrid,
    ) -> StateOutcome {
        let live_flags = &mut terrain.bridge_flag_execution_state();
        // 1. Resolve input cell.
        let Some(input_cell) = self.cell(rx, ry).copied() else {
            return StateOutcome::NoChange;
        };

        // 2. Filter: must be a Bridgehead. Body / Anchor / Tail route to the
        //    body driver.
        if !matches!(input_cell.role, BridgeCellRole::Bridgehead) {
            return StateOutcome::NoChange;
        }
        let Some(axis) = input_cell.axis else {
            return StateOutcome::NoChange;
        };

        // 3. Walk to anchor via the sub-tile predicate. The helper
        //    computes walk direction internally per the start cell's sub-tile
        //    and applies the per-axis start-cell gate. Failures (odd NS,
        //    above 4 EW, off-map) yield None — the damage is absorbed without
        //    state change.
        let map_w = self.width;
        let map_h = self.height;
        let sub_tile = |pos: (u16, u16)| {
            terrain.native_cell_sub_tile(terrain.native_cell_identity((pos.0 as i16, pos.1 as i16)))
        };
        let Some(anchor_pos) = crate::sim::bridge_specs::bridgehead_walk_to_anchor(
            (rx, ry),
            axis,
            |pos| Some(sub_tile(pos)),
            map_w,
            map_h,
        ) else {
            return StateOutcome::NoChange;
        };

        let Some(anchor_snapshot) = self.cell(anchor_pos.0, anchor_pos.1).copied() else {
            return StateOutcome::NoChange;
        };
        let input_is_final = matches!(
            input_cell.bridgehead_anchor_class,
            BridgeheadAnchorClass::AboutToFall
        );
        let anchor_is_final = matches!(
            anchor_snapshot.bridgehead_anchor_class,
            BridgeheadAnchorClass::AboutToFall
        );

        if input_is_final || anchor_is_final {
            use crate::sim::bridge_specs::{CellAction, SetBridgeDirectionResult};

            if let Some(anchor_cell) = self.cell_mut(anchor_pos.0, anchor_pos.1) {
                anchor_cell.bridgehead_anchor_class = BridgeheadAnchorClass::AboutToFall;
            }

            let center = crate::sim::bridge_specs::bridgehead_row_center(
                anchor_pos,
                axis,
                sub_tile(anchor_pos),
            );
            let zone_query = (center.0 as i16, center.1 as i16);
            let mut destroyed = Vec::new();
            let mut actions = Vec::new();
            for (slot, pos) in crate::sim::bridge_specs::bridgehead_blow_up_row(
                anchor_pos,
                axis,
                sub_tile(anchor_pos),
                map_w,
                map_h,
            )
            .into_iter()
            .enumerate()
            .filter_map(|(slot, pos)| pos.map(|pos| (slot, pos)))
            {
                if !destroyed.contains(&pos) {
                    destroyed.push(pos);
                }
                actions.push((pos, slot, CellAction::BlowUpBridge));
                if self.cell(pos.0, pos.1).is_some() {
                    let _ = self.write_overlay_byte(pos.0, pos.1, OVERLAY_BYTE_NONE);
                    let c = self
                        .cell_mut(pos.0, pos.1)
                        .expect("bridge cell existed before overlay write");
                    c.damage_state = DamageState::Destroyed;
                    if matches!(c.role, BridgeCellRole::Anchor | BridgeCellRole::Bridgehead) {
                        c.bridgehead_anchor_class = BridgeheadAnchorClass::AboutToFall;
                    }
                }
            }

            let ramp_a = crate::sim::bridge_specs::update_ramp_perpendicular_with_flags(
                self,
                anchor_pos,
                axis,
                Phase::CollapseA,
                is_high_bridge,
                terrain,
                live_flags,
            );
            let ramp_b = crate::sim::bridge_specs::update_ramp_perpendicular_with_flags(
                self,
                anchor_pos,
                axis,
                Phase::CollapseB,
                is_high_bridge,
                terrain,
                live_flags,
            );
            let mut damaged_variant_cells = ramp_a.damaged_variant_cells;
            let mut setter_transcript = ramp_a.setter_transcript;
            extend_unique_cells(&mut damaged_variant_cells, ramp_b.damaged_variant_cells);
            setter_transcript.extend(ramp_b.setter_transcript);
            for &perp_dir in &[Direction::E, Direction::W, Direction::N, Direction::S] {
                let (dx, dy) = perp_dir.offset();
                let nx = anchor_pos.0 as i32 + dx;
                let ny = anchor_pos.1 as i32 + dy;
                if nx < 0 || ny < 0 {
                    continue;
                }
                let pos = (nx as u16, ny as u16);
                if self
                    .cell(pos.0, pos.1)
                    .is_some_and(|c| matches!(c.damage_state, DamageState::Destroyed))
                    && !destroyed.contains(&pos)
                {
                    destroyed.push(pos);
                }
            }

            let adj = compute_adjacent_bridges_dirty(anchor_pos.0, anchor_pos.1, axis);
            return StateOutcome::Collapsed {
                binary_success: is_high_bridge,
                // Cloned before the move below; the BlowUpBridge triple + any
                // perpendicular finals are the minimap-dirty set (BR-16).
                radar_cells: destroyed.clone(),
                destroyed_cells: destroyed,
                set_bridge_direction: SetBridgeDirectionResult {
                    actions,
                    flag_stamp: None,
                },
                setter_transcript,
                adjacent_bridges_dirty: adj,
                zone_query,
                damaged_variant_cells,
            };
        }

        // 4. Write the anchor's bridgehead_anchor_class to AboutToFall
        //    (the most-damaged variant, 4th enum slot). Matches the
        //    reference engine's first-hit write to the anchor's tile-class
        //    field. A later hit that resolves AboutToFall enters the
        //    collapse path above. The hit bridgehead cell's own
        //    damage_state is never touched.
        if let Some(anchor_cell) = self.cell_mut(anchor_pos.0, anchor_pos.1) {
            anchor_cell.bridgehead_anchor_class = BridgeheadAnchorClass::AboutToFall;
        }

        // 5. Fire the perpendicular DamageA + DamageB writes. These do the
        //    state-byte bump on Anchor targets and the asymmetric A/B
        //    tile-class progression on both Anchor and Bridgehead targets.
        let ramp_a = crate::sim::bridge_specs::update_ramp_perpendicular_with_flags(
            self,
            anchor_pos,
            axis,
            Phase::DamageA,
            is_high_bridge,
            terrain,
            live_flags,
        );
        let ramp_b = crate::sim::bridge_specs::update_ramp_perpendicular_with_flags(
            self,
            anchor_pos,
            axis,
            Phase::DamageB,
            is_high_bridge,
            terrain,
            live_flags,
        );
        let mut damaged_variant_cells = ramp_a.damaged_variant_cells;
        extend_unique_cells(&mut damaged_variant_cells, ramp_b.damaged_variant_cells);

        StateOutcome::Absorbed {
            damaged_variant_cells,
        }
    }

    /// Bridge endpoint records for zone connectivity.
    /// Each active record connects ground zones on opposite sides of a bridge.
    pub fn endpoint_records(&self) -> &[BridgeEndpointRecord] {
        &self.endpoint_records
    }

    pub(crate) fn native_zone_source_size(&self) -> Option<(i32, i32)> {
        self.native_zone_source_size
    }

    pub fn iter_cells(&self) -> impl Iterator<Item = ((u16, u16), &BridgeRuntimeCell)> {
        self.cells
            .iter()
            .enumerate()
            .filter_map(move |(idx, cell)| {
                let cell = cell.as_ref()?;
                let rx = (idx % self.width as usize) as u16;
                let ry = (idx / self.width as usize) as u16;
                Some(((rx, ry), cell))
            })
    }
}

fn index_of(width: u16, height: u16, rx: u16, ry: u16) -> Option<usize> {
    (rx < width && ry < height).then_some(ry as usize * width as usize + rx as usize)
}

/// Enumerate the 25 cells in a 5×5 inclusive `[-2..=+2]` scan around
/// `center`. Yields cell coordinates clamped to non-negative `(u16, u16)`
/// (cells with negative computed coords are skipped — they're off-map).
///
/// Used by the engineer-repair trigger. Inclusive bounds `-2..=+2` produce
/// exactly 25 cells when the center is interior; off-map negative cells are
/// silently dropped.
#[cfg(test)]
pub fn cells_in_5x5_scan(center: (u16, u16)) -> impl Iterator<Item = (u16, u16)> {
    let (cx, cy) = (center.0 as i32, center.1 as i32);
    (-2..=2i32).flat_map(move |dy| {
        (-2..=2i32).filter_map(move |dx| {
            let nx = cx + dx;
            let ny = cy + dy;
            if nx < 0 || ny < 0 || nx > u16::MAX as i32 || ny > u16::MAX as i32 {
                None
            } else {
                Some((nx as u16, ny as u16))
            }
        })
    })
}

fn cardinal_neighbors(
    rx: u16,
    ry: u16,
    width: u16,
    height: u16,
) -> impl Iterator<Item = (u16, u16)> {
    const OFFSETS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
    OFFSETS.into_iter().filter_map(move |(dx, dy)| {
        let nx = rx as i32 + dx;
        let ny = ry as i32 + dy;
        (nx >= 0 && ny >= 0 && (nx as u16) < width && (ny as u16) < height)
            .then_some((nx as u16, ny as u16))
    })
}

fn bridge_layer_to_axis(layer: Option<&crate::map::resolved_terrain::BridgeLayer>) -> Option<Axis> {
    layer.map(|bl| bridge_direction_to_axis(bl.direction))
}

fn resolved_cell_has_runtime_deck(
    cell: &crate::map::resolved_terrain::ResolvedTerrainCell,
) -> bool {
    cell.bridge_facts.has_structural_bridge()
        || (cell.has_bridge_deck
            && cell.bridge_facts.family == crate::map::bridge_facts::BridgeStampFamily::None)
}

fn bridge_fact_axis(cell: &crate::map::resolved_terrain::ResolvedTerrainCell) -> Option<Axis> {
    cell.bridge_facts
        .direction
        .map(bridge_stamp_direction_to_axis)
}

fn initial_bridge_damage_state(
    cell: &crate::map::resolved_terrain::ResolvedTerrainCell,
) -> DamageState {
    if cell.bridge_facts.has_structural_bridge()
        || cell.bridge_facts.family != crate::map::bridge_facts::BridgeStampFamily::None
    {
        DamageState::from_state_byte(cell.bridge_facts.state_byte)
            .unwrap_or(DamageState::Healthy { variant: 0 })
    } else {
        DamageState::Healthy { variant: 0 }
    }
}

fn bridge_stamp_direction_to_axis(direction: u8) -> Axis {
    match direction & 7 {
        2 | 6 => Axis::EW,
        _ => Axis::NS,
    }
}

fn bridge_stamp_direction_to_direction(direction: u8) -> Direction {
    match direction & 7 {
        0 => Direction::N,
        1 => Direction::NE,
        2 => Direction::E,
        3 => Direction::SE,
        4 => Direction::S,
        5 => Direction::SW,
        6 => Direction::W,
        _ => Direction::NW,
    }
}

fn bridge_direction_to_axis(d: crate::map::resolved_terrain::BridgeDirection) -> Axis {
    use crate::map::resolved_terrain::BridgeDirection;
    match d {
        BridgeDirection::EastWest => Axis::EW,
        BridgeDirection::NorthSouth => Axis::NS,
        // Low bridges (wood) read bridge_layer separately; treat as NS for now.
        // Phase C may revisit if low needs distinct axis handling.
        BridgeDirection::Low => Axis::NS,
    }
}

/// HIGH bridge anchor overlays = 0x18, 0x19; LOW bridge anchor overlays = 0xED, 0xEE.
///
/// NOTE (Phase B): These overlay IDs are also used to mark every HIGH-bridge
/// deck cell's direction, so under this predicate every HIGH-bridge cell with
/// a bridge_layer becomes an anchor. Phase C anchor-walker correctness tests
/// (Task 27) will tighten this to only true anchor cells.
fn is_anchor_overlay(overlay_id: u8) -> bool {
    matches!(overlay_id, 0x18 | 0x19 | 0xED | 0xEE)
}

/// State-machine convention: NS-axis collapse walks E (dir=2) for ramp A;
/// EW-axis collapse walks S (dir=4) for ramp A. We pick A-direction as the
/// canonical anchor walk direction (cell 5 then walks the opposite from anchor).
fn anchor_walk_direction(axis: Axis) -> Direction {
    match axis {
        Axis::NS => Direction::E,
        Axis::EW => Direction::S,
    }
}

/// Walk the 6-cell anchor pattern. Cells beyond the map edge become `None`.
fn walk_anchor_pattern(
    span_id: u16,
    anchor: (u16, u16),
    axis: Axis,
    direction: Direction,
    width: u16,
    height: u16,
) -> AnchorSpan {
    let mut cells: [Option<(u16, u16)>; 6] = [None; 6];
    cells[0] = Some(anchor);

    let (dx, dy) = direction.offset();
    // Slot 1, 2, 3: walk +direction × 1, 2, 3.
    for step in 1..=3 {
        let nx = anchor.0 as i32 + dx * step;
        let ny = anchor.1 as i32 + dy * step;
        if nx >= 0 && ny >= 0 && (nx as u16) < width && (ny as u16) < height {
            cells[step as usize] = Some((nx as u16, ny as u16));
        }
    }

    // Slot 4: walk -direction × 1.
    let opp = direction.opposite();
    let (odx, ody) = opp.offset();
    let ox = anchor.0 as i32 + odx;
    let oy = anchor.1 as i32 + ody;
    if ox >= 0 && oy >= 0 && (ox as u16) < width && (oy as u16) < height {
        cells[4] = Some((ox as u16, oy as u16));
    }

    // Slot 5: optional extra cell, present only for the dir-W anchor. It is
    // the OPPOSITE step taken twice — `anchor + 2·E` — i.e. one cell beyond the
    // slot-4 opposite cell, NOT a duplicate of it. Matches
    // `bridge_facts::stamp_slots` (`ExtraDir6 = step(opposite, E)`). Writing
    // `+1` here aliased slot 4, which (a) left the true extra cell untagged and
    // (b) flipped the opposite cell's role Tail->Body via last-write-wins in
    // the pass-2 tagging loop.
    if direction == Direction::W {
        let ex = anchor.0 as i32 + 2;
        let ey = anchor.1 as i32;
        if ex >= 0 && ey >= 0 && (ex as u16) < width && (ey as u16) < height {
            cells[5] = Some((ex as u16, ey as u16));
        }
    }

    AnchorSpan {
        id: span_id,
        anchor,
        cells,
        axis,
        direction,
    }
}

/// The two cells the High machine's ramp branch hands to
/// `UpdateAdjacentBridges_High`, around the ramp's canonical sub-tile cell:
/// N then S on an EW ramp (`0x00576FFB..0x00577065`), W then E on an NS ramp
/// (`0x0057752F..0x0057757B`).
fn compute_adjacent_bridges_dirty(rx: u16, ry: u16, axis: Axis) -> Vec<(u16, u16)> {
    let mut out = Vec::with_capacity(2);
    let perpendiculars: [Direction; 2] = match axis {
        Axis::NS => [Direction::W, Direction::E],
        Axis::EW => [Direction::N, Direction::S],
    };
    for d in perpendiculars {
        let (dx, dy) = d.offset();
        let nx = rx as i32 + dx;
        let ny = ry as i32 + dy;
        if nx >= 0 && ny >= 0 {
            out.push((nx as u16, ny as u16));
        }
    }
    out
}

#[cfg(test)]
mod scan_tests;
#[cfg(test)]
mod tests;
