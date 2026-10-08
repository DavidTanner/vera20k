//! Evidence-bounded YR aircraft runtime contracts.

use crate::rules::locomotor_type::{MovementZone, SpeedType};
use crate::sim::cell_rect::{
    IsClearToMoveResult, LiveCellPassabilityQuery, evaluate_live_cell_passability,
};

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
    fn aircraft_landing_cell_leaf_preserves_winged_early_return() {
        assert!(aircraft_landing_cell_leaf_clear());
    }
}
