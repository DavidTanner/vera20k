//! Deterministic integer `atan2_bam`. BAM angles are `u16`, 0 = +x (east),
//! 0x4000 = +y.
//!
//! This is VERA's own table, not gamemd's retail trig (`util::native_trig`).
//! The wave yaw uses it.

use std::sync::OnceLock;

use crate::util::fixed_math::SimFixed;

/// Compute the BAM heading from a delta vector via deterministic integer
/// arithmetic. 0 BAM = +x, 0x4000 = +y, 0x8000 = −x, 0xC000 = −y.
///
/// Reduces to the first octant via abs+min/max, looks up `atan(min/max)` in
/// `atan_lut`, then assembles the full circle from the quadrant signs and the
/// which-of-|x|,|y|-is-larger bit. The lookup is by integer ratio
/// `min * 65536 / max`, so no `f32` enters the per-tick computation.
pub(crate) fn atan2_bam(dy: SimFixed, dx: SimFixed) -> u16 {
    int_atan2_bam(dy.to_bits() as i64, dx.to_bits() as i64)
}

/// Integer atan2 → BAM. Inputs are arbitrary signed integers (Q-form is
/// irrelevant — atan2 depends only on the ratio).
fn int_atan2_bam(y: i64, x: i64) -> u16 {
    if x == 0 && y == 0 {
        return 0;
    }
    let ax = x.unsigned_abs();
    let ay = y.unsigned_abs();
    // First-octant angle: atan(min/max) in [0, 0x2000] BAM.
    let phi: u32 = if ay <= ax {
        atan_octant_bam(ay, ax) as u32
    } else {
        atan_octant_bam(ax, ay) as u32
    };
    // Quadrant base (each quadrant is 0x4000 BAM wide). Within each quadrant,
    // `in_high_half` selects which half of the quadrant the angle lies in — the
    // half closer to the next cardinal direction, where the BAM is computed by
    // subtracting `phi` from the next cardinal's BAM rather than adding it to
    // the quadrant base.
    let (base, in_high_half) = match (x >= 0, y >= 0) {
        (true, true) => (0x0000u32, ay > ax),
        (false, true) => (0x4000u32, ay <= ax),
        (false, false) => (0x8000u32, ay > ax),
        (true, false) => (0xC000u32, ay <= ax),
    };
    let bam = if in_high_half {
        base + 0x4000 - phi
    } else {
        base + phi
    };
    (bam & 0xFFFF) as u16
}

/// First-octant atan in BAM. `num` and `den` satisfy `0 <= num <= den`,
/// `den > 0`. Returns `round(atan(num/den) * 32768/π)` in `[0, 0x2000]`.
///
/// Uses the integer ratio `num * 65536 / den` (with round-to-nearest) as the
/// LUT index, so the only fp involved is in the one-shot table build.
fn atan_octant_bam(num: u64, den: u64) -> u16 {
    if den == 0 {
        return 0;
    }
    let scaled = num
        .checked_mul(65536)
        .expect("atan2 inputs exceed i48 range");
    let ratio = ((scaled + den / 2) / den).min(65536) as usize;
    atan_lut()[ratio]
}

/// First-octant atan LUT: `ATAN[r]` = `round(atan(r / 65536) * 32768/π)` for
/// `r ∈ [0, 65536]`. Output is BAM in `[0, 0x2000]` (u16). See
/// [`bam_cos_table`] for the platform-determinism rationale of the f64 init.
fn atan_lut() -> &'static [u16; 65537] {
    static TABLE: OnceLock<Box<[u16; 65537]>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut t: Box<[u16; 65537]> = vec![0u16; 65537]
            .into_boxed_slice()
            .try_into()
            .expect("65537-entry vec to fixed array");
        for i in 0..=65536u32 {
            let ratio = (i as f64) / 65536.0;
            let bam = (ratio.atan() * (32768.0 / std::f64::consts::PI)).round() as i32;
            t[i as usize] = bam.clamp(0, 0x2000) as u16;
        }
        t
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::fixed_math::SIM_ZERO;

    #[test]
    fn atan2_bam_cardinal_directions() {
        // Exact-cardinal inputs must hit exact-cardinal BAM with no slack.
        assert_eq!(atan2_bam(SimFixed::from_num(0), SimFixed::from_num(1)), 0); // +x
        assert_eq!(
            atan2_bam(SimFixed::from_num(1), SimFixed::from_num(0)),
            0x4000 // +y
        );
        assert_eq!(
            atan2_bam(SimFixed::from_num(0), SimFixed::from_num(-1)),
            0x8000 // -x
        );
        assert_eq!(
            atan2_bam(SimFixed::from_num(-1), SimFixed::from_num(0)),
            0xC000 // -y
        );
    }

    #[test]
    fn atan2_bam_diagonal_directions() {
        // Equal-magnitude diagonals must land on the octant midpoints.
        assert_eq!(
            atan2_bam(SimFixed::from_num(1), SimFixed::from_num(1)),
            0x2000 // NE in math = +x +y at 45°
        );
        assert_eq!(
            atan2_bam(SimFixed::from_num(1), SimFixed::from_num(-1)),
            0x6000
        );
        assert_eq!(
            atan2_bam(SimFixed::from_num(-1), SimFixed::from_num(-1)),
            0xA000
        );
        assert_eq!(
            atan2_bam(SimFixed::from_num(-1), SimFixed::from_num(1)),
            0xE000
        );
    }

    #[test]
    fn atan2_bam_zero_zero_is_zero() {
        assert_eq!(atan2_bam(SIM_ZERO, SIM_ZERO), 0);
    }

    #[test]
    fn atan2_bam_matches_f64_reference() {
        // Sample 32 evenly-spaced cell deltas around the unit circle and
        // check the integer LUT result is within ±1 BAM of an f64 reference.
        // ±1 BAM tolerance covers the rounding budget of the table build.
        for n in 0..32 {
            let angle = (n as f64) * (2.0 * std::f64::consts::PI / 32.0);
            // Scale up to avoid the small-magnitude rounding band — atan2 is
            // ratio-only so any radius works.
            let dx = (angle.cos() * 1024.0) as i32;
            let dy = (angle.sin() * 1024.0) as i32;
            if dx == 0 && dy == 0 {
                continue;
            }
            let got = atan2_bam(SimFixed::from_num(dy), SimFixed::from_num(dx));
            let expected = (((dy as f64).atan2(dx as f64) * 32768.0 / std::f64::consts::PI) as i32)
                .rem_euclid(65536) as u16;
            let diff = (got as i32 - expected as i32).rem_euclid(65536);
            let signed_diff = diff.min(65536 - diff);
            assert!(
                signed_diff <= 1,
                "angle={:.2}rad (dx={},dy={}): got 0x{:04X}, expected 0x{:04X}",
                angle,
                dx,
                dy,
                got,
                expected
            );
        }
    }

    #[test]
    fn atan2_bam_sub_cell_inputs() {
        // SimFixed deltas can be sub-cell. Verify the LUT handles them by
        // their Q16.16 bit representation rather than truncating to integers.
        // (1.5, 0) → +x → 0 BAM
        assert_eq!(atan2_bam(SimFixed::lit("0"), SimFixed::lit("1.5")), 0);
        // (0.5, 0.5) → 45° → 0x2000 BAM
        assert_eq!(
            atan2_bam(SimFixed::lit("0.5"), SimFixed::lit("0.5")),
            0x2000
        );
    }
}
