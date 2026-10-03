"""Check saved Rust replay hash-composition attribution, never native goldens.

The gameplay and RNG comparison remains tools.native_oracle.first_difference;
only the explicitly recorded observation normalization lives here.
"""
from __future__ import annotations

import argparse
import ast
from collections import Counter
import difflib
import gzip
import hashlib
import io
import json
import math
from pathlib import Path, PurePosixPath, PureWindowsPath
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
HASH_COMPOSITION_GATE_FIELDS = {
    "VERA20K_DIAGNOSTIC_LEGACY_BUILDING_HASH": "legacy_building_hash",
    "VERA20K_DIAGNOSTIC_LEGACY_UNIT_DEPLOY_HASH": "legacy_unit_deploy_hash",
    "VERA20K_BARRACKS_HASH_CONTROL": "barracks_hash_control",
}
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


def _retained_replay_inputs(receipt_path: Path, inputs: list, modes: tuple[str, ...],
                            fixtures: dict) -> tuple[dict, dict, list]:
    """One bounded byte/identity owner for saved replay-comparison inputs."""
    root = receipt_path.parent.resolve()
    raw_inputs, saved, input_paths, snapshots = {}, {}, set(), []
    for item in require_array(inputs, "inputs"):
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
            if item["mode"] not in modes or fixture not in fixtures:
                raise ValueError(f"input mode/fixture is outside declared coverage: {name}")
            key = (item["mode"], fixture)
            if key in saved:
                raise ValueError(f"duplicate observations: {key}")
            saved[key] = rows(data, strict=True)
    return raw_inputs, saved, snapshots


