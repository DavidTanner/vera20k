# Rust Unit deployment/body hash attribution

One immutable Cargo-owned lib test binary ran the bridge, global and Slice6
fixtures in control and current modes. Both runs passed every existing test
assertion. The control restored three prior hash feeds without changing live
state: omit the asserted-zero Unit deployed byte, replace an absent deployment
animation/false landing byte with the prior `deploy_state=None` u8, and fold zero
for voxel Unit body counters. The final expected pin expressions selected their
known incoming values only in control mode. Both temporary patches have been
removed; no diagnostic gate remains in Rust source.

The [receipt](receipt.json) reproduces all three incoming-main final pins.
Every one of the 819 off/on observed boundaries matches after removing only
`tick_result.state_hash`. This includes complete serialized actors, commands,
Logic order, accumulated fire/lifecycle outputs, all three full RNG states,
ordered raw draws with exact caller text, and retained House/Factory inputs.

| Fixture | Rows | Incoming control pin | Current pin |
| --- | ---: | --- | --- |
| bridge | 201 | `E3FD9DD363C98F82` | `F6E02080D0BE29C1` |
| global | 601 | `851C57C576CA991A` | `C86F736CE95687B3` |
| slice6 | 17 | `1AD1402910CE62D8` | `B20E11E0579BC4E4` |

This is bounded Rust regression attribution, not a native replay golden or a
whole-match certificate. Native deployment, Stage and body cadence comparisons
remain in [unit_simple_deploy](../unit_simple_deploy.md). These three fixtures do
not enter simple deployment or Jumpjet movement. The control guards assert no
deployed actor, deployment animation or landing request. Voxel Units retain and
advance their real body counter in both modes; only its hash input changes.

Binary SHA is `de8229c4c0b993c1bf9680a21f7c5ac38024f50500d35df7661f4d3940a4ab2a`
and source SHA is `ff8532c50eb4088811d4cca3d314f4230462313aa36dbdd585dfede94109ac39`.
The shared build owner preserves label `siege-chopper-hash-control-20261002`,
including its executable basename; this corpus contains no executable copy.
The manifest, logs, execution receipts, combined temporary patch, producer source
and six compressed transcripts are hash-pinned as receipt inputs.

```sh
python -m tools.spatial_oracle.astar_path_finishing_replay --check \
  --hash-composition-receipt tools/spatial_oracle/unit_simple_deploy_replay/receipt.json
```

The existing checker is extended only with the explicit
`VERA20K_DIAGNOSTIC_LEGACY_UNIT_DEPLOY_HASH` receipt identity, whose two execution
receipts must both exit 0. Historical Building receipts keep their prior contract.
There is no caller or actor normalization in the primary off/on proof.

A [supplementary incoming comparison](incoming-comparison.json) compares the
saved Engineer/House corpus to these current observations. It verifies the
removed `deploy_state` and new attachment/landing/deployed fields are defaults,
projects only the intended voxel Unit counter and retired voxel animation state,
and compares everything else. It also permits source line/column changes and
exactly two dispatcher frame substitutions from prior ancestor
`1b9ec6658301ed7cf3caee624fbc9ca7202a0e66`: the renamed Foot dispatcher and Harvest
routing through it. The [stack edit census](incoming-stack-differences.json)
records all such changes. All 819 rows match after those explicit projections.
The compressed comparison source is retained with its two arguments: checkout
root and this corpus directory. This supplementary historical projection does
not weaken the same-binary proof or claim unchanged presentation counters.
