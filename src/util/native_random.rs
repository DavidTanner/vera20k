//! The native random generator's state words and raw draw: a 250-word table
//! seeded by a four-round hash, drawn by XOR-ing the word at the lead cursor
//! with the word 0x67 slots ahead. `g_MapGenRng @ 0x00ABE890` (random-map
//! generation) and the scenario stream are the same machine; the generated
//! map hands its cursor on to the live scenario
//! (`rng_continuation::MapGenRngContinuation`).
//!
//! The state, raw draw and integer `RandomRanged` reduction live here, shared
//! by the scenario and presentation streams. `map::rmg::RmgRng` keeps its
//! distinct chop-53 `uniform` reduction.
//!
//! Native goldens: `tools/rmg_oracle/vectors/rng.json` (seeded state and raw
//! draws, `map::rmg::rng` tests) and the `sim::rng` seed and draw fixtures.

/// Number of state words.
pub(crate) const STATE_WORDS: usize = 250;
/// Distance between the two cursors; also the second cursor's start.
pub(crate) const LAG: usize = 0x67;

/// Seed-hash table mixed into each round's input.
const SEED_TABLE_1: [u32; 4] = [0xBAA9_6887, 0x1E17_D32C, 0x03BC_DC3C, 0x0F33_D1B2];
/// Seed-hash table XOR-ed into each round's rotated square sum. The original
/// table starts with one more word (`0x48AA_D7E4`) that its pre-incremented
/// index never reads.
const SEED_TABLE_2: [u32; 4] = [0x4B0F_3B58, 0xE874_F0C3, 0x6955_C5A6, 0x55A7_CA46];

/// The state words for `seed`. Word `i` starts from `i` and takes four hash
/// rounds; each round folds in the previous round's input, the first the
/// seed.
pub(crate) fn seeded_words(seed: u32) -> [u32; STATE_WORDS] {
    let mut words = [0; STATE_WORDS];
    for (index, word) in words.iter_mut().enumerate() {
        let mut previous = seed;
        let mut mixed = index as u32;
        for round in 0..SEED_TABLE_1.len() {
            let input = SEED_TABLE_1[round] ^ mixed;
            // Signed high half: the original shifts arithmetically.
            let high = (input as i32) >> 16;
            let low = input & 0xFFFF;
            let square_mix =
                (!high.wrapping_mul(high)).wrapping_add((low as i32).wrapping_mul(low as i32));
            let rotated = ((square_mix >> 16) as u32) | ((square_mix as u32) << 16);
            let cross = (high as u32).wrapping_mul(low);
            let next = (rotated ^ SEED_TABLE_2[round]).wrapping_add(cross) ^ previous;
            previous = mixed;
            mixed = next;
        }
        *word = mixed;
    }
    words
}

/// One raw draw: XOR the lagged pair into the word at `lead`, return it, then
/// advance both cursors with wraparound.
#[inline]
pub(crate) fn draw(words: &mut [u32], lead: &mut usize, lagged: &mut usize) -> u32 {
    let value = words[*lead] ^ words[*lagged];
    words[*lead] = value;
    *lead += 1;
    if *lead >= STATE_WORDS {
        *lead = 0;
    }
    *lagged += 1;
    if *lagged >= STATE_WORDS {
        *lagged = 0;
    }
    value
}

/// Signed `Random__RandomRanged @ 0x0065C7E0`: sort the bounds, then mask
/// and reject raw draws. The closure leaves stream ownership and tracing
/// with the caller; equal bounds never call it.
pub(crate) fn ranged_i32(low: i32, high: i32, draw: impl FnMut() -> u32) -> i32 {
    let (low, high) = if low <= high {
        (low, high)
    } else {
        (high, low)
    };
    low.wrapping_add(ranged_offset(high.wrapping_sub(low) as u32, draw) as i32)
}

/// Unsigned-bound variant used by the scenario owner. Its ordinary spans
/// share the exact reduction and draw count with the signed native helper.
pub(crate) fn ranged_u32(low: u32, high: u32, draw: impl FnMut() -> u32) -> u32 {
    let (low, high) = if low <= high {
        (low, high)
    } else {
        (high, low)
    };
    low.wrapping_add(ranged_offset(high.wrapping_sub(low), draw))
}

fn ranged_offset(span: u32, mut draw: impl FnMut() -> u32) -> u32 {
    if span == 0 {
        return 0;
    }
    // Retain the existing scenario owner's finite guard for native spans
    // whose mask construction cannot terminate. This is a custom-data
    // residual, not a native return value; retail audio ranges stay below it.
    if span >= 0x7FFF_FFFF {
        return 0x8000_0000;
    }
    // One bit wider than the span's highest bit, including power-of-two
    // spans: next_power_of_two()-1 would omit the inclusive upper endpoint.
    let mask = u32::MAX >> span.leading_zeros();
    loop {
        let sample = draw() & mask;
        if sample <= span {
            return sample;
        }
    }
}

/// Logical Random2Class bytes in the original oracle's 0x3F4-byte layout.
/// Shared only by regression observations; production state persistence stays
/// with each existing owner.
#[cfg(test)]
pub(crate) fn state_hex(disabled: u8, lead: i32, lagged: i32, words: &[u32]) -> String {
    let mut bytes = Vec::with_capacity(0x3f4);
    bytes.extend_from_slice(&u32::from(disabled).to_le_bytes());
    bytes.extend_from_slice(&lead.to_le_bytes());
    bytes.extend_from_slice(&lagged.to_le_bytes());
    for word in words {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
