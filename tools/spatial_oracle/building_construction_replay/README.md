# Rust building-body hash attribution

This is a Rust-only comparison, not a native replay golden or an object
certificate. The six construction native joins are documented separately in
[`building_construction.md`](../building_construction.md).

One immutable Cargo-owned test binary ran the bridge, global and slice6 replay
fixtures twice. Control restored only the former two absent BuildingUp/
BuildingDown hash inputs instead of the new body tag and private body/sale
inputs. It reproduced all three incoming-main final pins. Current retained the
new folds. No simulation body, other hash input, fixture, ready byte, Stage,
primary, command or RNG decision was changed between those runs.

All819 observed boundaries compare equal after removing only
`tick_result.state_hash`: complete serialized entities, commands, live Logic
order, accumulated fire/lifecycle outputs, all three full RNG states and ordered
raw draws with exact caller positions. These fixtures neither construct nor sell
a building. This proves attribution of their hash changes, not excluded gameplay
or equivalence to unrecorded incoming-main observations.

| Fixture | Rows | Reproduced prior pin | Current pin |
| --- | ---: | --- | --- |
| bridge | 201 | `8DE16E7B3B6FE661` | `8D29B490A034EEC9` |
| global | 601 | `C3551AAB44B60A29` | `82E90C5AA33D9A44` |
| slice6 | 17 | `67C241E9F79C8F43` | `8DD6943289215676` |

The receipt hashes every compressed and original observation, execution log,
gate patch and saved Cargo manifest. Its binary SHA is
`72cb0848bea2a1a45ae3171a74b796fbb09afe39f62ac01e99366cdbca813b70`;
label `object-construction-hash-attribution-v1-20261002` retains the actual
basename under the shared build owner. No executable copy is added here.
The temporary test-only gate was removed before final validation.

```sh
python -m tools.spatial_oracle.astar_path_finishing_replay --check \
  --hash-composition-receipt tools/spatial_oracle/building_construction_replay/receipt.json
```

To observe the ungated final candidate, set `VERA20K_REPLAY_DIAGNOSTICS` to a new
directory and run the existing three filtered lib tests through `tools.cargo_run`.
The former pins and current pins remain Rust regression receipts; native arithmetic,
RNG and lifecycle comparisons remain in their original-executable corpora.
