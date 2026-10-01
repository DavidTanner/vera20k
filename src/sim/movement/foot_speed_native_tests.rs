use super::*;
use serde_json::Value;

fn corpus() -> Value {
    serde_json::from_str(include_str!(
        "../../../tools/spatial_oracle/track_speed_native.json"
    ))
    .unwrap()
}

fn native64(value: &Value) -> NativeF64Bits {
    NativeF64Bits::from_bits(u64::from_str_radix(value.as_str().unwrap(), 16).unwrap())
}

fn integer(input: &Value, key: &str, fallback: i32) -> i32 {
    input[key].as_i64().map_or(fallback, |value| value as i32)
}

fn flag(input: &Value, key: &str, fallback: bool) -> bool {
    input[key].as_bool().unwrap_or(fallback)
}

fn getter_inputs(input: &Value) -> FootSpeedInputs {
    FootSpeedInputs {
        raw_type_speed: integer(input, "raw", 17),
        house_multiplier: NativeF32Bits::from_bits(
            u32::from_str_radix(input["house_bits"].as_str().unwrap(), 16).unwrap(),
        ),
        crate_multiplier: native64(&input["crate_bits"]),
        faster: flag(input, "faster", false),
        veteran_multiplier: native64(&input["veteran_bits"]),
        applied_fraction: native64(&input["applied_bits"]),
        unit_flag_carrier: integer(input, "flag_owner", -1) != -1,
    }
}

#[test]
fn live_getter_matches_original_staged_house_crate_veterancy_fraction_and_ctf() {
    let corpus = corpus();
    let cases = corpus["getters"].as_array().unwrap();
    assert_eq!(cases.len(), 75);
    for case in cases {
        assert_eq!(
            current_speed(getter_inputs(&case["input"])).unwrap(),
            case["output"].as_i64().unwrap() as i32,
            "{}",
            case["state"]
        );
    }
}
