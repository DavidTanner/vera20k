# Integrated repair progress hash attribution

All 601 saved global-fixture boundaries match after removing only
`tick_result.state_hash`. The 600 committed tick hashes differ. Commands,
complete recorded actor fields, Logic order, accumulated fire/lifecycle output,
retained path inputs, all three complete RNG states and ordered draws with
their exact caller locations remain identical.

| Fixture | Boundaries | Control final hash | Current final hash |
| --- | ---: | ---: | ---: |
| global | 601 | 3713302477009678589 | 14817136268200160838 |

This is Rust-only hash composition attribution in a bounded synthetic replay.
It establishes no native gameplay equivalence or native golden value.

## Same binary and diagnostic assertion

Both modes use the same unchanged Cargo-owned test executable and exit0.
`VERA20K_DIAGNOSTIC_LEGACY_BUILDING_HASH=1` omits only the
`building-repair-progress-620` discriminator and Building-owned
`repair_progress()` Stage hash contribution. Current retains both feeds.
Every other hash feed and simulation body executes in both modes.

The temporary patch also changes the final test assertion. Control asserts
the exact incoming pin `0x33884CEDCDBD70FD`; current compares `final_hash` to
itself to measure the new composition. Current exit0 therefore does not assert
an independently pinned new final hash. The strict retained-row checker
compares the actual recorded current hash with the receipt and proves complete
recorded-row equality after its sole tick-hash projection. Gate removal and
final ungated exact-pin validation remain separate work by the mechanism owner.

Control recovers the global pin committed in incoming main
`96204fef1a9b8aef3143a50eed4d5fb6bb7a29f7`. The incoming source and both exact
pre-gate Rust source snapshots are retained. No separate incoming-main binary
run was needed or performed for this attribution. Recovery with the candidate
binary does not establish full incoming-main gameplay or unrecorded-state
equality.

Binary SHA-256: `2ebec24e634b04cfa24d748f152ba16bbb30e0502e7c3c94a719df6532e6beb9`.
Source SHA-256: `c058f08e13ac6f444fe3bd4d36ebf6a75f1b948d304547f12e77ffc91bea7783`.
Cargo label: `depot-service-hash-attribution-main1018-20261003`.
The original build manifest and execution receipts are retained byte-for-byte.
The shared build owner retains the actual executable. The producer used
macOS/aarch64 and the Rust/Cargo versions recorded in the manifest; this is
not a cross-platform execution result.

## Fixture and retention limits

The existing global harness uses authored GAWEAP/GAREFN/HARV/MTNK/E1 rules and
executes the production `advance_tick` record/replay path for 600 ticks, plus
one initial seed boundary. It has no depot type or repair service order. This
package attributes the new Stage fold in that supplied global fixture.
Production depot service and original executable comparisons have separate
evidence. Recorded equality cannot certify unrecorded state, rendering, an
asset-loaded map or a whole match.

The producer script records the explicit gate, diagnostics directory and
retail-INI requirement. Inherited host environment is not serialized. These
are diagnostic runs, not certification of the final ungated build. The
earlier parent replay package remains unchanged.

The [strict receipt](receipt.json) hashes every retained envelope and original
byte stream, including raw JSONL observations, logs, patch, manifest/execution
receipts, producer and source snapshots. Compression reuses
`shrapnel_repair.packet_io.compressed`: no filename, mtime zero and a
host-independent envelope. Actor fields, RNG caller strings and raw JSONL
bytes are preserved; no caller or actor normalization is enabled.

Use the configured [oracle Python runtime](../../../native_oracle.md), then run:

```sh
python -m tools.spatial_oracle.astar_path_finishing_replay --check \
  --hash-composition-receipt tools/spatial_oracle/depot_service_replay/main1018/receipt.json
```

While staged in the ignored directory, replace the receipt path with
`.local-audit/depot-service/replay-main1018/retained/receipt.json`.
The unchanged checker validates recorded command/cwd producer identities and
opens retained inputs relative to the receipt; it does not open original
producer paths. [check-result.json](check-result.json) records its actual
successful comparison. Original execution logs and the independent quick
strict-row comparison remain separate hash-pinned inputs.
