"""Check saved Rust replay hash-composition attribution, never native goldens.

The gameplay and RNG comparison remains tools.native_oracle.first_difference;
only the explicitly recorded observation normalization lives here.
"""
from __future__ import annotations

import argparse
import gzip
import hashlib
import json
from pathlib import Path
import re

from tools.native_oracle import _canonical, first_difference

EVIDENCE = Path(__file__).with_suffix("")
SOURCE_LOCATION = re.compile(r"(\.rs):\d+:\d+")
MAIN963_FIXTURES = {
    "bridge": (201, 0xCC11DDD91928916A),
    "global": (601, 0xEB5433C611607535),
    "slice6": (17, 0x5BA5F8CBB5F72FF5),
}
MAIN963_OMITTED_FEEDS = [
    "retained-navigation-history", "house-spatial-threat",
    "foot-530-coefficient", "techno-508-contribution",
]


def rows(data: bytes) -> list[dict]:
    return [json.loads(line) for line in data.splitlines()]


def normalized(observations: list[dict], *, old_fields: bool = False,
               omit_tick_hash: bool = False, callers: bool = False) -> list[dict]:
    result = json.loads(_canonical(observations))
    for row in result:
        if old_fields:
            row.pop("retained_path_inputs", None)
            for actor in row["entities"]:
                actor.pop("cached_spatial_threat", None)
                actor["navigation"].pop("threat_avoidance_coefficient", None)
        if omit_tick_hash and row["tick_result"] is not None:
            row["tick_result"].pop("state_hash")
        if callers:
            for draw in row["draws"]:
                draw["callers"] = SOURCE_LOCATION.sub(r"\1:<line>:<column>", draw["callers"])
    return result


def check(current_observations: Path | None = None) -> dict:
    receipt = json.loads((EVIDENCE / "receipt.json").read_bytes())
    saved = {}
    # Hash the original raw bytes as well as their deterministic gzip envelope.
    # A changed observation, provenance manifest or validation log fails closed.
    for item in receipt["inputs"]:
        compressed = (EVIDENCE / item["file"]).read_bytes()
        if hashlib.sha256(compressed).hexdigest() != item["sha256"]:
            raise ValueError(f"input SHA mismatch: {item['file']}")
        data = gzip.decompress(compressed) if item["file"].endswith(".gz") else compressed
        if hashlib.sha256(data).hexdigest() != item["raw_sha256"]:
            raise ValueError(f"raw input SHA mismatch: {item['file']}")
        if item.get("fixture"):
            saved[item["mode"], item["fixture"]] = rows(data)
    results = []
    for expected in receipt["comparisons"]:
        fixture = expected["fixture"]
        main = saved["main", fixture]
        legacy = saved["candidate-legacy", fixture]
        current = saved["candidate-current", fixture]
        for name, observations in (("main", main), ("legacy", legacy), ("current", current)):
            if len(observations) != expected["rows"]:
                raise ValueError(f"{fixture}/{name}: row count changed")
        difference = first_difference(
            normalized(main, old_fields=True, callers=True),
            normalized(legacy, old_fields=True, callers=True))
        if difference:
            raise ValueError(f"{fixture} main/legacy: {difference}")
        difference = first_difference(normalized(legacy, omit_tick_hash=True),
                                      normalized(current, omit_tick_hash=True))
        if difference:
            raise ValueError(f"{fixture} current/legacy: {difference}")
        for name, observations in (("legacy", legacy), ("current", current)):
            for row in observations:
                inputs = row["retained_path_inputs"]
                if not inputs["navigation_present"] or inputs["team_count"] != 0:
                    raise ValueError(f"{fixture}/{name}: navigation/Team prestate changed")
                if any(not house["spatial_grid_all_zero"] for house in inputs["houses"]):
                    raise ValueError(f"{fixture}/{name}: nonzero House threat grid")
                for actor in inputs["actors"]:
                    if actor["coefficient_bits"] != 0 or actor["cached_contribution"] != 0 or actor["team_override"]:
                        raise ValueError(f"{fixture}/{name}: retained threat input changed")
        result = dict(fixture=fixture, rows=len(current),
                      legacy_hash=legacy[-1]["tick_result"]["state_hash"],
                      current_hash=current[-1]["tick_result"]["state_hash"])
        if first_difference(expected, result):
            raise ValueError(f"{fixture}: final hash receipt changed")
        if main[-1]["tick_result"]["state_hash"] != result["legacy_hash"]:
            raise ValueError(f"{fixture}: legacy did not reproduce main pin")
        if current_observations is not None:
            fresh = rows((current_observations / f"{fixture}.jsonl").read_bytes())
            difference = first_difference(normalized(current, callers=True),
                                          normalized(fresh, callers=True))
            if difference:
                raise ValueError(f"{fixture} fresh current: {difference}")
        results.append(result)
    return {"scope": "Rust-only composition attribution", "comparisons": results,
            "comparison_sha256": hashlib.sha256(_canonical(results)).hexdigest()}


