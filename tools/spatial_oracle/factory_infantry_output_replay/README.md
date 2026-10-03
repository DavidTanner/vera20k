# Rust barracks hash attribution

This records the same diagnostic Rust binary in current and constructor-facing
hash-control modes. Every field in all601 global and17 slice6 rows matches after
removing **only `tick_result.state_hash`**. Serialized actors, mission/Factory
inputs, all three full1,012-byte RNG buffers, draws and exact callers remain in
the comparison. This is a Rust-only composition check, not native goldens or a
whole-object/match proof.

The native shared TechnoUnlimbo body-facing snap at6F6DAA leaves the Rust facing
timer's constructor state as `Some(0)`. The private Foot6B3 and Factory5D lifecycle
feeds also enter the hash. The temporary control projected only those hash
reads back to their earlier composition; actor state and RNG stayed unchanged.
It restores incoming mainc60 hashes6002303832519451608/global and
4880850378294197527/slice6. Current hashes are512540194600632770/global and
17678781126079047671/slice6. The exact before/temporary source leaves and their
mechanically computed patch are retained. `world_hash` was restored before final
checks; the control is absent from production source.

```sh
PYTHONDONTWRITEBYTECODE=1 python -m tools.spatial_oracle.astar_path_finishing_replay \
  --check --hash-composition-receipt tools/spatial_oracle/factory_infantry_output_replay/receipt.json
PYTHONDONTWRITEBYTECODE=1 python -O -m tools.spatial_oracle.astar_path_finishing_replay \
  --check --hash-composition-receipt tools/spatial_oracle/factory_infantry_output_replay/receipt.json
```

The existing checker owns strict rows, retained byte identities and the narrow
unlabelled diagnostic variant. It preserves the legacy labelled checks. This
receipt retains complete lossless gzip rows, exact Cargo commands/logs and the
recorded environment-presence gate. Saved checks perform no Cargo execution,
native emulation, source restoration or baseline generation.

Binary SHA256 captured after the recorded pair:
`c41a138a5aaeaf4dbdafa5ee32a0f3bb52ec208dc393129c44ac5a182bc59802`.
The experiment used an unlabelled Cargo artifact. A build-time whole-source
manifest, per-run binary/source captures, literal control environment value and
whole-command wall durations were not retained. Their fields are explicitly
unavailable; `unlabelled-binary-identity.json` is **not a Cargo label manifest**.
The same-binary assertion has the recorded common path, paired command/logs and
one final binary SHA capture as its bounds. Binary bytes are not copied and may
be retired by the Cargo owner. Recorded paths are provenance, never fallback
paths opened by the checker.

The complete observations and source-control inputs originate from the frozen
`barracks-output-replay-causal-mainc60-v2` experiment under the object-completion
evidence root. Native behavior remains independently established in the public
`factory_infantry_output` package; its profiles, receipts and exports are unchanged.
