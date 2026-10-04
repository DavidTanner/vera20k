# Ghidra annotation passes

[`ApplyGhidraPass.java`](ghidra_pass/ApplyGhidraPass.java) replays a recorded
annotation pass onto any copy of the gamemd.exe Ghidra database. Several copies
exist (the main annotation machine, downloaded snapshots), so a pass made on one
copy is kept as a JSON ledger in [`ghidra_pass/passes/`](ghidra_pass/passes/) and
replayed on the others instead of being redone by hand.

The script refuses a program whose executable SHA-256 is not the ledger's
(`1cdd1180e49024fbda8ad568caac2e86e856063ff67ab38f62b7d2c7bb84298c`). It never
overwrites state it did not plan against: each operation records what it expects
to find, and anything else is a conflict that is reported and skipped.

## Running a pass

Run `check` first. It reports every operation and writes nothing. `apply` writes
the pending operations in one transaction, reads each one back and rolls the whole
transaction back if any readback fails. Save the program afterwards.

In the Ghidra GUI, add `tools/ghidra_pass` to the Script Manager's script
directories, run `ApplyGhidraPass.java`, then pick the ledger and the mode.

Headless, on a project that is not open in the GUI:

```sh
"$GHIDRA_INSTALL_DIR/support/analyzeHeadless" /path/to/projects ProjectName \
  -process gamemd.exe -noanalysis -scriptPath tools/ghidra_pass \
  -postScript ApplyGhidraPass.java tools/ghidra_pass/passes/<pass>.json check
```

Replace `check` with `apply` to write; add `-readOnly` to rehearse an apply
without saving. `-process` takes the program's file name in the project.

Each operation prints as `PENDING` (would be written), `DONE` (already in the
database) or `CONFLICT` (the database differs from what the pass expected). A
conflict usually means another copy changed that function first; compare the two
and decide by hand. A ledger with `"atomic": true` applies nothing while any
conflict remains.

## Ledger format

```json
{
  "format": 1,
  "pass": "2026-10-04 VERA-cited names",
  "tag": "[2026-10-04 VERA-cited names]",
  "program_sha256": "1cdd1180e49024fbda8ad568caac2e86e856063ff67ab38f62b7d2c7bb84298c",
  "atomic": false,
  "ops": [
    {"op": "rename_function", "address": "0x004010C0", "from": "FUN_004010c0", "to": "Name"},
    {"op": "append_plate", "address": "0x004010C0", "text": "Evidence for the name."},
    {"op": "create_function", "address": "0x00418E14", "name": "FUN_00418e14"}
  ]
}
```

- `rename_function` renames the function that starts at `address` when its
  current name is `from`. A name that already belongs to another function is a
  conflict.
- `append_plate` appends `tag` and `text` as a new paragraph of the function's
  plate comment (or the plate at a non-function address). It is done once the
  plate holds that paragraph and conflicts with a different paragraph under the
  same tag.
- `create_function` creates a function at an instruction start that no function
  covers. Ghidra computes the body.

The ledger is also the record of what the pass changed and why: the plate text
carries the evidence, following the tagged-paragraph convention in
[the Ghidra workflow](../docs/research/ghidra-workflow.md#names-and-their-sources).
