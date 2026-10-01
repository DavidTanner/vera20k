//! Exact x87 leaf of the Foot speed getter (Foot 0x004DB1A0), executed against
//! tools/spatial_oracle/track_speed_native.json.
//!
//! RESIDUAL (unfinished migration): this leaf is test-only. The production
//! getter (`foot_speed::owner_current_speed_from_fraction`) has no house
//! speed factor or CTF flag-carrier halving yet; when it gains them, its rows
//! move onto it and this leaf goes. The Drive/Ship speed prefix and braking
//! distance this file used to hold are ported once, in
//! `drive_locomotion::apply_track_speed_prefix` and `track_speed`.

use crate::util::native_x87::{NativeF32Bits, NativeF64Bits};
use crate::util::native_x87::{NativeX87Error, X87Chop53 as X};

#[derive(Clone, Copy, Debug)]
pub(crate) struct FootSpeedInputs {
    pub raw_type_speed: i32,
    pub house_multiplier: NativeF32Bits,
    pub crate_multiplier: NativeF64Bits,
    pub faster: bool,
    pub veteran_multiplier: NativeF64Bits,
    pub applied_fraction: NativeF64Bits,
    pub unit_flag_carrier: bool,
}

/// Foot4DB1A0 consumes low32 after each native signed64 ftol, then optionally
/// halves the signed final i32. Each operand is a live getter input.
pub(crate) fn current_speed(input: FootSpeedInputs) -> Result<i32, NativeX87Error> {
    let house = X::load_f32(input.house_multiplier)?;
    let house = X::load_f64(X::store_f64(house)?)?;
    let product = X::mul(X::load_i32(input.raw_type_speed), house);
    let product = X::mul(product, X::load_f64(input.crate_multiplier)?);
    let mut stage = X::ftol_i64(product)? as i32;
    if input.faster {
        stage = X::ftol_i64(X::mul(
            X::load_i32(stage),
            X::load_f64(input.veteran_multiplier)?,
        ))? as i32;
    }
    stage = X::ftol_i64(X::mul(
        X::load_i32(stage),
        X::load_f64(input.applied_fraction)?,
    ))? as i32;
    Ok(if input.unit_flag_carrier {
        stage / 2
    } else {
        stage
    })
}

#[cfg(test)]
#[path = "foot_speed_native_tests.rs"]
mod tests;