def compare_final_observations(saved: dict, data_by_fixture: dict[str, bytes],
                              normalize_callers: bool) -> list[dict]:
    results = []
    for fixture in MAIN963_FIXTURES:
        data = data_by_fixture[fixture]
        fresh = rows(data)
        captured = saved["current", fixture]
        difference = first_difference(normalized(captured, callers=normalize_callers),
                                      normalized(fresh, callers=normalize_callers))
        if difference:
            raise ValueError(f"main963/{fixture}: final ungated replay changed: {difference}")
        caller_changes = sum(a["callers"] != b["callers"]
                             for old, new in zip(captured, fresh)
                             for a, b in zip(old["draws"], new["draws"]))
        results.append(dict(fixture=fixture, rows=len(fresh),
                            raw_sha256=hashlib.sha256(data).hexdigest(),
                            caller_positions_changed=caller_changes))
    return results


def check_main963(receipt_path: Path, current_observations: Path | None = None,
                  normalize_final_callers: bool = False) -> dict:
    """Strict same-binary control; never normalize incoming movement state.

    This separate receipt preserves the historical main7412 contract. It can
    establish recorded control/current equality and reproduction of incoming
    final pins, not equality to unrecorded main963 per-frame observations.
    """
    receipt = json.loads(receipt_path.read_bytes())
    if receipt["schema_version"] != 1:
        raise ValueError("main963 receipt schema changed")
    if receipt["gate"]["environment"] != "VERA20K_DIAGNOSTIC_LEGACY_PATH_HASH":
        raise ValueError("main963 control gate identity changed")
    if receipt["gate"]["omitted_feeds"] != MAIN963_OMITTED_FEEDS:
        raise ValueError("main963 control must omit only the four declared feeds")
    saved, raw_inputs, observation_data = {}, {}, {}
    root = receipt_path.parent.resolve()
    for item in receipt["inputs"]:
        path = (root / item["file"]).resolve()
        if not path.is_relative_to(root) or item["file"] in raw_inputs:
            raise ValueError("main963 input path escapes receipt or is duplicated")
        compressed = path.read_bytes()
        if hashlib.sha256(compressed).hexdigest() != item["sha256"]:
            raise ValueError(f"main963 input SHA mismatch: {item['file']}")
        data = gzip.decompress(compressed) if item["file"].endswith(".gz") else compressed
        if hashlib.sha256(data).hexdigest() != item["raw_sha256"]:
            raise ValueError(f"main963 raw input SHA mismatch: {item['file']}")
        raw_inputs[item["file"]] = data
        if item.get("fixture"):
            key = (item["mode"], item["fixture"])
            if key in saved:
                raise ValueError(f"main963 duplicate observations: {key}")
            saved[key] = rows(data)
            observation_data[key] = data
    expected_keys = {(mode, fixture) for mode in ("control", "current")
                     for fixture in MAIN963_FIXTURES}
    if "final_ungated" in receipt:
        expected_keys |= {("final", fixture) for fixture in MAIN963_FIXTURES}
    followups = receipt.get("final_followups", [])
    followup_modes = [item["mode"] for item in followups]
    if (len(set(followup_modes)) != len(followup_modes)
            or any(mode in ("control", "current", "final") for mode in followup_modes)):
        raise ValueError("integrated final observation modes are duplicated or reserved")
    expected_keys |= {(mode, fixture) for mode in followup_modes
                      for fixture in MAIN963_FIXTURES}
    if set(saved) != expected_keys:
        raise ValueError("main963 saved observation sets differ from the declared pair/final coverage")
    gate = receipt["gate"]
    if hashlib.sha256(raw_inputs[gate["file"]]).hexdigest() != gate["sha256"]:
        raise ValueError("main963 gate patch SHA mismatch")
    binary = receipt["binary"]
    manifest = json.loads(raw_inputs[binary["manifest_file"]])
    if not re.fullmatch(r"[0-9a-f]{64}", binary["source_sha256"]):
        raise ValueError("main963 exact source identity is required")
    if manifest["source"]["source_sha256"] != binary["source_sha256"]:
        raise ValueError("main963 manifest source identity differs")
    if binary["sha256"] not in {artifact["sha256"] for artifact in manifest["artifacts"]}:
        raise ValueError("main963 binary identity is absent from label manifest")
    for mode, enabled in (("control", "1"), ("current", None)):
        run = receipt["runs"][mode]
        if (run["binary_sha256"] != binary["sha256"]
                or run["source_sha256"] != binary["source_sha256"]
                or run["legacy_path_hash"] != enabled):
            raise ValueError(f"main963/{mode}: same-binary or mode identity differs")
        # Each separately recorded command/exit receipt must itself be pinned.
        execution = json.loads(raw_inputs[run["receipt_file"]])
        if (execution["binary_sha256"] != binary["sha256"]
                or execution["binary_unchanged"] is not True):
            raise ValueError(f"main963/{mode}: execution binary receipt differs")
    results = []
    for fixture, (count, incoming_pin) in MAIN963_FIXTURES.items():
        control, current = saved["control", fixture], saved["current", fixture]
        if len(control) != count or len(current) != count:
            raise ValueError(f"main963/{fixture}: observation row count changed")
        # The same binary has the same caller positions. Compare everything
        # except the intentionally suppressed tick hash, with no other masks.
        difference = first_difference(normalized(control, omit_tick_hash=True),
                                      normalized(current, omit_tick_hash=True))
        if difference:
            raise ValueError(f"main963/{fixture}: control/current gameplay delta: {difference}")
        for mode, observations in (("control", control), ("current", current)):
            for index, row in enumerate(observations):
                required = {"next_frame", "tick_result", "commands", "entities",
                            "logic_order", "fire_events_accumulated",
                            "lifecycle_outputs_accumulated", "rng", "draws",
                            "retained_path_inputs"}
                if set(row) != required or set(row["rng"]) != {"scenario", "main", "mapgen"}:
                    raise ValueError(f"main963/{fixture}/{mode}: diagnostic coverage changed")
                inputs = row["retained_path_inputs"]
                if not inputs["navigation_present"] or inputs["team_count"] != 0:
                    raise ValueError(f"main963/{fixture}/{mode}: navigation/Team prestate changed")
                if any(not house["spatial_grid_all_zero"] for house in inputs["houses"]):
                    raise ValueError(f"main963/{fixture}/{mode}: nonzero House grid")
                if len(inputs["actors"]) != len(row["entities"]):
                    raise ValueError(f"main963/{fixture}/{mode}: actor provenance incomplete")
                for actor in inputs["actors"]:
                    if (actor["coefficient_bits"] != 0 or actor["cached_contribution"] != 0
                            or actor["team_override"]):
                        raise ValueError(f"main963/{fixture}/{mode}: retained threat prestate changed")
                if (index == 0) != (row["tick_result"] is None):
                    raise ValueError(f"main963/{fixture}/{mode}: seed/tick boundary changed")
        control_hash = control[-1]["tick_result"]["state_hash"]
        if control_hash != incoming_pin:
            raise ValueError(f"main963/{fixture}: control does not reproduce incoming pin")
        results.append(dict(fixture=fixture, rows=count, control_hash=control_hash,
                            current_hash=current[-1]["tick_result"]["state_hash"]))
    if first_difference(receipt["comparisons"], results):
        raise ValueError("main963 comparison receipt changed")
    result = {"scope": "Rust-only main963 same-binary composition attribution",
            "comparisons": results,
            "comparison_sha256": hashlib.sha256(_canonical(results)).hexdigest()}
    def saved_final(final: dict, mode: str) -> dict:
        final_binary = final["binary"]
        final_manifest = json.loads(raw_inputs[final_binary["manifest_file"]])
        if (not re.fullmatch(r"[0-9a-f]{64}", final_binary["source_sha256"])
                or final_manifest["source"]["source_sha256"] != final_binary["source_sha256"]
                or final_binary["sha256"] not in
                {artifact["sha256"] for artifact in final_manifest["artifacts"]}):
            raise ValueError("main963 final labelled source/binary identity differs")
        execution = json.loads(raw_inputs[final["receipt_file"]])
        if (execution["binary_sha256"] != final_binary["sha256"]
                or execution["source_sha256"] != final_binary["source_sha256"]
                or execution["binary_unchanged"] is not True or execution["exit_code"] != 0):
            raise ValueError("main963 final ungated execution identity/result differs")
        allowed_normalization = ["draw.callers .rs line/column only"]
        if final["normalization"] not in ([], allowed_normalization):
            raise ValueError("main963 final normalization can only change caller line/column")
        final_results = compare_final_observations(
            saved, {f: observation_data[mode, f] for f in MAIN963_FIXTURES},
            bool(final["normalization"]))
        if first_difference(final["observations"], final_results):
            raise ValueError("main963 saved final observation receipt changed")
        return dict(observations=final_results, normalization=final["normalization"], identity={
            "source_sha256": final_binary["source_sha256"],
            "binary_sha256": final_binary["sha256"],
        })
    if "final_ungated" in receipt:
        final_result = saved_final(receipt["final_ungated"], "final")
        result["saved_final_observations"] = final_result["observations"]
        result["saved_final_normalization"] = final_result["normalization"]
        result["saved_final_identity"] = final_result["identity"]
    if followups:
        result["integrated_final_followups"] = [
            dict(mode=item["mode"], **saved_final(item, item["mode"]))
            for item in followups]
    if current_observations is not None:
        fresh_results = compare_final_observations(
            saved, {f: (current_observations / f"{f}.jsonl").read_bytes()
                    for f in MAIN963_FIXTURES}, normalize_final_callers)
        result["final_observations"] = fresh_results
        result["final_normalization"] = (["draw.callers .rs line/column only"]
                                         if normalize_final_callers else [])
    elif normalize_final_callers:
        raise ValueError("final caller normalization requires final observations")
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", required=True)
    parser.add_argument("--current-observations", type=Path,
                        help="also compare a fresh VERA20K_REPLAY_DIAGNOSTICS directory")
    parser.add_argument("--main963-receipt", type=Path,
                        help="check a separate strict main963 same-binary receipt")
    parser.add_argument("--normalize-final-caller-positions", action="store_true",
                        help="explicitly allow only RNG caller line/column changes in final ungated observations")
    args = parser.parse_args()
    if args.main963_receipt is not None:
        result = check_main963(args.main963_receipt, args.current_observations,
                               args.normalize_final_caller_positions)
    else:
        if args.normalize_final_caller_positions:
            parser.error("explicit final caller normalization requires a main963 receipt")
        result = check(args.current_observations)
        result["main963"] = check_main963(EVIDENCE / "main963" / "receipt.json")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
