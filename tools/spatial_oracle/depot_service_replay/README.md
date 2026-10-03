# Repair progress hash attribution

All 601 saved global-fixture boundaries match after removing only
`tick_result.state_hash`. The 600 committed tick hashes differ. Actor fields,
commands, Logic order, accumulated fire/lifecycle output, retained gameplay
inputs, all three complete RNG states and every ordered draw with its exact
caller location remain identical.

| Fixture | Boundaries | Control final pin | Current final pin |
| --- | ---: | ---: | ---: |
| global | 601 | 1281236110607522088 | 8184490706220851914 |

This is Rust-only hash composition attribution in a bounded synthetic replay.
It establishes no native gameplay equivalence or native golden value.

## Control and identity

The same unchanged Cargo-owned test executable ran both modes and both exited
0. `VERA20K_DIAGNOSTIC_LEGACY_BUILDING_HASH=1` omits only the
`building-repair-progress-620` discriminator and Building-owned
`repair_progress()` Stage contribution. All other hash feeds and simulation
bodies execute in both modes. The patch also selects the corresponding expected
final pin in the test assertion, which permits both runs to succeed; that
selector changes no simulation state.

Control reproduces the global pin committed in incoming main `2909aba80d45582b2e4dc928c83c66ba685e52dd`.
The original incoming source is retained. No separate incoming-main binary run
was needed or performed for this attribution. Current keeps the new Stage fold.
This recovers the prior pin with the candidate binary; it does not claim full
incoming-main gameplay or unobserved-state equality.

Binary SHA-256: `34d590711a68a28ff7e55600aaa8411332071f0266777be7be690fc28cf602a5`.
Source SHA-256: `d92f9d4be139a66d2028af2b6c24a52d1ed523cc4291dbbddab2a6aa25849e3a`.
Cargo label: `depot-service-hash-attribution-20261003`.
The original manifest and execution receipts are preserved byte-for-byte. The
label retains the actual test executable through the shared build owner.
The producer was macOS/aarch64 with the Rust/Cargo versions recorded in the
manifest; this package is not a cross-platform execution result.

## Scope and validation

The existing global harness uses authored GAWEAP/GAREFN/HARV/MTNK/E1 rules and
runs the production `advance_tick` record/replay path for 600 ticks, with one
initial seed boundary. It has no repair depot type or depot service order.
The Stage fold is therefore attributed in this supplied global fixture;
production depot servicing and original executable comparisons have separate
evidence. Equality is limited to the complete recorded observation fields and
cannot establish equality of unrecorded state, rendering or an asset-loaded map.

The gate and expected-pin selector are temporary diagnostics. Their removal and
final candidate validation are the mechanism owner's separate work. This
package records the diagnostic runs and does not certify the final ungated
build. The original producer script records the gate/diagnostics settings and
retail-INI requirement; inherited host process environment is not serialized.

The [strict receipt](receipt.json) hashes every compressed envelope and original
byte stream, including logs, patch, original manifest/execution receipts and
source snapshots. Compression reuses `shrapnel_repair.packet_io.compressed`
(no filename, mtime zero, host-independent envelope). RNG caller strings, actor
fields and raw JSONL bytes are preserved. Only the existing checker's explicit
tick-hash projection is used; no gameplay or caller normalization is enabled.

Use the configured [oracle Python runtime](../../native_oracle.md), then run:

```sh
python -m tools.spatial_oracle.astar_path_finishing_replay --check \
  --hash-composition-receipt tools/spatial_oracle/depot_service_replay/receipt.json
```

While this package is staged in the ignored directory, replace the receipt path
with `.local-audit/depot-service/replay/retained/receipt.json`.
The unchanged comparison owner validates the recorded command/cwd as producer
identities and opens retained inputs relative to the receipt. It does not open
the original producer paths. [check-result.json](check-result.json) contains
its actual successful result; the per-run execution logs remain separate inputs.
