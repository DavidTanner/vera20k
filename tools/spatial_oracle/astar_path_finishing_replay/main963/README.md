# Main963 replay hash composition

This is a separate **Rust-only** receipt. Historical main7412 inputs and the
default checker remain unchanged. No incoming-main baseline build was run.

The same immutable test executable ran both modes. The shared-runner label
`bridge-path-main963-hash-unload-red-v2-20261001` preserves source SHA
`dfa8855edd38a88d381c45082f7b3ae697cc408e0a7f08816cb6a8f189b0162f`
and binary SHA
`f87ea237a8ddac71bcc2b9ee205e5e5873fc865ecd774b1827233ff69392e99f`.
The incoming main was `896e007ba88164246d3b94e305d88c003b035b4a`.

The temporary test-only environment gate
`VERA20K_DIAGNOSTIC_LEGACY_PATH_HASH=1` omitted only the navigation history,
House spatial-threat, Foot+530 coefficient and cached Techno+508 contribution
hash feeds. Its exact patch is pinned, with raw SHA
`e58a85757a7902687702ef94f8e2bd2d6944911be86249978531aa90fbb822c5`.
The gate was removed precisely before final candidate validation; production
and the normal test mode retain all four feeds. Incoming Walk route retirement
and movement state were never restored or masked. The Team override fold was
never gated.

| Fixture | Rows | Incoming pin reproduced by control | Current composition |
| --- | ---: | --- | --- |
| bridge | 201 | `CC11DDD91928916A` | `8DE16E7B3B6FE661` |
| global | 601 | `EB5433C611607535` | `89764F170FCFA3FB` |
| slice6 | 17 | `5BA5F8CBB5F72FF5` | `CA1388579211B978` |

All 819 observations match after omitting **only** `tick_result.state_hash`.
No actor fields or caller positions are normalized in the paired comparison.
Commands, full serialized actors, logic order, accumulated fire/lifecycle
output, all three native RNG states, seed/frame draws and their exact caller
positions remain compared. Every row has navigation present, zero House
grids, zero coefficient bits, initialized zero contributions and no Teams or
Team overrides. Control passed the three incoming ratchets; normal mode failed
only their final hash assertions, producing the justified pins above.

The labelled binary predates the separate transport-unload threat correction.
These synthetic replay fixtures do not exercise unloading. This comparison
does not validate that dependency, nonzero-threat routes, whole-native gameplay
or the exhaustive bridge goal. Exact recorded control/current equality and
reproduced incoming final pins do not establish unrecorded incoming-main
per-frame equality. The actual final ungated validation is recorded below.

`receipt.json` pins deterministic gzip observations/logs, command receipts,
the build manifest and exact gate patch. The strict comparison passed with SHA
`c09d4aed8501053ea3e298b58bb7b81b4b9c0686aac3c59f27b3ff84b9013546`:

```sh
/Users/halvor/Documents/vera20k-dev/.venv/bin/python \
  -m tools.spatial_oracle.astar_path_finishing_replay --check \
  --main963-receipt tools/spatial_oracle/astar_path_finishing_replay/main963/receipt.json
```

After a final ungated replay capture through `VERA20K_REPLAY_DIAGNOSTICS`, add
`--current-observations <directory>`. That check retains every tick hash and
requires the new pins. If removing the diagnostic gate or other validated
source edits change RNG backtrace positions, explicitly add
`--normalize-final-caller-positions`. It permits only `.rs:line:column` changes
in draw callers, preserving filenames, function identities, stack order,
values, indices and draw order. The result records each fixture's raw input
SHA and the number of caller positions changed. Paired controls always remain
strict. Fresh results must be retained with their final binary/source identity;
the saved pre-unload-fix binary does not stand in for that validation.

## Actual final ungated replay

The final label `bridge-path-main963-final-tests-v1-20261001` has source SHA
`66455fa1564c18544eb6e66e2a8699a29771bc66a9fbd963fdf15a300e5c5264`
and test binary SHA
`bcc7efbc8d77fc8d39f99da94ceb471dd798bc86793adb6b7881d567edd2616b`.
It includes the separate unload correction and has no temporary hash gate.
Its source and binary differ from the diagnostic pair; no byte equality is
claimed. The final three replay tests passed with the new pins.

The first exact final comparison failed only on a caller source position
(`bridge_parity_harness_tests.rs:516:9` became `:520:9`). The explicitly selected
caller-position normalization then passed all 819 rows, including every tick
hash. Changed caller strings number 59 for bridge, 328 for global and 12 for
slice6. There is no actor, gameplay, RNG value/order, function-name, filename
or stack-order normalization. The final raw observation SHAs are:

- bridge: `689d90f4f92f30a2f8d1b2daffe797d4fc677e8a3e151a3cdff66505a44b4dd7`
- global: `c33092d2fe05470e150b91f9a22ddb7961faa991ac3f352713ecfe7b906338d1`
- slice6: `2cba9b36050681984846a8f6a408a7902e4f86d79a4de23b127947030ff708d0`

The final observations, run log/receipt, both comparison logs and labelled
test manifest are pinned as additional inputs. The saved main963 checker now
validates them without a fresh-directory argument; ordinary `--check` also
checks this receipt after the unchanged historical main7412 comparison.
The optional fresh-directory mode remains available and requires explicit
caller-position permission whenever those positions differ.

Root's final validation reported 151 focused tests passed (including selected
retail ignored tests and the unload regression), 9389 library tests passed
with 225 ignored, and clippy passed. The release label
`bridge-path-main963-release-v1-20261001` shares the final source identity.
Retail production captures and native unload comparisons have separate owners;
this synthetic replay receipt does not substitute for them.
