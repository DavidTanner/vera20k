//! Deterministic BAM trig: `cos_bam`/`sin_bam` (Q16.16 tables) and an integer
//! `atan2_bam`. BAM angles are `u16`, 0 = +x (east), 0x4000 = +y.
//!
//! These are VERA's own tables, not gamemd's retail sine table
//! (`util::native_trig`). The Hover integrator and the wave yaw use them.

use std::sync::OnceLock;

use crate::util::fixed_math::SimFixed;

/// Q16.16 cosine table indexed by BAM angle. `BAM_COS[i]` is `cos(i * 2π/65536)`
/// quantized to i32 Q16.16. Exact 1-BAM resolution, no interpolation.
///
/// Built once at first call from f64 trig and frozen as integers; the rounding
/// step absorbs the sub-ULP differences between platforms' libm so the table
/// is bitwise identical across machines. Boxed to keep the 256 KB array off
/// the stack during init.
fn bam_cos_table() -> &'static [i32; 65536] {
    static TABLE: OnceLock<Box<[i32; 65536]>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut t: Box<[i32; 65536]> = vec![0i32; 65536]
            .into_boxed_slice()
            .try_into()
            .expect("65536-entry vec to fixed array");
        for i in 0..65536u32 {
            let angle = (i as f64) * (2.0 * std::f64::consts::PI / 65536.0);
            t[i as usize] = (angle.cos() * 65536.0).round() as i32;
        }
        t
    })
}

/// Q16.16 sine table; see `bam_cos_table` for the determinism rationale.
fn bam_sin_table() -> &'static [i32; 65536] {
    static TABLE: OnceLock<Box<[i32; 65536]>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut t: Box<[i32; 65536]> = vec![0i32; 65536]
            .into_boxed_slice()
            .try_into()
            .expect("65536-entry vec to fixed array");
        for i in 0..65536u32 {
            let angle = (i as f64) * (2.0 * std::f64::consts::PI / 65536.0);
            t[i as usize] = (angle.sin() * 65536.0).round() as i32;
        }
        t
    })
}

/// Deterministic `cos(bam)` as `SimFixed`. O(1) table lookup.
#[inline]
pub(crate) fn cos_bam(bam: u16) -> SimFixed {
    SimFixed::from_bits(bam_cos_table()[bam as usize])
}

/// Deterministic `sin(bam)` as `SimFixed`. O(1) table lookup.
#[inline]
pub(crate) fn sin_bam(bam: u16) -> SimFixed {
    SimFixed::from_bits(bam_sin_table()[bam as usize])
}

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
    use crate::util::fixed_math::{SIM_ONE, SIM_ZERO};

    #[test]
    fn cos_bam_cardinal_angles() {
        // 0 BAM = 0 rad: cos = 1
        assert_eq!(cos_bam(0), SIM_ONE);
        // 0x4000 BAM = π/2: cos ≈ 0
        let c = cos_bam(0x4000);
        assert!(c.abs() <= SimFixed::from_bits(2), "cos(π/2) = {:?}", c);
        // 0x8000 BAM = π: cos = -1
        assert_eq!(cos_bam(0x8000), SimFixed::from_num(-1));
        // 0xC000 BAM = 3π/2: cos ≈ 0
        let c = cos_bam(0xC000);
        assert!(c.abs() <= SimFixed::from_bits(2), "cos(3π/2) = {:?}", c);
    }

    #[test]
    fn sin_bam_cardinal_angles() {
        // 0 BAM: sin = 0
        assert_eq!(sin_bam(0), SIM_ZERO);
        // 0x4000 BAM = π/2: sin = 1
        assert_eq!(sin_bam(0x4000), SIM_ONE);
        // 0x8000 BAM = π: sin ≈ 0
        let s = sin_bam(0x8000);
        assert!(s.abs() <= SimFixed::from_bits(2), "sin(π) = {:?}", s);
        // 0xC000 BAM = 3π/2: sin = -1
        assert_eq!(sin_bam(0xC000), SimFixed::from_num(-1));
    }

    #[test]
    fn bam_trig_pythagorean_identity() {
        // cos² + sin² ≈ 1 for arbitrary BAMs. Q16.16 rounding budget is a
        // few ULPs per multiplication.
        for &bam in &[0u16, 0x1234, 0x2BCD, 0x4001, 0x7FFF, 0xABCD, 0xFFFF] {
            let c = cos_bam(bam);
            let s = sin_bam(bam);
            let sum = c * c + s * s;
            let err = (sum - SIM_ONE).abs();
            assert!(
                err <= SimFixed::lit("0.001"),
                "bam={:#06X}: cos²+sin² = {:?} (err {:?})",
                bam,
                sum,
                err
            );
        }
    }

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
