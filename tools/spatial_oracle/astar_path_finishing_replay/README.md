# Rust replay hash composition receipt

This folder preserves a **Rust-only comparison**, not gamemd goldens or native
whole-route parity. It attributes the three historical replay hash changes to
the newly retained navigation history and initialized Techno threat contribution.
Native instruction comparisons remain in `../astar_path_finishing.json` and the
other native packets; this receipt does not replace them.

Main `7412f45c0ba6a223752b785361d99ecaa5abe250` was built with observation-only
hooks through the shared runner, label
`bridge-main7412-replay-baseline-20261001`. The saved manifest records source SHA
`13c9b13de14f0233927bca10970266c9ef3afa444372c3d12037e5308b858c07` and binary SHA
`df423e1d541b56ce7a2a56be37f9b34d2ade218f7eb7ed1578214dc0366582ed`.
The manifest itself has SHA
`6176ba765701b0ae8b2a1911679aed849dcb4634bf5c05970f256ae8d32efaca`.

Candidate legacy and current observations came from the same Cargo73454 binary,
SHA `960bc549a86944c511be6c6467b477a917d6516b0bccab6426e4e0c7173ad319`, without a
build or source change between the direct runs. The candidate source is bounded
as main7412 plus the retained navigation/identity/lifecycle changes and diagnostic
hooks. No candidate source manifest survives, so no exact source SHA is claimed.

The temporary `VERA20K_DIAGNOSTIC_LEGACY_PATH_HASH=1` test-only gate omitted just
the four new folds through the existing hash body: retained navigation, House
spatial threat, Foot coefficient and cached Techno contribution. Root removed
that gate after attribution. The production hash retains all four folds.

| Fixture | Observation rows | Reproduced old hash | Current composition hash |
| --- | ---: | --- | --- |
| bridge | 201 | `B592E027A510DB45` | `EB5CF8B11E9B7DC6` |
| global | 601 | `7099F3F4CCAB0F1E` | `B25283B67486B760` |
| slice6 | 17 | `BB0943C9DBB18884` | `2A956763E046638A` |

Main versus candidate legacy has zero differing rows after omitting only
`retained_path_inputs`, the new actor `cached_spatial_threat` and navigation
`threat_avoidance_coefficient` fields, and replacing `.rs:line:column` in RNG
caller backtraces. Functions, filenames, stack order, values, draw indices and
RNG order remain compared. Candidate current versus legacy has zero differing
rows after omitting **only** `tick_result.state_hash`.

Every candidate observation has navigation present, House grids all zero, Foot
coefficient bits zero, initialized cached contributions zero, and no live Teams
or Team overrides. The navigation fold and `Some(0)` contribution fold still
change the hash. This comparison covers the recorded owner state, commands,
logic order, accumulated fire/lifecycle outputs, and RNG draws, including seed
draws. Equal legacy per-frame hashes supplement those observations; state not
recorded here is not separately certified.

The saved main run passed four tests. Candidate legacy passed the three old pins
and two snapshot restores; its additional two-moving-vehicles test failed.
Candidate current passed the two snapshot restores and failed the three old pins
with the attributed values above, before the justified pin updates. That separate
movement failure remains open in this receipt. Root validates the updated pins
and final candidate separately; this evidence does not certify the bridge goal.

All nine raw JSONLs are preserved byte-for-byte in deterministic gzip with empty
filenames and `mtime=0`. `receipt.json` pins both compressed and raw input SHAs,
the main source manifest, logs, command receipts and observation patch.

Run the saved comparison using the native-oracle environment (Unicorn is an
import dependency of the shared comparison helper):

```sh
/Users/halvor/Documents/vera20k-dev/.venv/bin/python -m tools.spatial_oracle.astar_path_finishing_replay --check
```

This passed on 2026-10-01 with comparison SHA
`e91866e9d1dcd1537ea3154690c1f440103fdfd761f59a3f5a2b7a4d04417cc7`.
The companion uses `tools.native_oracle.first_difference` and `_canonical`;
it owns observation normalization only, not another gameplay/comparison algorithm.
It also rejects changed input bytes before comparing rows.

To record a fresh current run through the existing diagnostic owner:

```sh
VERA20K_REPLAY_DIAGNOSTICS=target/bridge-path-replay-observations \
  /Users/halvor/Documents/vera20k-dev/.venv/bin/python -m tools.cargo_run -- \
  test -p vera20k --lib -- --nocapture \
  bridge_crossing_replay_is_deterministic_and_baseline_stable \
  global_skirmish_replay_is_deterministic_and_baseline_stable \
  replay_hash_stable_through_slice6
/Users/halvor/Documents/vera20k-dev/.venv/bin/python \
  -m tools.spatial_oracle.astar_path_finishing_replay --check \
  --current-observations target/bridge-path-replay-observations
```

The fresh comparison permits only caller source-position changes. It does not
silently replace these receipts or accept a future gameplay divergence.

The later [main963 same-binary receipt](main963/README.md) separately establishes
the three integrated hash-composition changes. Its strict checker and preserved
inputs leave this historical main7412 contract unchanged.

The `main963/receipt.json` also pins `final_followups` through the same saved-final
comparison owner. The integrated main965 followup retains compressed819-row
transcripts, execution receipt/results, both build manifests and strict-retail
validation logs in `main963/integrated-main965/`. It compares all recorded fields
and tick hashes to the original current-mode baseline, with only the explicitly
listed RNG caller source line/column normalization. It keeps the historical
main963 final identity and copies no executables. Build-time source fingerprints
include separately pending tooling/policy files; they do not claim later PR byte
identity. Production observations retain their separate bounded comparator.
