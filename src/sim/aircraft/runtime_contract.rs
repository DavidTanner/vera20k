//! Evidence-bounded YR aircraft runtime contracts.

use crate::rules::locomotor_type::{MovementZone, SpeedType};
use crate::sim::cell_rect::{
    IsClearToMoveResult, LiveCellPassabilityQuery, evaluate_live_cell_passability,
};

/// Convert the statically proved default paradrop edge source into launch facing.
/// Named location: YR linked-aircraft paradrop launch dispatch.
pub fn paradrop_edge_facing_word(default_edge: i32, alternate_type_state: bool) -> u16 {
    let edge = if default_edge == -1 { 0 } else { default_edge };
    let doubled = edge.wrapping_mul(2);
    let base = doubled.wrapping_shl(13);
    if alternate_type_state {
        base.wrapping_sub(0x6001) as u16 & 0xe000
    } else {
        base as u16
    }
}

/// AircraftClass's shared Cell leaf for landing probes.
///
/// Winged returns before map, zone, occupation, wall, or land reads. The live
/// caller must still apply pad/occupant ownership and shroud/state rules.
// Native: AircraftClass::IsCellOccupied wrapper -> CellClass::IsClearToMove.
pub fn aircraft_landing_cell_leaf_clear() -> bool {
    matches!(
        evaluate_live_cell_passability(LiveCellPassabilityQuery {
            target: (0, 0),
            speed_type: SpeedType::Winged,
            movement_zone: MovementZone::Normal,
            requested_zone: None,
            actual_zone: 0,
            requested_layer: None,
            ignore_infantry: false,
            ignore_vehicles: false,
            land_passable: false,
            path_grid: None,
            resolved_terrain: None,
            raw_occupation: None,
        }),
        IsClearToMoveResult::ClearWinged
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paradrop_edge_normalization_and_facing_are_exact() {
        assert_eq!(paradrop_edge_facing_word(-1, false), 0);
        assert_eq!(paradrop_edge_facing_word(1, false), 0x4000);
        assert_eq!(paradrop_edge_facing_word(2, false), 0x8000);
        assert_eq!(paradrop_edge_facing_word(3, false), 0xc000);
        assert_eq!(paradrop_edge_facing_word(1, true), 0xc000);
    }

    #[test]
    fn aircraft_landing_cell_leaf_preserves_winged_early_return() {
        assert!(aircraft_landing_cell_leaf_clear());
    }
}