def check_hash_composition(receipt_path: Path) -> dict:
    """Attribute recorded hash folds using one unchanged Rust binary.

    Fixture names, row counts and incoming pins come from the receipt. The only
    comparison projection is the existing tick-hash removal; no old actor fields,
    RNG caller positions or retained gameplay inputs are normalized. Execution
    receipts name the recorded producer's explicit diagnostic gate field;
    inherited process environment is neither required nor exposed. The Unit
    deployment control switches only the expected final pins, so both runs must
    pass. Historical Building receipts retain their original exit contract.
    The explicitly unlabelled barracks variant retains Cargo logs and exact
    hash-control leaves while refusing to claim an unavailable whole-source
    identity, per-run binary capture or labelled build manifest.
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
    unlabelled = "identity_file" in require_object(receipt["binary"], "binary")
    binary_fields = (("identity_file", "sha256", "source_sha256", "whole_source_identity_available")
                     if unlabelled else ("manifest_file", "sha256", "source_sha256"))
    binary = _composition_object(receipt["binary"], "binary", binary_fields, ("label",))
    require_sha256(binary["sha256"], "binary.sha256")
    if unlabelled:
        require_value(binary["source_sha256"], None, "unlabelled binary.source_sha256")
        require_value(binary["whole_source_identity_available"], False,
                      "unlabelled binary.whole_source_identity_available")
    else:
        require_sha256(binary["source_sha256"], "binary.source_sha256")
    if "label" in binary:
        require_string(binary["label"], "binary.label")
    gate = _composition_object(receipt["gate"], "gate", ("file", "sha256", "environment"),
                               ("effect", "removal_required_before_final_checks"))
    require_sha256(gate["sha256"], "gate.sha256")
    gate_environment = require_string(gate["environment"], "gate.environment")
    if gate_environment not in HASH_COMPOSITION_GATE_FIELDS:
        raise ValueError("gate.environment is not a supported hash-composition control")
    if unlabelled and gate_environment != "VERA20K_BARRACKS_HASH_CONTROL":
        raise ValueError("unlabelled diagnostic identity supports only the recorded barracks hash control")
    gate_field = HASH_COMPOSITION_GATE_FIELDS[gate_environment]
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

    raw_inputs, saved, input_snapshots = _retained_replay_inputs(
        receipt_path, receipt["inputs"], tuple(runs), pins)
    snapshots = [receipt_snapshot, *input_snapshots]
    if set(saved) != {(mode, fixture) for mode in runs for fixture in pins}:
        raise ValueError("saved observation sets differ from the declared control/current fixture coverage")

    def retained(name, label):
        name = require_string(name, label)
        if name not in raw_inputs:
            raise ValueError(f"{label} is not a hash-pinned retained input")
        return raw_inputs[name]

    if hashlib.sha256(retained(gate["file"], "gate.file")).hexdigest() != gate["sha256"]:
        raise ValueError("gate patch SHA mismatch")
    if unlabelled:
        identity = _composition_object(parse_json_bytes(
            retained(binary["identity_file"], "binary.identity_file"), "unlabelled binary identity"),
            "unlabelled binary identity", ("schema_version", "kind", "recorded_path", "binary_sha256",
                                           "cargo_manifest_available", "whole_source_identity_available",
                                           "source_leaf", "limits"))
        require_value(identity["schema_version"], 1, "unlabelled binary schema_version")
        require_value(identity["kind"], "unlabelled-diagnostic-binary-identity", "unlabelled binary kind")
        require_value(identity["binary_sha256"], binary["sha256"], "unlabelled binary SHA")
        require_value(identity["cargo_manifest_available"], False, "unlabelled cargo_manifest_available")
        require_value(identity["whole_source_identity_available"], False,
                      "unlabelled whole_source_identity_available")
        recorded_binary = require_string(identity["recorded_path"], "unlabelled recorded_path")
        if not (PurePosixPath(recorded_binary).is_absolute() or PureWindowsPath(recorded_binary).is_absolute()):
            raise ValueError("unlabelled recorded binary path must be absolute")
        limits = require_array(identity["limits"], "unlabelled identity limits")
        if not limits or any(not require_string(limit, "unlabelled identity limit") for limit in limits):
            raise ValueError("unlabelled identity must retain its evidence limits")
        leaf = _composition_object(identity["source_leaf"], "unlabelled source leaf",
            ("path", "before_file", "before_sha256", "temporary_file", "temporary_sha256", "restored_sha256"))
        require_value(leaf["path"], "src/sim/world/world_hash.rs", "unlabelled source leaf.path")
        for phase in ("before", "temporary"):
            data = retained(leaf[f"{phase}_file"], f"source leaf.{phase}_file")
            if hashlib.sha256(data).hexdigest() != require_sha256(
                    leaf[f"{phase}_sha256"], f"source leaf.{phase}_sha256"):
                raise ValueError(f"unlabelled {phase} source leaf identity differs")
        require_value(leaf["restored_sha256"], leaf["before_sha256"], "unlabelled restored source identity")
        patch = "".join(difflib.unified_diff(
            retained(leaf["before_file"], "before source").decode("utf-8").splitlines(keepends=True),
            retained(leaf["temporary_file"], "temporary source").decode("utf-8").splitlines(keepends=True),
            fromfile="a/" + leaf["path"], tofile="b/" + leaf["path"])).encode()
        if patch != retained(gate["file"], "gate.file"):
            raise ValueError("unlabelled gate patch differs from its exact source leaves")
    else:
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
                                  "binary_sha256", "binary_unchanged", "source_sha256", gate_field,
                                  *(("log_file",) if unlabelled else ())))
        require_value(execution["schema_version"], 1, f"{mode} execution.schema_version")
        require_value(execution["mode"], mode, f"{mode} execution.mode")
        if (execution["binary_sha256"] != binary["sha256"]
                or execution["source_sha256"] != binary["source_sha256"]
                or execution["binary_unchanged"] is not True):
            raise ValueError(f"{mode}: execution binary/source identity differs")
        expected_gate = (mode == "control") if unlabelled else enabled
        if type(execution[gate_field]) is not type(expected_gate) or execution[gate_field] != expected_gate:
            raise ValueError(f"{mode}: execution gate identity differs")
        command = require_array(execution["command"], f"{mode}.command")
        if not command or any(not require_string(argument, f"{mode}.command argument") for argument in command):
            raise ValueError(f"{mode}: execution command is empty")
        cwd = require_string(execution["cwd"], f"{mode}.cwd")
        # These are recorded producer paths, not files opened on the checking
        # host. A saved macOS/Linux receipt must also validate on Windows.
        if unlabelled:
            require_value(command[0], "cargo", f"{mode}: unlabelled recorded command")
            log = retained(execution["log_file"], f"{mode}.log_file").decode("utf-8")
            commands = [ast.literal_eval(line.removeprefix("Command: ")) for line in log.splitlines()
                        if line.startswith("Command: ")]
            if commands != [command] or f"Checkout: {cwd}\n" not in log:
                raise ValueError(f"{mode}: unlabelled command/cwd differs from saved Cargo log")
            path_type = PureWindowsPath if PureWindowsPath(recorded_binary).is_absolute() else PurePosixPath
            relative_binary = path_type(recorded_binary).relative_to(path_type(cwd))
            if not any(f"({value})" in log for value in (str(relative_binary), relative_binary.as_posix())):
                raise ValueError(f"{mode}: unlabelled binary pathname differs from saved Cargo log")
            summary = (f"test result: ok. {len(pins)} passed; 0 failed;" if execution["exit_code"] == 0
                       else f"test result: FAILED. 0 passed; {len(pins)} failed;")
            if summary not in log:
                raise ValueError(f"{mode}: unlabelled test summary differs from saved Cargo log")
        for recorded_path in ((cwd,) if unlabelled else (command[0], cwd)):
            if not (PurePosixPath(recorded_path).is_absolute()
                    or PureWindowsPath(recorded_path).is_absolute()):
                raise ValueError(f"{mode}: execution command/cwd must be absolute")
        seconds = execution["seconds"]
        if unlabelled:
            require_value(seconds, None, f"{mode}: unlabelled whole-command seconds unavailable")
        elif type(seconds) not in (int, float) or not math.isfinite(seconds) or seconds < 0:
            raise ValueError(f"{mode}: execution seconds must be finite and nonnegative")
        exit_code = require_int(execution["exit_code"], f"{mode}.exit_code")
        allowed_exits = ((0, 101) if mode == "current"
                         and gate_environment in ("VERA20K_DIAGNOSTIC_LEGACY_BUILDING_HASH",
                                                  "VERA20K_BARRACKS_HASH_CONTROL")
                         else (0,))
        if exit_code not in allowed_exits:
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
    result = {"scope": "Rust-only same-binary hash-composition attribution",
            "incoming_main": dict(incoming), "comparisons": results,
            "normalization": list(HASH_COMPOSITION_NORMALIZATION),
            "execution_exit_codes": {mode: execution["exit_code"] for mode, execution in executions.items()},
            "comparison_sha256": hashlib.sha256(_canonical(results)).hexdigest()}
    if unlabelled:
        result["binary_provenance"] = dict(sha256=binary["sha256"],
            kind="unlabelled-diagnostic", whole_source_identity_available=False,
            cargo_manifest_available=False, source_leaf=dict(leaf), limits=list(limits))
    return result


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


GUNNER_MIGRATION_NORMALIZATION = [
    "tick_result.state_hash only",
    "entity.current_weapon_index must be zero before removal",
    "entity.weapon_override=null must migrate to current_weapon_number=0",
    "entity.current_turret_index must be -1 in these non-gunner fixtures",
    "entity.charge_turret_delay must equal the retained last FireAt rearm duration",
    "draws.callers .rs line/column only",
]
GUNNER_MIGRATION_TESTS = {
    "bridge": "sim::world::bridge_parity_harness_tests::bridge_crossing_replay_is_deterministic_and_baseline_stable",
    "global": "sim::world::global_parity_harness_tests::global_skirmish_replay_is_deterministic_and_baseline_stable",
    "slice6": "sim::world::slice6_retask_tests::replay_hash_stable_through_slice6",
}
GUNNER_OLD_FIELDS = {"current_weapon_index", "weapon_override"}
GUNNER_NEW_FIELDS = {"current_weapon_number", "current_turret_index", "charge_turret_delay"}


def compare_gunner_migration(baseline: list[dict], current: list[dict], fixture: str) -> dict:
    """Compare the bounded non-gunner fixtures, checking every removed feed.

    Last-shot slot and optional transport override are retired. These supplied
    fixtures have neither gunner nor charge types: all old slots are zero,
    all overrides absent and all new turret indices remain constructor -1.
    The new saved ROF word is derived solely from the unchanged FireEvent/rearm
    history; it is never accepted as an unconstrained extra field.
    """
    if len(baseline) != len(current) or len(baseline) < 2:
        raise ValueError(f"{fixture}: observation coverage differs or lacks a committed tick")
    left = normalized(baseline, omit_tick_hash=True, callers=True)
    right = normalized(current, omit_tick_hash=True, callers=True)
    actor_count = changed_hashes = draw_count = caller_changes = 0
    old_slots, new_weapons, new_turrets, delay_values, caller_edits = (Counter() for _ in range(5))
    delays, previous_fires, rearm_copies = {}, [], []
    for index, (old, new) in enumerate(zip(baseline, current)):
        label = f"{fixture}/{index}"
        for mode, row in (("baseline", old), ("current", new)):
            require_exact_keys(row, REPLAY_OBSERVATION_FIELDS, f"{label}/{mode}")
            require_exact_keys(require_object(row["rng"], "rng"), ("scenario", "main", "mapgen"), "rng")
            require_value(row["next_frame"], index, f"{label}/{mode}.next_frame")
            if (index == 0) != (row["tick_result"] is None):
                raise ValueError(f"{label}: seed/tick boundary changed")
            if row["tick_result"] is not None:
                _composition_u64(row["tick_result"].get("state_hash"), f"{label}/{mode}.state_hash")
        if index:
            changed_hashes += old["tick_result"]["state_hash"] != new["tick_result"]["state_hash"]
        actors = require_array(old["entities"], f"{label}.entities")
        current_actors = require_array(new["entities"], f"{label}.current_entities")
        if len(actors) != len(current_actors):
            raise ValueError(f"{label}: entity coverage differs")
        identities = [require_int(actor.get("stable_id"), "actor.stable_id") for actor in actors]
        if len(identities) != len(set(identities)):
            raise ValueError(f"{label}: duplicated actor identity")
        fires = require_array(old["fire_events_accumulated"], f"{label}.fires")
        if (len(fires) < len(previous_fires)
                or first_difference(previous_fires, fires[:len(previous_fires)])):
            raise ValueError(f"{label}: accumulated FireEvent history changed")
        just_fired = {}
        for fire in fires[len(previous_fires):]:
            attacker = require_int(fire.get("attacker"), "FireEvent.attacker")
            if attacker in just_fired or attacker not in identities or index == 0:
                raise ValueError(f"{label}: unsupported repeated, absent or seed FireAt producer")
            just_fired[attacker] = fire
        previous_fires = fires
        for offset, (before, after) in enumerate(zip(actors, current_actors)):
            actor = identities[offset]
            actor_label = f"{label}/actor{actor}"
            if (not GUNNER_OLD_FIELDS <= before.keys() or GUNNER_NEW_FIELDS & before.keys()
                    or not GUNNER_NEW_FIELDS <= after.keys() or GUNNER_OLD_FIELDS & after.keys()
                    or before.keys() - GUNNER_OLD_FIELDS != after.keys() - GUNNER_NEW_FIELDS):
                raise ValueError(f"{actor_label}: unexpected entity schema migration")
            require_value(after["stable_id"], actor, f"{actor_label}.stable_id")
            old_slot = require_int(before["current_weapon_index"], "retired last-shot slot")
            require_value(old_slot, 0, f"{actor_label}.current_weapon_index")
            require_value(before["weapon_override"], None, f"{actor_label}.weapon_override")
            weapon = require_int(after["current_weapon_number"], "current weapon")
            require_value(weapon, 0, f"{actor_label}.current_weapon_number")
            turret = require_int(after["current_turret_index"], "current turret")
            require_value(turret, -1, f"{actor_label}.current_turret_index")
            if actor not in delays:
                require_value(before["last_fire_frame"], -100, f"{actor_label}.constructor last fire")
                delays[actor] = 0
            if actor in just_fired:
                require_value(before["last_fire_frame"], index - 1, f"{actor_label}.last_fire_frame")
                timer = _composition_object(before["rearm_timer"], "rearm timer", ("duration", "start_frame"))
                require_value(timer["start_frame"], index - 1, f"{actor_label}.rearm start")
                delays[actor] = require_int(timer["duration"], "fired rearm duration")
                rearm_copies.append(dict(frame=index - 1, actor=actor,
                    weapon=just_fired[actor]["weapon"], duration=delays[actor]))
            delay = require_int(after["charge_turret_delay"], "retained charge delay")
            require_value(delay, delays[actor], f"{actor_label}.charge_turret_delay")
            actor_count += 1
            old_slots[str(old_slot)] += 1
            new_weapons[str(weapon)] += 1
            new_turrets[str(turret)] += 1
            delay_values[str(delay)] += 1
            for field in GUNNER_OLD_FIELDS:
                left[index]["entities"][offset].pop(field)
            for field in GUNNER_NEW_FIELDS:
                right[index]["entities"][offset].pop(field)
        old_draws = require_array(old["draws"], "baseline draws")
        new_draws = require_array(new["draws"], "current draws")
        if len(old_draws) != len(new_draws):
            raise ValueError(f"{label}: ordered RNG draw count differs")
        for old_draw, new_draw in zip(old_draws, new_draws):
            a = require_string(old_draw["callers"], "baseline callers")
            b = require_string(new_draw["callers"], "current callers")
            draw_count += 1
            if a != b:
                caller_changes += 1
                if SOURCE_LOCATION.sub(r"\1:<line>:<column>", a) != SOURCE_LOCATION.sub(r"\1:<line>:<column>", b):
                    raise ValueError(f"{label}: RNG caller change exceeds source positions")
                for old_line, new_line in zip(a.splitlines(), b.splitlines()):
                    if old_line != new_line:
                        caller_edits[(old_line.strip(), new_line.strip())] += 1
    if difference := first_difference(left, right):
        raise ValueError(f"{fixture}: gunner migration gameplay delta: {difference}")
    return dict(fixture=fixture, rows=len(current), actor_observations=actor_count,
        baseline_hash=baseline[-1]["tick_result"]["state_hash"],
        current_hash=current[-1]["tick_result"]["state_hash"],
        changed_tick_hashes=changed_hashes,
        legacy_last_shot_slot_counts=dict(sorted(old_slots.items())),
        legacy_null_override_count=actor_count,
        current_weapon_number_counts=dict(sorted(new_weapons.items())),
        current_turret_index_counts=dict(sorted(new_turrets.items())),
        charge_turret_delay_counts=dict(sorted(delay_values.items())),
        fire_rearm_copies=rearm_copies, draw_count=draw_count,
        caller_positions_changed=caller_changes,
        caller_edits=[dict(before=a, after=b, occurrences=count)
                      for (a, b), count in sorted(caller_edits.items())])


def check_gunner_migration(receipt_path: Path) -> dict:
    """Validate the Rust migration and any retained final passing replay."""
    receipt_path = receipt_path.resolve()
    snapshot = require_regular_file(receipt_path, "gunner migration receipt")
    receipt = _composition_object(parse_json_bytes(snapshot.raw, "gunner migration receipt"),
        "gunner migration receipt", ("schema_version", "kind", "scope", "incoming_main", "runs",
                                     "inputs", "comparisons", "normalization", "limits"),
        ("final_validation",))
    require_value(receipt["schema_version"], 1, "gunner migration schema_version")
    require_value(receipt["kind"], "rust-gunner-state-migration", "gunner migration kind")
    require_string(receipt["scope"], "gunner migration scope")
    if first_difference(receipt["normalization"], GUNNER_MIGRATION_NORMALIZATION):
        raise ValueError("gunner migration normalization differs from the guarded field contract")
    for limit in require_array(receipt["limits"], "limits"):
        require_string(limit, "limit")
    incoming = _composition_object(receipt["incoming_main"], "incoming_main", ("head", "pins"))
    head = require_string(incoming["head"], "incoming_main.head")
    if not re.fullmatch(r"[0-9a-f]{40}", head):
        raise ValueError("gunner migration requires a full source-base commit identity")
    pins = _composition_object(incoming["pins"], "incoming pins", tuple(GUNNER_MIGRATION_TESTS))
    for fixture, pin in pins.items():
        _composition_u64(pin, f"incoming pin {fixture}")
    runs = _composition_object(receipt["runs"], "runs", ("baseline", "current"), ("final",))
    if ("final" in runs) != ("final_validation" in receipt):
        raise ValueError("final execution and validation must be declared together")
    final = None
    if "final_validation" in receipt:
        final = _composition_object(receipt["final_validation"], "final_validation",
            ("source_base", "source_base_change_file", "full_suite", "observations", "normalization"))
        if not re.fullmatch(r"[0-9a-f]{40}", require_string(final["source_base"], "final source base")):
            raise ValueError("final execution requires a full source-base commit identity")
        if final["normalization"] not in ([], ["draw.callers .rs line/column only"]):
            raise ValueError("final replay may normalize only caller source positions")
    raw, saved, snapshots = _retained_replay_inputs(receipt_path, receipt["inputs"], tuple(runs), pins)
    if set(saved) != {(mode, fixture) for mode in runs for fixture in pins}:
        raise ValueError("gunner migration observation coverage differs from its three fixture pairs")
    executions = {}
    for mode in runs:
        run = _composition_object(runs[mode], f"run {mode}",
            ("execution_file", "log_file", "binary_sha256"))
        for field in ("execution_file", "log_file"):
            if require_string(run[field], f"{mode}.{field}") not in raw:
                raise ValueError(f"{mode}.{field} is not a frozen input")
        final_fields = ("full_suite_log_sha256", "full_suite_exit_code") if mode == "final" else ()
        execution = _composition_object(parse_json_bytes(raw[run["execution_file"]], mode), mode,
            ("command", "cwd", "exit_code", "seconds", "binary_sha256", "binary_unchanged", "source_base", *final_fields),
            ("source_edits_at_build", "source_sha256", "stage"))
        if (require_sha256(execution["binary_sha256"], "executed binary")
                != require_sha256(run["binary_sha256"], "declared binary")):
            raise ValueError(f"{mode}: executed binary identity differs")
        require_value(execution["binary_unchanged"], True, f"{mode}.binary_unchanged")
        require_value(execution["source_base"], final["source_base"] if mode == "final" else head,
                      f"{mode}.source_base")
        if "source_sha256" in execution:
            require_sha256(execution["source_sha256"], f"{mode}.source_sha256")
        if "source_edits_at_build" in execution:
            require_string(execution["source_edits_at_build"], f"{mode}.source_edits_at_build")
        if "stage" in execution:
            require_string(execution["stage"], f"{mode}.stage")
        command = require_array(execution["command"], f"{mode}.command")
        expected_args = ["--exact", *GUNNER_MIGRATION_TESTS.values(), "--nocapture", "--test-threads=1"]
        if not command or first_difference(command[1:], expected_args):
            raise ValueError(f"{mode}: executed fixture command differs")
        for value in (command[0], execution["cwd"]):
            value = require_string(value, f"{mode}.command/cwd")
            if not (PurePosixPath(value).is_absolute() or PureWindowsPath(value).is_absolute()):
                raise ValueError(f"{mode}: execution command/cwd must be absolute")
        seconds = execution["seconds"]
        if type(seconds) not in (int, float) or not math.isfinite(seconds) or seconds < 0:
            raise ValueError(f"{mode}: invalid execution duration")
        exit_code = require_int(execution["exit_code"], f"{mode}.exit_code")
        allowed = (0, 101) if mode == "current" else (0,)
        if exit_code not in allowed:
            raise ValueError(f"{mode}: unexpected execution result")
        log = raw[run["log_file"]].decode("utf-8")
        result = "ok. 3 passed; 0 failed" if exit_code == 0 else "FAILED. 0 passed; 3 failed"
        if f"test result: {result};" not in log:
            raise ValueError(f"{mode}: test summary does not match the declared execution")
        panics = re.findall(r"(?m)^thread '([^']+)'(?: \(\d+\))? panicked at ", log)
        expected_panics = [] if exit_code == 0 else list(GUNNER_MIGRATION_TESTS.values())
        if sorted(panics) != sorted(expected_panics):
            raise ValueError(f"{mode}: failed test identities differ from the final pin assertions")
        executions[mode] = execution
    for mode in runs:
        require_value(executions[mode]["cwd"], executions["baseline"]["cwd"], f"{mode} execution cwd")
    results = []
    for fixture, (count, _) in MAIN963_FIXTURES.items():
        if len(saved["baseline", fixture]) != count:
            raise ValueError(f"{fixture}: changed bounded fixture coverage")
        result = compare_gunner_migration(saved["baseline", fixture], saved["current", fixture], fixture)
        require_value(result["baseline_hash"], pins[fixture], f"{fixture}.incoming pin")
        if executions["current"]["exit_code"] == 101:
            # Only the expected final pin assertion may fail. Other per-tick,
            # gameplay and replay assertions cannot be excused by a hash change.
            log = raw[runs["current"]["log_file"]].decode("utf-8")
            panics = list(re.finditer(r"(?m)^thread '([^']+)'(?: \(\d+\))? panicked at [^\n]+\n", log))
            position = next(i for i, panic in enumerate(panics)
                            if panic.group(1) == GUNNER_MIGRATION_TESTS[fixture])
            end = panics[position + 1].start() if position + 1 < len(panics) else len(log)
            block = log[panics[position].end():end]
            pair = rf"left:\s*{result['current_hash']}\s+right:\s*{result['baseline_hash']}(?:\s|$)"
            if not re.search(pair, block):
                raise ValueError(f"{fixture}: missing exact old/new final pin failure")
        results.append(result)
    if first_difference(receipt["comparisons"], results):
        raise ValueError("gunner migration comparison receipt changed")
    result = dict(scope=receipt["scope"], normalization=GUNNER_MIGRATION_NORMALIZATION,
        comparisons=results, comparison_sha256=hashlib.sha256(_canonical(results)).hexdigest(),
        execution_exit_codes={mode: run["exit_code"] for mode, run in executions.items()})
    if final is not None:
        # The post-probe source-base update was documentation only. Retain its
        # actual diff and reject a receipt that broadens that attribution.
        name = require_string(final["source_base_change_file"], "source base change file")
        if name not in raw:
            raise ValueError("final source-base change is not a frozen input")
        headers = re.findall(r"(?m)^diff --git (.+)$", raw[name].decode("utf-8"))
        if headers != ["a/docs/research/ghidra-workflow.md b/docs/research/ghidra-workflow.md"]:
            raise ValueError("final source-base change exceeds the recorded documentation update")
        suite = _composition_object(final["full_suite"], "full suite", ("log_file", "passed", "ignored"))
        name = require_string(suite["log_file"], "full suite log")
        if name not in raw:
            raise ValueError("full suite log is not a frozen input")
        require_value(hashlib.sha256(raw[name]).hexdigest(),
            require_sha256(executions["final"]["full_suite_log_sha256"], "full suite SHA"), "full suite log SHA")
        require_value(executions["final"]["full_suite_exit_code"], 0, "full suite exit")
        passed = require_int(suite["passed"], "full suite passed")
        ignored = require_int(suite["ignored"], "full suite ignored")
        if passed < 3 or ignored < 0:
            raise ValueError("invalid full suite coverage")
        log = raw[name].decode("utf-8")
        summaries = re.findall(r"(?m)^test result: (.+)$", log)
        prefix = f"ok. {passed} passed; 0 failed; {ignored} ignored;"
        if len(summaries) != 1 or not summaries[0].startswith(prefix):
            raise ValueError("final full suite result differs from its recorded coverage")
        # Reuse the existing final-replay comparator: new actor fields and tick
        # hashes stay present. There is no second field migration at this stage.
        data = {item["fixture"]: raw[item["file"]] for item in receipt["inputs"]
                if item.get("mode") == "final"}
        observations = compare_final_observations(saved, data, bool(final["normalization"]))
        if first_difference(final["observations"], observations):
            raise ValueError("final replay comparison receipt changed")
        result["final_validation"] = dict(observations=observations,
            normalization=final["normalization"], binary_sha256=executions["final"]["binary_sha256"],
            source_base=final["source_base"], full_suite=dict(passed=passed, failed=0, ignored=ignored))
    for retained in [snapshot, *snapshots]:
        assert_snapshot_unchanged(retained, str(retained.path))
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", required=True)
    parser.add_argument("--current-observations", type=Path,
                        help="also compare a fresh VERA20K_REPLAY_DIAGNOSTICS directory")
    parser.add_argument("--main963-receipt", type=Path,
                        help="check a separate strict main963 same-binary receipt")
    parser.add_argument("--hash-composition-receipt", type=Path,
                        help="check a fixture-driven Rust-only same-binary hash attribution")
    parser.add_argument("--gunner-migration-receipt", type=Path,
                        help="check guarded Rust IFV weapon/turret state migration observations")
    parser.add_argument("--normalize-final-caller-positions", action="store_true",
                        help="explicitly allow only RNG caller line/column changes in final ungated observations")
    args = parser.parse_args()
    if args.gunner_migration_receipt is not None:
        if (args.hash_composition_receipt is not None or args.main963_receipt is not None
                or args.current_observations is not None or args.normalize_final_caller_positions):
            parser.error("gunner migration receipt does not accept other comparison modes")
        result = check_gunner_migration(args.gunner_migration_receipt)
    elif args.hash_composition_receipt is not None:
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
