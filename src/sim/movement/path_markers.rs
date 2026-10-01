//! Foot path-replay queue: the direction list a found path is installed as,
//! and the cursor its consumers advance or exhaust.

use crate::sim::components::FootPathQueue;
use crate::util::direction::TUBE_STEP_DIRECTION;

fn direction_from_step(from: (i16, i16), to: (u16, u16)) -> u8 {
    let dx = i32::from(to.0 as i16) - i32::from(from.0);
    let dy = i32::from(to.1 as i16) - i32::from(from.1);
    crate::util::direction::direction_from_delta(dx, dy).unwrap_or(TUBE_STEP_DIRECTION)
}

pub(super) fn install_path_replay(
    queue: &mut FootPathQueue,
    reference: (u16, u16),
    path: &[(u16, u16)],
    first_destination: usize,
) {
    let mut from = (reference.0 as i16, reference.1 as i16);
    queue.directions.clear();
    for &destination in path.iter().skip(first_destination) {
        queue
            .directions
            .push(direction_from_step(from, destination));
        from = (destination.0 as i16, destination.1 as i16);
    }
    queue.cursor = 0;
    queue.reference_cell = Some((reference.0 as i16, reference.1 as i16));
}

pub(super) fn accept_path_replay(
    queue: &mut FootPathQueue,
    endpoint: (i16, i16),
    consumed_directions: usize,
) {
    queue.reference_cell = Some(endpoint);
    consume_path_replay(queue, consumed_directions);
}

/// Accepted chain4B1DF7/6A143A and Drive tube4B1362..136E pop the queue
/// without rewriting Foot+558.
pub(super) fn consume_path_replay(queue: &mut FootPathQueue, consumed_directions: usize) {
    let cursor = usize::from(queue.cursor)
        .saturating_add(consumed_directions)
        .min(queue.directions.len());
    queue.cursor = cursor.min(u16::MAX as usize) as u16;
}

/// Walk75BD89..75BDB1 propagates a -1 head into the next word before
/// shifting. Retargeting an already-paid head must not expose its old suffix.
/// Original block comparisons: tools/spatial_oracle/walk_first_step.json.
pub(super) fn consume_walk_path_replay(queue: &mut FootPathQueue) {
    let invalidated = queue.remaining_directions().is_empty();
    consume_path_replay(queue, 1);
    if invalidated {
        queue.clear_live_head();
    }
}

/// Explicit owner abandonment, distinct from FootStop_Moving4DF0D0.
pub(super) fn exhaust_path_replay(queue: &mut FootPathQueue) {
    queue.cursor = queue.directions.len().min(u16::MAX as usize) as u16;
}
