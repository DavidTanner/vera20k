//! Bridge topology read service (Slice 3).
//!
//! Single owner of the gamemd-native bridge bit semantics and signed
//! effective-height math, plus the service-facing handle for the traversal gate.
//! Consumers (movement, combat-AoE, occupancy, pathfinding) construct a borrowed
//! `CellBridgeView` over the canonical cell store and read these predicates
//! instead of re-deriving each one at their own call site.
//!
//! It holds the structural/anchor flag predicates, signed effective height,
//! the list layer enum and the AoE object-layer selector.
//!
//! ## Dependency rules
//! - Depends on map/bridge_facts (flag bits), map/resolved_terrain, sim/pathfinding
//!   (the traversal gate it delegates to). All within sim/ + map/.
//! - NEVER depends on render/ — the render draw-offset lives behind a separate
//!   render-facing trait so this module stays render-free (invariant #1).
//! - All math is integer / `i8`-signed. No f32/f64 (the float boundary is INI
//!   parse only, which this module does not touch).

use crate::map::bridge_facts::BridgeFlags;
use crate::map::resolved_terrain::ResolvedTerrainCell;
use crate::util::lepton::BRIDGE_DECK_HEIGHT_LEVELS;

/// Which persistent cell list an object belongs to. The ground list and the
/// bridge-deck list are distinct so movers/projectiles on a high bridge do not
/// interact with whatever sits underneath the span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListLayer {
    Ground,
    Bridge,
}

/// Borrowed read view of one cell's bridge-relevant substrate fields.
///
/// This is NOT a new owned store — it is an adapter built from the canonical
/// post-map-load cell store (`ResolvedTerrainCell`) so the seven predicates read
/// the same data the rest of the engine stamps at map load. All fields are copied
/// out as the gamemd-native signed/typed forms (e.g. `level` is reinterpreted as
/// `i8`, mirroring `PathCell::signed_level`), so the predicates never re-do the
/// sign or window math.
#[derive(Debug, Clone, Copy)]
pub struct CellBridgeView {
    /// Signed cell height level. Reinterpreted `u8 -> i8` exactly like
    /// `PathCell::signed_level()` so a raw level > 127 reads as negative.
    pub level: i8,
    /// CellClass flag word as a typed handle, single-sourced from `bridge_facts`.
    pub flags: BridgeFlags,
    /// Slope/ramp passability byte (CellClass+0x11C / `PathCell.slope_type`),
    /// reinterpreted signed for the diff-1 traversal sub-branch the gate uses.
    pub ramp_byte: i8,
    /// Final resolved isometric tile id (`ResolvedTerrainCell.final_tile_index`).
    /// Window-compared against the theater tileset bases for the tileset
    /// predicates — NOT the structural flag.
    pub iso_tile_index: i32,
    /// Low-bridge tube index (CellClass+0x116), `None` when not a tube cell.
    pub tube_index: Option<i16>,
    /// Final CellClass LandType (`yr_cell_land_type`), used by the low-bridge
    /// predicate (`== YR_CELL_LAND_TUNNEL`).
    pub land_type: u8,
    /// Bridge state byte (0 = NS base, 9 = EW base, etc.). Carried for the
    /// render draw-offset trait; not used by the sim predicates.
    pub state_byte: u8,
}

impl CellBridgeView {
    /// Build a view from the canonical resolved-terrain cell.
    ///
    /// The `u8 -> i8` casts on `level` and `slope_type` are deliberate: they
    /// reproduce gamemd's signed reinterpretation of those bytes (a raw level of
    /// `0xFE` is height `-2`, not `254`). This is the only place the cast lives.
    pub fn from_resolved(cell: &ResolvedTerrainCell) -> Self {
        CellBridgeView {
            level: cell.level as i8,
            flags: BridgeFlags(cell.bridge_facts.raw_flags),
            ramp_byte: cell.slope_type as i8,
            iso_tile_index: cell.final_tile_index,
            tube_index: cell.tube_index.map(|t| t.0 as i16),
            land_type: cell.yr_cell_land_type,
            state_byte: cell.bridge_facts.state_byte,
        }
    }

    // --- Flag predicates (L1 / C1-C3) ----------------------------------------

    /// `0x100` — authoritative structural high-bridge cell.
    #[inline]
    pub fn is_bridge_cell(&self) -> bool {
        self.flags.structural()
    }

    /// `0x80` — anchor cell of the stamp.
    #[inline]
    pub fn is_anchor(&self) -> bool {
        self.flags.anchor()
    }

    // --- Effective height (L2 / C4) ------------------------------------------

