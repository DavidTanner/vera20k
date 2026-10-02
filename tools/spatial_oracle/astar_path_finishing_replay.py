"""Check saved Rust replay hash-composition attribution, never native goldens.

The gameplay and RNG comparison remains tools.native_oracle.first_difference;
only the explicitly recorded observation normalization lives here.
"""
from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import json
import math
from pathlib import Path
import re

from tools.native_oracle import _canonical, first_difference
from tools.tactical_certification.core import (
    assert_snapshot_unchanged, parse_json_bytes, require_array, require_exact_keys,
    require_int, require_object, require_regular_file, require_sha256,
    require_string, require_value,
)

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
REPLAY_OBSERVATION_FIELDS = (
    "next_frame", "tick_result", "commands", "entities", "logic_order",
    "fire_events_accumulated", "lifecycle_outputs_accumulated", "rng", "draws",
    "retained_path_inputs",
)
HASH_COMPOSITION_NORMALIZATION = ["tick_result.state_hash only"]
MAX_RETAINED_INPUT_BYTES = 128 * 1024 * 1024


def rows(data: bytes, *, strict: bool = False) -> list[dict]:
    return [dict(parse_json_bytes(line, f"replay row {index}")) if strict else json.loads(line)
            for index, line in enumerate(data.splitlines())]


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


def _composition_object(value, label: str, required: tuple[str, ...],
                        optional: tuple[str, ...] = ()) -> dict:
    document = require_object(value, label)
    require_exact_keys(document, (*required, *(key for key in optional if key in document)), label)
    return document


def _composition_u64(value, label: str) -> int:
    number = require_int(value, label)
    if not 0 <= number < 1 << 64:
        raise ValueError(f"{label} must be a u64")
    return number


