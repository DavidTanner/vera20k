# Barracks hash attribution after main7e9

All 618 complete Rust observations (global601 and slice6 17) match between the
current and control modes after removing **only `tick_result.state_hash`**.
Actors, commands, outputs, full Scenario/Main/MapGen buffers, draws and exact
caller positions remain in the comparison. These are synthetic Rust replay
fixtures, not native goldens or whole-object/match certification.

| Fixture | Incoming main / control | Current |
| --- | --- | --- |
| global | `0xB966_DA80_21F0_8904` | `0x0F2C_FFB8_42D8_155E` |
| slice6 | `0xE28A_BFC8_CA12_3369` | `0xF008_D6D1_349C_CD9C` |

The temporary test-only hash projection omits Foot6B3 and Factory5D feeds and
hashes a copied FacingClass with constructor `start_frame: Some(0)` projected
to `None`. It does not change actual actors or gameplay. Incoming main's
Drive/Ship migration remains: retained state hashes once inside the active or
stashed class payload; the retired entity feeds stay removed.

[The strict receipt](receipt.json) uses the existing
[saved checker](../../astar_path_finishing_replay.py) labelled schema. Current
mode exits101 on exactly the two previous final pins; control exits0 and
recovers both incoming pins. The two baseline leaves cite this receipt.

The no-run label is `object-barracks-output-hash-main7e9-20261003`. Its exact
[manifest](build-manifest.json) records incoming HEAD
`7e932b968e9b901142d80944095af68fe178ebdc`, diagnostic build source
`17961a0f124f0758704502b07918cae4a7cff517bc9c65da0eb881e78b3cf72a` and binary
`9029caa55950c93d2144f96ae00ad7a83a02fa518601f7166e74b6b88c9b1089`.
Per-run captures retain the original command/environment, real elapsed time,
before/after binary, manifest and full source identities. Build/test logs,
Cargo inputs and complete rows are pinned losslessly; no executable is copied
into this folder. A Cargo source manifest does not hermetically seal ignored
retail assets, Cargo configuration or dependency caches.

Root restored the exact original hash leaf
`fd39b2dcb26078e1b3634e219fa4a5660ca999ca9ba204f4d171e4746618712b`
before promotion. The older c60 packet remains unchanged, including its
explicit unavailable whole-source identity. This packet records the new
labelled experiment separately.

Run the portable saved checks from the repository root:

```sh
PYTHONDONTWRITEBYTECODE=1 python -m tools.spatial_oracle.astar_path_finishing_replay --check --hash-composition-receipt tools/spatial_oracle/factory_infantry_output_replay/main7e9/receipt.json
PYTHONDONTWRITEBYTECODE=1 python -O -m tools.spatial_oracle.astar_path_finishing_replay --check --hash-composition-receipt tools/spatial_oracle/factory_infantry_output_replay/main7e9/receipt.json
```

Both modes produce identical retained check results. [capture.py](capture.py)
is the exact original external caller: it delegates build-artifact/source
identity and full-row checks to their existing owners. Live repetition requires
the recorded frozen source and preserved label, or a fresh fully recorded
no-run diagnostic build under owner control. Stage the gzip source/patch/log
inputs under their original names in a fresh external preparation directory;
the caller refuses source drift, inherited competing gates or replaced output.
Current mode removes `VERA20K_BARRACKS_HASH_CONTROL`; control sets it to `1`.
Both run the same two exact test filters from the same preserved executable.
Root owns installing/restoring the temporary source and all Cargo operations.

Preserve this active causal label through PR validation/publication. Afterwards,
review its required binary/debug dependencies and archived evidence, then use
the [Cargo owner's retirement dry run](../../../README.md#cargo-ownership-and-labeled-builds)
before any explicitly selected retirement. No retirement is performed here.
