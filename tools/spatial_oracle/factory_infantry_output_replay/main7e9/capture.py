#!/usr/bin/env python3
"""External caller for the existing labelled-build and saved-row owners.

Root installs/restores the prepared hash-only leaf and owns the Cargo build.
This caller neither builds nor edits checkout source, and never copies a binary.
"""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time


FIXTURES = {
    "global": (601, "sim::world::global_parity_harness_tests::global_skirmish_replay_is_deterministic_and_baseline_stable"),
    "slice6": (17, "sim::world::slice6_retask_tests::replay_hash_stable_through_slice6"),
}
GATE = "VERA20K_BARRACKS_HASH_CONTROL"


def sha(data):
    return hashlib.sha256(data).hexdigest()


def load(path):
    return json.loads(path.read_bytes())


def write(path, data):
    with path.open("xb") as stream:
        stream.write(data)


def save(path, value):
    write(path, (json.dumps(value, indent=2) + "\n").encode())


def prepared(path):
    data = load(path / "preparation.json")
    for name, identity in data["files"].items():
        raw = (path / name).read_bytes()
        if len(raw) != identity["bytes"] or sha(raw) != identity["sha256"]:
            raise ValueError(f"prepared input changed: {name}")
    return data


def owners(checkout):
    sys.path.insert(0, str(checkout))
    from tools.cargo_run import preserved_artifact, preserved_manifest, source_identity
    from tools.spatial_oracle.astar_path_finishing_replay import (
        HASH_COMPOSITION_GATE_FIELDS, check_hash_composition, rows,
    )
    return preserved_artifact, preserved_manifest, source_identity, HASH_COMPOSITION_GATE_FIELDS, check_hash_composition, rows


def run(args):
    checkout = args.checkout.resolve()
    preparation = args.preparation.resolve()
    output = args.output.resolve()
    if output.is_relative_to(checkout):
        raise ValueError("capture output must remain outside checkout")
    prep = prepared(preparation)
    artifact_owner, manifest_owner, source_owner, gates, _, row_owner = owners(checkout)
    manifest_path = args.manifest.resolve()
    directory, _, entries = manifest_owner(manifest_path.parents[2], manifest_path.parent.name)
    if (directory / "manifest.json").resolve() != manifest_path:
        raise ValueError("manifest path differs from preserved owner")
    manifest_raw = manifest_path.read_bytes()
    manifest = json.loads(manifest_raw)
    candidates = [entry for entry in entries if Path(entry["file"]).name.startswith("vera20k-")]
    if len(candidates) != 1:
        raise ValueError("label must contain exactly one vera20k lib-test executable")
    binary = artifact_owner(directory, candidates[0]).resolve()
    binary_sha = candidates[0]["sha256"]
    source = source_owner(checkout)
    if source != manifest["source"] or source["head"] != prep["incoming_main"]["head"]:
        raise ValueError("live frozen source differs from the build manifest")
    if sha((checkout / prep["source_leaf"]["path"]).read_bytes()) != prep["source_leaf"]["temporary_sha256"]:
        raise ValueError("installed temporary hash leaf differs from prepared bytes")
    if any(name in os.environ for name in gates if name != GATE):
        raise ValueError("another diagnostic hash gate is inherited")
    command = [str(binary), "--exact", "--nocapture", "--test-threads=1", *(name for _, name in FIXTURES.values())]
    output.mkdir(exist_ok=False)
    write(output / "build-manifest.json", manifest_raw)
    write(output / "build.log", (preparation / "labelled-diagnostic-build-v1.log").read_bytes())
    for name in ("Cargo.toml", "Cargo.lock"):
        write(output / name, (checkout / name).read_bytes())
    save(output / "caller-identity.json", {
        "file": str(Path(__file__).resolve()), "sha256": sha(Path(__file__).read_bytes()),
        "python": sys.version, "preparation_sha256": sha((preparation / "preparation.json").read_bytes()),
    })
    outcomes = []
    for mode in ("current", "control"):
        diagnostics = output / mode
        diagnostics.mkdir()
        log = output / f"{mode}.log"
        binary_before = sha(binary.read_bytes())
        manifest_before = sha(manifest_path.read_bytes())
        source_before = source_owner(checkout)
        if binary_before != binary_sha or manifest_before != sha(manifest_raw) or source_before != source:
            raise ValueError(f"{mode}: inputs changed before invocation")
        environment = dict(os.environ, PYTHONDONTWRITEBYTECODE="1", VERA20K_REPLAY_DIAGNOSTICS=str(diagnostics))
        environment.pop(GATE, None)
        if mode == "control":
            environment[GATE] = "1"
        started = time.time()
        timer = time.monotonic()
        with log.open("xb") as stream:
            process = subprocess.run(command, cwd=checkout, env=environment,
                                     stdout=stream, stderr=subprocess.STDOUT)
        elapsed = time.monotonic() - timer
        binary_after = sha(binary.read_bytes())
        manifest_after = sha(manifest_path.read_bytes())
        source_after = source_owner(checkout)
        unchanged = binary_before == binary_after == binary_sha
        execution = {
            "schema_version": 1, "mode": mode, "command": command, "cwd": str(checkout),
            "exit_code": process.returncode, "seconds": elapsed, "binary_sha256": binary_sha,
            "binary_unchanged": unchanged, "source_sha256": source["source_sha256"],
            "barracks_hash_control": environment.get(GATE),
        }
        save(output / f"{mode}.execution.json", execution)
        capture = {
            "schema_version": 1, "mode": mode, "started_unix": started,
            "completed_unix": time.time(), "command": command,
            "binary_path": str(binary), "binary_before_sha256": binary_before,
            "binary_after_sha256": binary_after, "manifest_path": str(manifest_path),
            "manifest_before_sha256": manifest_before, "manifest_after_sha256": manifest_after,
            "source_before": source_before, "source_after": source_after,
            "selected_environment": {name: environment.get(name) for name in
                (GATE, "VERA20K_REPLAY_DIAGNOSTICS", "VERA20K_REQUIRE_RETAIL_INI", "VERA20K_REQUIRE_RETAIL_ASSETS")},
            "log_sha256": sha(log.read_bytes()), "observations": {},
        }
        failure = None
        for fixture, (count, _) in FIXTURES.items():
            path = diagnostics / f"{fixture}.jsonl"
            if path.is_file():
                raw = path.read_bytes()
                observations = row_owner(raw, strict=True)
                capture["observations"][fixture] = {"bytes": len(raw), "sha256": sha(raw),
                    "rows": len(observations), "final_hash": observations[-1]["tick_result"]["state_hash"] if observations and observations[-1]["tick_result"] else None}
                if len(observations) != count:
                    failure = f"{fixture}: expected {count} rows"
            else:
                failure = f"{fixture}: missing rows"
        save(output / f"{mode}.capture.json", capture)
        if not unchanged or manifest_before != manifest_after or source_before != source_after:
            failure = "binary/manifest/source changed during invocation"
        if process.returncode not in ((0, 101) if mode == "current" else (0,)):
            failure = f"unexpected test exit {process.returncode}"
        outcomes.append({"mode": mode, "exit_code": process.returncode, "failure": failure})
        if failure:
            save(output / "capture-failure.json", {"status": "FAIL", "outcomes": outcomes})
            raise ValueError(f"{mode}: {failure}; retained all available outputs")
    save(output / "capture-summary.json", {"status": "CAPTURED_NOT_YET_COMPARED", "outcomes": outcomes})
    print(json.dumps({"status": "CAPTURED_NOT_YET_COMPARED", "output": str(output), "outcomes": outcomes}, indent=2))