def check_hash_composition(receipt_path: Path) -> dict:
    """Attribute a recorded Building hash fold using one unchanged Rust binary.

    Fixture names, row counts and incoming pins come from the receipt. The only
    comparison projection is the existing tick-hash removal; no old actor fields,
    RNG caller positions or retained gameplay inputs are normalized. Execution
    receipts use the recorded producer's explicit legacy_building_hash field;
    inherited process environment is neither required nor exposed.
    """
    receipt_path = receipt_path.resolve()
    receipt_snapshot = require_regular_file(receipt_path, "hash-composition receipt")
    receipt = _composition_object(parse_json_bytes(receipt_snapshot.raw, "hash-composition receipt"),
        "hash-composition receipt", ("schema_version", "kind", "binary", "gate", "runs",
                                    "inputs", "incoming_main", "comparisons"),
        ("scope", "limits", "normalization"))
    require_value(receipt["schema_version"], 1, "hash-composition schema_version")
    require_value(receipt["kind"], "same-binary-rust-hash-composition", "hash-composition kind")
    if "scope" in receipt:
        require_string(receipt["scope"], "hash-composition scope")
    if "limits" in receipt:
        for limit in require_array(receipt["limits"], "hash-composition limits"):
            require_string(limit, "hash-composition limit")
    if "normalization" in receipt and first_difference(
            HASH_COMPOSITION_NORMALIZATION, receipt["normalization"]):
        raise ValueError("hash-composition normalization must remove only tick_result.state_hash")
    binary = _composition_object(receipt["binary"], "binary",
                                 ("manifest_file", "sha256", "source_sha256"), ("label",))
    require_sha256(binary["sha256"], "binary.sha256")
    require_sha256(binary["source_sha256"], "binary.source_sha256")
    if "label" in binary:
        require_string(binary["label"], "binary.label")
    gate = _composition_object(receipt["gate"], "gate", ("file", "sha256", "environment"),
                               ("effect", "removal_required_before_final_checks"))
    require_sha256(gate["sha256"], "gate.sha256")
    require_value(gate["environment"], "VERA20K_DIAGNOSTIC_LEGACY_BUILDING_HASH", "gate.environment")
    if "effect" in gate:
        require_string(gate["effect"], "gate.effect")
    if "removal_required_before_final_checks" in gate:
        require_value(gate["removal_required_before_final_checks"], True, "gate.removal_required_before_final_checks")
    runs = _composition_object(receipt["runs"], "runs", ("control", "current"))
    incoming = _composition_object(receipt["incoming_main"], "incoming_main", ("head", "pins"))
    head = require_string(incoming["head"], "incoming_main.head")
    if not re.fullmatch(r"[0-9a-f]{7,40}", head):
        raise ValueError("incoming_main.head must be a recorded hex commit identity")
    pins = require_object(incoming["pins"], "incoming_main.pins")
    if not pins or any(not fixture for fixture in pins):
        raise ValueError("incoming_main.pins must name nonempty fixture coverage")
    for fixture, pin in pins.items():
        _composition_u64(pin, f"incoming_main.pins.{fixture}")

    root = receipt_path.parent.resolve()
    raw_inputs, saved, input_paths = {}, {}, set()
    snapshots = [receipt_snapshot]
    for item in require_array(receipt["inputs"], "inputs"):
        item = _composition_object(item, "input", ("file", "sha256", "raw_sha256"), ("mode", "fixture"))
        name = require_string(item["file"], "input.file")
        path = (root / name).resolve()
        if (not name or Path(name).is_absolute() or ".." in Path(name).parts
                or not path.is_relative_to(root) or path in input_paths or path == receipt_path.resolve()):
            raise ValueError(f"input path escapes receipt or is duplicated: {name}")
        input_paths.add(path)
        snapshot = require_regular_file(path, f"input {name}", maximum_length=MAX_RETAINED_INPUT_BYTES)
        snapshots.append(snapshot)
        if snapshot.sha256 != require_sha256(item["sha256"], f"{name}.sha256"):
            raise ValueError(f"input SHA mismatch: {name}")
        if name.endswith(".gz"):
            with gzip.GzipFile(fileobj=io.BytesIO(snapshot.raw)) as stream:
                data = stream.read(MAX_RETAINED_INPUT_BYTES + 1)
        else:
            data = snapshot.raw
        if len(data) > MAX_RETAINED_INPUT_BYTES:
            raise ValueError(f"raw input exceeds retained byte budget: {name}")
        if hashlib.sha256(data).hexdigest() != require_sha256(item["raw_sha256"], f"{name}.raw_sha256"):
            raise ValueError(f"raw input SHA mismatch: {name}")
        raw_inputs[name] = data
        if ("mode" in item) != ("fixture" in item):
            raise ValueError("input mode and fixture must be supplied together")
        if "fixture" in item:
            fixture = require_string(item["fixture"], f"{name}.fixture")
            if item["mode"] not in ("control", "current") or fixture not in pins:
                raise ValueError(f"input mode/fixture is outside declared coverage: {name}")
            key = (item["mode"], fixture)
            if key in saved:
                raise ValueError(f"duplicate observations: {key}")
            saved[key] = rows(data, strict=True)
    if set(saved) != {(mode, fixture) for mode in runs for fixture in pins}:
        raise ValueError("saved observation sets differ from the declared control/current fixture coverage")

    def retained(name, label):
        name = require_string(name, label)
        if name not in raw_inputs:
            raise ValueError(f"{label} is not a hash-pinned retained input")
        return raw_inputs[name]

    if hashlib.sha256(retained(gate["file"], "gate.file")).hexdigest() != gate["sha256"]:
        raise ValueError("gate patch SHA mismatch")
    manifest = parse_json_bytes(retained(binary["manifest_file"], "binary.manifest_file"), "cargo label manifest")
    require_value(manifest.get("schema"), 1, "cargo label manifest.schema")
    source = require_object(manifest.get("source"), "cargo label manifest.source")
    if source.get("source_sha256") != binary["source_sha256"]:
        raise ValueError("manifest source identity differs")
    artifacts = require_array(manifest.get("artifacts"), "cargo label manifest.artifacts")
    if not any(require_object(artifact, "artifact").get("sha256") == binary["sha256"] for artifact in artifacts):
        raise ValueError("binary identity is absent from label manifest")
    executions = {}
    for mode, enabled in (("control", "1"), ("current", None)):
        run = _composition_object(runs[mode], f"runs.{mode}", ("receipt_file",))
        execution = _composition_object(
            parse_json_bytes(retained(run["receipt_file"], f"runs.{mode}.receipt_file"), f"{mode} execution"),
            f"{mode} execution", ("schema_version", "mode", "command", "cwd", "exit_code", "seconds",
                                  "binary_sha256", "binary_unchanged", "source_sha256", "legacy_building_hash"))
        require_value(execution["schema_version"], 1, f"{mode} execution.schema_version")
        require_value(execution["mode"], mode, f"{mode} execution.mode")
        if (execution["binary_sha256"] != binary["sha256"]
                or execution["source_sha256"] != binary["source_sha256"]
                or execution["binary_unchanged"] is not True):
            raise ValueError(f"{mode}: execution binary/source identity differs")
        if type(execution["legacy_building_hash"]) is not type(enabled) or execution["legacy_building_hash"] != enabled:
            raise ValueError(f"{mode}: execution gate identity differs")
        command = require_array(execution["command"], f"{mode}.command")
        if not command or any(not require_string(argument, f"{mode}.command argument") for argument in command):
            raise ValueError(f"{mode}: execution command is empty")
        cwd = require_string(execution["cwd"], f"{mode}.cwd")
        if not Path(command[0]).is_absolute() or not Path(cwd).is_absolute():
            raise ValueError(f"{mode}: execution command/cwd must be absolute")
        seconds = execution["seconds"]
        if type(seconds) not in (int, float) or not math.isfinite(seconds) or seconds < 0:
            raise ValueError(f"{mode}: execution seconds must be finite and nonnegative")
        exit_code = require_int(execution["exit_code"], f"{mode}.exit_code")
        if exit_code not in ((0,) if mode == "control" else (0, 101)):
            raise ValueError(f"{mode}: execution exit is outside the recorded Rust test attribution")
        executions[mode] = execution
    for field in ("command", "cwd"):
        if difference := first_difference(executions["control"][field], executions["current"][field]):
            raise ValueError(f"same-binary execution {field} differs: {difference}")

    results, fixtures = [], set()
    for expected in require_array(receipt["comparisons"], "comparisons"):
        expected = _composition_object(expected, "comparison", ("fixture", "rows", "control_hash", "current_hash"))
        fixture = require_string(expected["fixture"], "comparison.fixture")
        if fixture in fixtures or fixture not in pins:
            raise ValueError("comparison fixture is duplicated or outside incoming pin coverage")
        fixtures.add(fixture)
        count = require_int(expected["rows"], "comparison.rows")
        if count < 2:
            raise ValueError("comparison requires a seed boundary and committed tick")
        for field in ("control_hash", "current_hash"):
            _composition_u64(expected[field], f"comparison.{field}")
        control, current = saved["control", fixture], saved["current", fixture]
        for mode, observations in (("control", control), ("current", current)):
            if len(observations) != count:
                raise ValueError(f"{fixture}/{mode}: observation row count changed")
            for index, row in enumerate(observations):
                label = f"{fixture}/{mode}/{index}"
                require_exact_keys(row, REPLAY_OBSERVATION_FIELDS, label)
                require_exact_keys(require_object(row["rng"], f"{label}.rng"), ("scenario", "main", "mapgen"), f"{label}.rng")
                if (index == 0) != (row["tick_result"] is None):
                    raise ValueError(f"{label}: seed/tick boundary changed")
                if row["tick_result"] is not None:
                    tick = require_object(row["tick_result"], f"{label}.tick_result")
                    _composition_u64(tick.get("state_hash"), f"{label}.tick_result.state_hash")
        difference = first_difference(normalized(control, omit_tick_hash=True),
                                      normalized(current, omit_tick_hash=True))
        if difference:
            raise ValueError(f"{fixture}: control/current gameplay delta: {difference}")
        control_hash = control[-1]["tick_result"]["state_hash"]
        if control_hash != pins[fixture]:
            raise ValueError(f"{fixture}: control does not reproduce incoming pin")
        results.append(dict(fixture=fixture, rows=count, control_hash=control_hash,
                            current_hash=current[-1]["tick_result"]["state_hash"]))
    if fixtures != set(pins):
        raise ValueError("comparison fixture coverage differs from incoming pins")
    if first_difference(receipt["comparisons"], results):
        raise ValueError("comparison receipt changed")
    for snapshot in snapshots:
        assert_snapshot_unchanged(snapshot, str(snapshot.path))
    return {"scope": "Rust-only same-binary hash-composition attribution",
            "incoming_main": dict(incoming), "comparisons": results,
            "normalization": list(HASH_COMPOSITION_NORMALIZATION),
            "execution_exit_codes": {mode: execution["exit_code"] for mode, execution in executions.items()},
            "comparison_sha256": hashlib.sha256(_canonical(results)).hexdigest()}


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
                if set(row) != set(REPLAY_OBSERVATION_FIELDS) or set(row["rng"]) != {"scenario", "main", "mapgen"}:
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
    parser.add_argument("--hash-composition-receipt", type=Path,
                        help="check a fixture-driven Rust-only same-binary Building hash attribution")
    parser.add_argument("--normalize-final-caller-positions", action="store_true",
                        help="explicitly allow only RNG caller line/column changes in final ungated observations")
    args = parser.parse_args()
    if args.hash_composition_receipt is not None:
        if (args.main963_receipt is not None or args.current_observations is not None
                or args.normalize_final_caller_positions):
            parser.error("hash-composition receipt does not accept main963, fresh observations or caller normalization")
        result = check_hash_composition(args.hash_composition_receipt)
    elif args.main963_receipt is not None:
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
