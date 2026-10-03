# Single fresh read-only critic pass — 2026-10-03

Reviewer: `/root/rally_critic`; reviewed candidate after native comparisons,
full retail lib plus focused corrections, Clippy, Python checks, actual GPU
readbacks and five production captures. It performed no implementation or Cargo.

Confirmed P2: the new rally render call acquired selection through a quadratic
membership scan (`ordered.contains` for every selected actor), even for 20,000
mobile actors producing no rally geometry. Earlier timings began after selection
acquisition. The same pattern affected post-sim reconciliation; repeated front
insertion made armed-selection recovery quadratic too. Fix the shared selection
owner, preserve native ordering, and measure acquisition plus rally production.

No other confirmed defect in the examined ordinary stock-factory chain. The
review traced input preparation, selection dispatch, Archive/Stop ownership,
coordinate-query authority, drawing gates/order, palette/A consumers, shroud
cache invalidation, camera upload and capture admission. Original caller bytes
support both rally passes and the later action-line call.

Independent checks passed: native rally regeneration, all five portable capture
comparisons, validation-log byte identities and git diff --check. The reviewer
inspected saved Rust/GPU/Python results without rerunning Cargo.

Coverage is bounded: no certification of native building occlusion, whole
Scenario/revelation history or ordinary mouse-input production behavior. Other
procedural families, enabled fog/dynamic AlphaShapes, RGBA/FX composition,
planning, EMP reachability and ConYard repacking remain open. The documented
minor clipping/camera-edge residuals were not additional findings.

Owner disposition: accepted. Shared membership indexing and deque insertion now
replace the quadratic paths. Ordering was rechecked against ObjectSelect5F4520
and ReadINI715793..7157AE original bytes. Full retail revalidation passed9651 tests (238 ignored), Clippy passed, and
the expanded20k-mobile workload passed. Selection plus rally preparation fell
from16.5765ms to2.0991ms with a complete ledger, and34.5089ms to4.3806ms
when recovering a missing ledger. These are20-sample optimized means, not
whole-match FPS. Source and raw timing logs are retained in this directory. No repeat critic was requested.

Final production validation: release v4 `c3aaa9ba6e930ca359cbddc1f2b7a7569e148f6b28365d6947fa95a830b72404` passed a fresh1500-step
retail capture. All480,000 pixels and1501 state boundaries match v2; the portable
five-capture archive passes its native106-store rally mask/floor comparisons.
The owner inspected the actual GPU image. No runtime Rust changed afterward.
