# Allied depot waiter independent review

2026-10-03. One fresh read-only critic under AGENTS.md and
`.agents/skills/_shared/review.md`. The critic did not implement this chain.

Reviewed `feature/native-allied-depot-waiters`, HEAD
`cd5c1124f8f44dc4b39865b5c6c677e51e99c090`, against fetched main
`7e932b968e9b901142d80944095af68fe178ebdc`, including the uncommitted
installed-Drive getter migration, validation receipt and indexed evidence.
No source, Git, Ghidra, external issue or Cargo mutations were performed.
This report is the critic's only saved file.

## Findings

No confirmed actionable defects or merge blockers in the reviewed scope.
No additional in-scope refactor is warranted by the inspected consumers.

The only production change is the read-only pending-entry projection in
`src/app/diagnostics/tactical_capture/map_observation.rs:768`. Its validator
accepts an absent historical field, null, or a positive u64 target identity;
it rejects bools and invalid numeric identities. Comparison remains strict.
The old ignored unmarked/batched reproduction is removed and replaced by the
complete-frame test. The path-only test seam reuses the existing seam and
shared entry receiver; it does not introduce an admission implementation,
gameplay state authority, duplicated native port or widened simulation fields.

## Issue 937 disposition

Closing #937 is justified for its current title and body: missing production
Building Mark and ordinary frame scheduling in the saved waiter reproduction.
`building_dock_frame_tests.rs` constructs the depot through Reveal/Mark, checks
all nine occupation cells, advances ordinary complete frames, exercises both
prepared live Logic orders, and checks service release followed by the first
subsequent pending Foot admission. All three eventually repair, detach and
leave the foundation. The retail profiles separately exercise Queue/Place,
ordinary combat damage, full travel/service turns and departure.

This disposition does not establish a queue bug was fixed or certify the entire
native crowded trajectory. The crowded capture leaves the third tank at 40 HP
on the foundation, retains its request until the incumbent releases and then
clears it. Controlled original Unit/Drive/NULL/retry boundaries reproduce the
corresponding prepared stop/refusal result. Original FindPath, paid track travel
and whole Logic execution for that retail approach remain unproved. The receipt
preserves this trigger, effect, frequency limit and downstream risk. Keep that
qualification in the PR and issue closure. #1023 remains separate.

## Evidence checked

- Independently reexecuted the current `building_repair --depot-waiters --check`
  against active-retail executable SHA256
  `1cdd1180e49024fbda8ad568caac2e86e856063ff67ab38f62b7d2c7bb84298c`.
  It passed, with no files written. All 23 current sidecar source pins match.
- Inspected the native reader order, Mark, Logic loop, Foot retry tail and
  post-FindPath instruction excerpts. The corrected admitted membership and
  broad LocalSize are declared inputs; the rejected narrow/false-membership
  diagnostic is not used as a golden. The golden records exact timers,
  retries, queue/path/head/selector/latch state, real Unit code 6 responses,
  executed distance 256 and native RNG observations.
- Traced production retry to `world/object_turn.rs:920`, after the object's
  movement. The new tests call the existing rules, Mark, mission, radio,
  destination and movement owners. Installed Drive reads use the selected
  runtime's retained getters after #1031.
- Read actual final logs: strict-retail lib 9574 passed, 0 failed, 230 ignored;
  Clippy finished successfully; Python 85 tests passed; field ratchet 2513/2513;
  release build completed. All three new Rust tests appear as passing in the
  full log. No redundant Cargo run was launched by this critic.
- Independently checked 94 hashed references in `allied_waiters` and the
  published runtime index's 43 artifacts (54 referenced objects including
  summaries/build provenance). No missing files, byte-count mismatches or
  SHA256 mismatches were found. `git diff --check origin/main` passed.
- Read the actual strict comparison reports and command receipts. Both
  cross-version reports are MISMATCH with only initial/final state-hash
  differences, no errors and parity certification NONE. The same-current-
  binary crowded repeat is MATCH with no differences or errors. No comparison
  uses normalization or legacy relaxations. The #1031 hash-layout explanation
  is explicitly a source-backed inference for these depot values, not a
  dedicated depot legacy-hash execution or full hidden-state identity proof.
  Inspected #1031's actual hash/snapshot diff: the two entity Option feeds are
  removed, retained runtimes enter locomotor payload hashing, and snapshot
  version changes from 287 to 288. The publication draft preserves the limits.
- Visually inspected saved Allied opening6094, loop6330, closing6644 and the
  integrated southern8500 endpoint. These show distinct A/B/C phases and the
  final idle depot with the three tanks off the pad. The receipt clearly
  separates intermediate animation-state observations, endpoint Rust GPU
  equality and unestablished native pixel equivalence.

## Coverage limits

This review supports the scoped production reproduction, regression coverage,
read-only observability and prepared native component comparisons. It does not
certify all depots/units, native whole-match scheduling/path search, complete
hidden-state equivalence, native rendered pixels or whole-game determinism.
No additional confirmed unrelated defect was found during this pass.
