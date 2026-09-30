//! Eight-neighbour selection in Infantry51D52B..51D6BA.
//!
//! Admission belongs to the class receiver. Its candidate query distinguishes
//! illegal cells, legal fallback cells, and preferred cells. The first legal
//! cell is retained while the search continues for a preferred surface.

use crate::map::resolved_terrain::{NativeCellQuery, ResolvedTerrainGrid};

/// Infantry51D258..51D2D0: round the away heading to an octant, then add
/// RandomRanged(0,4)-2. The heading uses physical coordinates, not Foot+4C.
/// Use the shared wide coordinate subtraction before native f32 narrowing.
pub(super) fn source_start_direction(
    current: (i32, i32),
    source: (i32, i32),
    rng: &mut crate::sim::rng::SimRng,
) -> i32 {
    let away = crate::util::direction_tables::facing16_between(
        [source.0, source.1],
        [current.0, current.1],
    );
    octant_start(away, rng)
}

/// Infantry51D2D9..51D390, the null-coordinate arm: the body facing when the
/// physical coordinate (vt+0x48) sits at its cell centre, else the heading
/// from the centre to it; rounded to an octant, then RandomRanged(0,4)-2.
/// The draw precedes the arm's nearby-cell search, so it is paid whether
/// or not the fallback runs.
pub(super) fn null_start_direction(
    current: (i32, i32),
    facing: u16,
    rng: &mut crate::sim::rng::SimRng,
) -> i32 {
    let within = [current.0 & 0xFF, current.1 & 0xFF];
    let heading = if within == [0x80, 0x80] {
        facing
    } else {
        crate::util::direction_tables::facing16_between([0x80, 0x80], within)
    };
    octant_start(heading, rng)
}

fn octant_start(heading: u16, rng: &mut crate::sim::rng::SimRng) -> i32 {
    let octant = (((u32::from(heading) >> 12) + 1) >> 1) as i32 & 7;
    rng.next_range_u32_inclusive(0, 4) as i32 - 2 + octant
}

/// `None` refuses entry, `Some(false)` retains a fallback, `Some(true)` selects
/// immediately. Native NullCell is (0,0), so a legal zero cell is not a result.
pub(super) fn select_neighbor<E>(
    seed: (i16, i16),
    start_direction: i32,
    mut inspect: impl FnMut((i16, i16), i32) -> Result<Option<bool>, E>,
) -> Result<Option<(i16, i16)>, E> {
    let mut fallback = (0, 0);
    for offset in 0..8 {
        let direction = (start_direction + offset) & 7;
        let (dx, dy) = crate::util::direction::DIRECTION_DELTAS[direction as usize];
        let candidate = (
            seed.0.wrapping_add(dx as i16),
            seed.1.wrapping_add(dy as i16),
        );
        let Some(preferred) = inspect(candidate, direction)? else {
            continue;
        };
        if fallback == (0, 0) {
            fallback = candidate;
        }
        if preferred {
            return Ok((candidate != (0, 0))
                .then_some(candidate)
                .or((fallback != (0, 0)).then_some(fallback)));
        }
    }
    Ok((fallback != (0, 0)).then_some(fallback))
}

/// Scatter51D62A uses the same 6D6410 projection as FNPC. Input Z is ignored
/// by that native projection. Only a direct candidate without structural
/// bridge flag100 is preferred; neither failure invalidates the fallback.
pub(super) fn preferred_surface(terrain: &ResolvedTerrainGrid, candidate: (i16, i16)) -> bool {
    let cells = NativeCellQuery::canonical(terrain);
    let coord = (i32::from(candidate.0), i32::from(candidate.1));
    if crate::sim::find_nearby_cell::project_world_coordinate_with_lookup(
        coord.0 * 256 + 128,
        coord.1 * 256 + 128,
        |x, y| cells.projection_view(x, y),
    ) != coord
    {
        return false;
    }
    let cell = cells.lookup(candidate);
    cells.flags(cell) & 0x100 == 0
}

#[cfg(test)]
#[path = "scatter_cell_tests.rs"]
mod tests;
