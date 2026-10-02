# IFV state migration: bounded Rust replay attribution

The three synthetic replays retain all recorded gameplay and RNG observations
after the IFV weapon owner migration. Their hashes change because the old `u8`
last-shot feed becomes the authoritative signed `i32` current-weapon feed and
the redundant optional weapon override feed is removed. The new turret index
and saved charge duration feed drawing and are excluded from simulation hashing.

This is **Rust regression evidence**, not a native parity claim. Native selector,
charge and FireAt component evidence lives in
[ifv_turret_switching.md](../ifv_turret_switching.md).

| Fixture | Recorded boundaries | Old final hash | New final hash |
| --- | ---: | --- | --- |
| Bridge | 201 | `F6E02080D0BE29C1` | `C40E68BDB392AF79` |
| Global | 601 | `C86F736CE95687B3` | `11C7DD744E2A1928` |
| Slice 6 | 17 | `B20E11E0579BC4E4` | `56050F10EACE5E86` |

The [receipt](receipt.json) pins the compressed and raw SHA-256 of every input,
the exact comparison census and every changed RNG caller location. Its checker
extends the existing [replay comparison owner](../astar_path_finishing_replay.py).
It checks these values before removing fields from the equality comparison:

- All 4,647 old actor records have last-shot slot `0` and no weapon override;
  their replacements have current weapon `0` and turret index `-1`.
- The saved charge duration starts at `0` and changes only to the baseline rearm
  duration on each unchanged FireEvent. There are 13 `105mm` shots in the global
  fixture; the retained values `50`, `51` and `52` account for 623 actor records.
  A later rearm write without a FireEvent cannot replace that retained duration.
- All 400 ordered RNG draws match. In 388 draws only Rust source line/column
  positions changed; full caller paths, draw values and all three complete RNG
  stream states remain equal.
- Only the 816 committed tick hashes are omitted. Commands, actor field
  remainders, Logic order, accumulated fire/lifecycle outputs and retained
  House/Factory/navigation inputs compare exactly. No retained configuration
  fingerprint or other state difference is excused.

The baseline ran the three tests successfully. The retained **probe** was run
before repinning and failed only their three final old-hash assertions. The
checker requires those exact test identities and old/new values; it does not
accept arbitrary failed tests. The retained **final** run passes all three tests
and matches every probe observation, including the new state fields and tick
hashes. Only caller source positions changed in its 400 RNG draws. The same
binary's full-suite log records **9,494 passed, 0 failed, 227 ignored**.

Both execution receipts record source base
`be14d088727f1745f34bc0e7a41055616ff67eaf` and an unchanged binary during execution:

- Baseline binary: `016363e6b93c0f04764d71f561f580eb5662469060294613c74b388cccc76125`.
- Probe binary: `989a770d48dc48a05998eb6c27984c8ec5d41c880376c66bd70e68362565c59d`.

The final execution records source base
`eea32db2dff2906a41d1c106ec48abf1fc72e81d` and binary
`64766bb07e50fb1d3109c71c6cf7110ef1e0668706991cada176a8d682197de7`.
The receipt also pins its complete diagnostic transcripts, execution log and
full-suite log; the checker reuses the existing final-observation comparator.

The baseline had only the new failing render atlas regression beyond that source
base; the probe contained the IFV implementation before pin edits. The saved
build logs and execution receipts establish these runs, but do not provide a
hermetic full-tree build manifest. Frozen fixture source files are the exact
baseline commit bytes. `hash-migration.patch.gz`, `fireat-migration.patch.gz`
and `gunner-owner.rs.gz` retain the affected state and hash owners at comparison
time. The subsequent main fast-forward changed only the Ghidra workflow document;
`final-source-base-change.patch.gz` retains that diff. Final replay comparison
keeps all hashes and fields present, with no second state migration.

To recheck the frozen evidence without Cargo or retail assets:

```sh
python -m tools.spatial_oracle.astar_path_finishing_replay --check \
  --gunner-migration-receipt tools/spatial_oracle/ifv_turret_replay/receipt.json
python -m unittest tools.tests.test_astar_path_finishing_replay
```

These fixtures contain no gunner or charge types. They do not test secondary
last-shot slots, boarding, nonzero gunner modes or native rendered output; those
claims require their dedicated component, passenger and production capture checks.