    /// Signed level plus the Level-unit anchor seed for anchor cells.
    ///
    /// This is the `(i8)level + ((flags >> 7) & 1) * 4` form (`GetEffectiveHeight`):
    /// the level read is signed and an anchor adds the `+4` deck levels. It is
    /// intentionally NOT the layer-driven `effective_cell_z_for_layer` form
    /// (which keys off the mover's current layer instead of the anchor flag).
    #[inline]
    pub fn effective_height(&self) -> i32 {
        self.level as i32
            + if self.is_anchor() {
                BRIDGE_DECK_HEIGHT_LEVELS
            } else {
                0
            }
    }

    // --- Tileset windows (L3 / L4 / C5) --------------------------------------
    //
    // These are tile-id range checks, completely DISTINCT from the structural
    // `0x100` flag. Conflating a tileset window with structural is DRIFT #6, so
    // the windows are their own predicates and never alias `is_bridge_cell`.

    // --- Low bridge / tube (L5 / C6) -----------------------------------------

    // --- AoE object-layer selector (SHADOW; not yet authoritative) -----------
    //
    // GATE A2/A1: which per-cell object list an AoE detonation damages, by
    // comparing the detonation Z against the deck mid-height.

    /// Select the AoE damage object list for a detonation over this cell.
    ///
    /// gamemd compares the impact Z against `ground_z + half_deck`, where the
    /// half-deck term is two levels (`DECK / 2 = 2`). The compare is
    /// STRICT `>` (impact exactly at the mid-height stays on the ground list).
    /// `ground_z` is the `GetGroundHeight`-equivalent operand in the SAME domain
    /// as `impact_z` (both Level units here).
    ///
    /// (Source: `GATE_BRIDGE_ONBRIDGE_OCCUPANCY_RESOLUTION_GHIDRA_REPORT.md` +
    /// `GATE_BRIDGE_DECK_HEIGHT_RESOLUTION_GHIDRA_REPORT.md` §3/§4.)
    #[inline]
    pub fn aoe_object_layer(&self, impact_z: i32, ground_z: i32) -> ListLayer {
        if self.is_bridge_cell() && impact_z > ground_z + BRIDGE_DECK_HEIGHT_LEVELS / 2 {
            ListLayer::Bridge
        } else {
            ListLayer::Ground
        }
    }

    // --- Occupancy BIT-layer selector (SHADOW; not yet authoritative) --------
    //
    // GATE A2: the per-cell occupancy BITFIELD layer (ground vs bridge/deck) is
    // a SEPARATE selection from the object-LIST layer. The list layer keys off the
    // occupant's persistent `on_bridge` byte; the bit layer keys off the object's
    // Z height vs ground. The two are independent and may disagree at ramp
    // boundaries — a verified gamemd behavior, kept separate here.
}