def promote(args):
    checkout = args.checkout.resolve()
    preparation = args.preparation.resolve()
    capture = args.capture.resolve()
    output = args.output.resolve()
    prep = prepared(preparation)
    _, _, _, _, checker, row_owner = owners(checkout)
    restored = sha((checkout / prep["source_leaf"]["path"]).read_bytes())
    if restored != prep["source_leaf"]["before_sha256"]:
        raise ValueError("root must restore exact original world_hash before promotion/final checks")
    if output.exists():
        raise ValueError("promotion output exists; do not replace retained evidence")
    manifest_raw = (capture / "build-manifest.json").read_bytes()
    manifest = json.loads(manifest_raw)
    executions = {mode: load(capture / f"{mode}.execution.json") for mode in ("current", "control")}
    binary_sha = executions["current"]["binary_sha256"]
    inputs, selected, comparisons = [], {}, []
    for mode in ("current", "control"):
        proof = load(capture / f"{mode}.capture.json")
        if (proof["binary_before_sha256"] != binary_sha or proof["binary_after_sha256"] != binary_sha
                or proof["manifest_before_sha256"] != sha(manifest_raw)
                or proof["manifest_after_sha256"] != sha(manifest_raw)
                or proof["source_before"] != manifest["source"] or proof["source_after"] != manifest["source"]
                or proof["log_sha256"] != sha((capture / f"{mode}.log").read_bytes())):
            raise ValueError(f"{mode}: per-run identity proof differs")
        selected[f"{mode}.execution.json"] = (capture / f"{mode}.execution.json").read_bytes()
        selected[f"{mode}.capture.json"] = (capture / f"{mode}.capture.json").read_bytes()
        selected[f"{mode}.log.gz"] = (capture / f"{mode}.log").read_bytes()
        for fixture in FIXTURES:
            name = f"{mode}-{fixture}.jsonl.gz"
            raw = (capture / mode / f"{fixture}.jsonl").read_bytes()
            if sha(raw) != proof["observations"][fixture]["sha256"]:
                raise ValueError(f"{mode}/{fixture}: captured observations changed")
            selected[name] = raw
    for name in ("preparation.json", "world_hash.before-control.rs", "world_hash.temporary-control.rs", "world_hash-control.patch"):
        selected[name + (".gz" if name.endswith((".rs", ".patch")) else "")] = (preparation / name).read_bytes()
    selected["build-manifest.json"] = manifest_raw
    selected["build.log.gz"] = (capture / "build.log").read_bytes()
    for name in ("Cargo.toml", "Cargo.lock"):
        selected[name] = (capture / name).read_bytes()
    selected["caller-identity.json"] = (capture / "caller-identity.json").read_bytes()
    selected["capture.py"] = Path(__file__).read_bytes()
    if load(capture / "caller-identity.json")["sha256"] != sha(selected["capture.py"]):
        raise ValueError("capture caller changed after execution")
    for fixture, (count, _) in FIXTURES.items():
        current = row_owner(selected[f"current-{fixture}.jsonl.gz"], strict=True)
        control = row_owner(selected[f"control-{fixture}.jsonl.gz"], strict=True)
        comparisons.append({"fixture": fixture, "rows": count,
            "control_hash": control[-1]["tick_result"]["state_hash"],
            "current_hash": current[-1]["tick_result"]["state_hash"]})
    output.mkdir(parents=True, exist_ok=False)
    for name, raw in selected.items():
        transport = gzip.compress(raw, mtime=0) if name.endswith(".gz") else raw
        write(output / name, transport)
        item = {"file": name, "sha256": sha(transport), "raw_sha256": sha(raw)}
        for mode in ("current", "control"):
            for fixture in FIXTURES:
                if name == f"{mode}-{fixture}.jsonl.gz":
                    item.update(mode=mode, fixture=fixture)
        inputs.append(item)
    receipt = {
        "schema_version": 1, "kind": "same-binary-rust-hash-composition",
        "scope": "Integrated main7e9 Rust-only attribution of constructor-facing/Foot6B3/Factory5D hash folds; retained Drive/Ship migration unchanged",
        "binary": {"manifest_file": "build-manifest.json", "sha256": binary_sha,
            "source_sha256": manifest["source"]["source_sha256"], "label": Path(load(capture / "current.capture.json")["manifest_path"]).parent.name},
        "gate": {"file": "world_hash-control.patch.gz", "sha256": prep["source_leaf"]["patch_sha256"],
            "environment": GATE, "removal_required_before_final_checks": True,
            "effect": "Only omit Foot6B3/Factory5D hash feeds and hash a copied facing start_frame Some(0) as None; actors/gameplay/Drive/Ship payloads unchanged"},
        "runs": {mode: {"receipt_file": f"{mode}.execution.json"} for mode in ("current", "control")},
        "inputs": inputs, "incoming_main": prep["incoming_main"], "comparisons": comparisons,
        "normalization": ["tick_result.state_hash only"],
        "limits": [
            "All618 synthetic Rust observations retain complete actors, commands, outputs and three RNG buffers/draws/exact callers. Only tick_result.state_hash is projected out.",
            "The labelled manifest establishes the frozen tracked/nonignored build source and binary bytes. Ignored/local assets and dependency caches are not hermetically sealed by Cargo.",
            "Per-run before/after binary/manifest/full source identities and original logs are retained; root restored the exact original hash leaf before this check.",
            "This is same-binary Rust hash attribution, not native golden regeneration or whole-object/match proof. The old c60 receipt remains unchanged.",
        ],
    }
    receipt_path = output / "receipt.json"
    save(receipt_path, receipt)
    try:
        result = checker(receipt_path)
    except Exception as error:
        save(output / "promotion-failure.json", {"status": "FAIL", "error": str(error)})
        raise
    save(output / "check-normal.json", result)
    print(json.dumps({"status": "PASS", "output": str(output), "receipt_sha256": sha(receipt_path.read_bytes()), "comparison": result}, indent=2))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="operation", required=True)
    for name in ("run", "promote"):
        command = commands.add_parser(name)
        command.add_argument("--checkout", type=Path, required=True)
        command.add_argument("--preparation", type=Path, required=True)
        command.add_argument("--output", type=Path, required=True)
        if name == "run":
            command.add_argument("--manifest", type=Path, required=True)
        else:
            command.add_argument("--capture", type=Path, required=True)
    args = parser.parse_args()
    try:
        (run if args.operation == "run" else promote)(args)
    except Exception as error:
        print(f"{args.operation}: {type(error).__name__}: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