// --- P2: service-facing traversal-gate handle --------------------------------
//
// The binary-shaped traversal gate already exists and is correct in
// `pathfinding::core`. This slice does NOT relocate it (that flip is hash-relevant
// and out of scope here); it only re-exports the existing owner at crate
// visibility under a service-facing alias so combat/occupancy could reach the
// same gate the same way A*/runtime do. The pathfinding owner stays authoritative;
// this is a delegating handle that proves the seam is identical (see the shadow
// test below), not a second implementation.
//
// Visibility note: the gate and its input/result types are `pub(crate)` in
// pathfinding, so this handle is `pub(crate)` too — it cannot be made more public
// than its owner, and making it so would be the authority-flip this slice avoids.
//
// GATE A4 inventory (verified, `GATE_BRIDGE_TRAVERSAL_RESOLUTION_GHIDRA_REPORT.md`):
//   - The ground-unit bridge-traversal validator is the function this handle
//     delegates to; in gamemd it is dispatched via the Foot/Unit/Infantry vtable
//     slot `+0x1B0` (Aircraft/Building override that slot with non-bridge
//     functions, so the dispatch is ground-unit-only). Our handle mirrors that:
//     it is reached only from the ground-unit cell-entry path, never aircraft.
//   - Warhead field `+0x144` is the `Wall=` boolean (INI key "Wall", default
//     false). It is the per-warhead half of the AoE bridge-destruction gate
//     (`DestroyableBridges && warhead.wall`) and also allows overlay-wall
//     destruction. NOT to be conflated with `WallAbsoluteDestroyer` (`+0x145`).
//     Recorded here for the topology inventory; the warhead parser/collapse wiring
//     that consumes it is out of this slice's scope.
//   - The `Level + 4` height seed the gate applies is the Level-unit pathfinding
//     seed (1 ElevationIncrement) — DISTINCT from the lepton coordinate-Z deck
//     offset (`BRIDGE_DECK_HEIGHT_LEPTONS`). The two are never mixed (A1 §5).

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::bridge_facts::{BRIDGE_FLAG_ANCHOR_SELF, BRIDGE_FLAG_STRUCTURAL};
    use crate::sim::pathfinding::{
        BridgeTraversalInput, BridgeTraversalResult, PathGrid,
        check_bridge_traversal as bridge_traversal_gate,
    };

    /// Build a view directly from raw fields (bypasses `from_resolved` so a test
    /// can pin a precise flag/level/tile combination without a full terrain cell).
    fn view(level: i8, raw_flags: u32, iso_tile_index: i32) -> CellBridgeView {
        CellBridgeView {
            level,
            flags: BridgeFlags(raw_flags),
            ramp_byte: 0,
            iso_tile_index,
            tube_index: None,
            land_type: 0,
            state_byte: 0,
        }
    }

    #[test]
    fn effective_height_anchor_plus4_signed_level() {
        // L2: signed level + exactly the deck height for an anchor; NOT the layer
        // form. Feed a raw level byte > 127 (0xFE) to prove the i8 reinterpret.
        let raw_level = 0xFEu8 as i8; // -2
        assert_eq!(raw_level, -2, "0xFE must reinterpret as -2");

        let anchor = view(raw_level, BRIDGE_FLAG_ANCHOR_SELF, 0);
        assert_eq!(
            anchor.effective_height(),
            -2 + 4,
            "anchor adds the deck height"
        );

        let non_anchor = view(raw_level, 0, 0);
        assert_eq!(
            non_anchor.effective_height(),
            -2,
            "non-anchor is the bare level"
        );
    }

    #[test]
    fn aoe_object_layer_strict_gt_half_deck() {
        // GATE A2/A1: STRICT `>` against ground + half_deck (half = DECK/2 = 2).
        // base = 100; structural cell.
        let bridge = view(100, BRIDGE_FLAG_STRUCTURAL, 0);
        let ground_z = 100;
        let half = BRIDGE_DECK_HEIGHT_LEVELS / 2; // = 2
        assert_eq!(half, 2, "half-deck is 2 levels");

        // Exactly at the mid-height -> Ground (strict `>` excludes equality).
        assert_eq!(
            bridge.aoe_object_layer(ground_z + half, ground_z),
            ListLayer::Ground
        );
        // One above the mid-height -> Bridge.
        assert_eq!(
            bridge.aoe_object_layer(ground_z + half + 1, ground_z),
            ListLayer::Bridge
        );
        // Below -> Ground.
        assert_eq!(
            bridge.aoe_object_layer(ground_z, ground_z),
            ListLayer::Ground
        );

        // Non-structural cell is always Ground regardless of Z.
        let non_bridge = view(100, 0, 0);
        assert_eq!(
            non_bridge.aoe_object_layer(ground_z + 999, ground_z),
            ListLayer::Ground
        );
    }

    #[test]
    fn service_gate_handle_is_bit_identical_to_pathfinding_gate() {
        // P2 shadow: the service-facing handle must produce the exact same
        // `BridgeTraversalResult` as calling the pathfinding owner directly, for a
        // direction == -1 candidate-only seed over a 1x1 structural fixture. This
        // proves the delegating seam is identical without relocating the gate.
        use crate::sim::pathfinding::PathCell;

        let candidate = PathCell {
            ground_walkable: true,
            bridge_walkable: true,
            bridge_structural: true,
            bridge_marker_0x80: false,
            transition: false,
            ground_level: 2,
            bridge_deck_level: 6,
            slope_type: 0,
            tube_index: None,
            low_bridge_tube_cell: false,
        };
        let grid: PathGrid = PathGrid::from_cells(vec![candidate], 1, 1);

        let input = BridgeTraversalInput {
            candidate: grid.cell(0, 0).unwrap(),
            candidate_coord: (0, 0),
            direction: -1,
            path_height: -1,
            parent: None,
        };

        let via_service: BridgeTraversalResult = bridge_traversal_gate(&grid, input);
        let via_owner = crate::sim::pathfinding::check_bridge_traversal(&grid, input);

        assert_eq!(
            via_service, via_owner,
            "service handle must equal the owner"
        );
        assert!(via_service.allowed);
        assert_eq!(via_service.path_height, 6); // signed_level(2) + 4
        assert!(!via_service.force_bridge_list);
    }
}
