# Jumpjet integration after incoming barracks main

A normal release Nighthawk now completes ordinary Move, moving/untethered Stop,
and landing through the owned Jumpjet instance and independent Foot air caches.
The synchronous occupied-slot callback remains covered by the composed native
controls and Rust comparisons; this single release actor does not realize it.

This records candidate `fef1b0e71c14ee41917476c6a4ff82ff83cd05b6`, after
incoming main `c19e544581` (PR1034) and integration `87705976`. The observer keeps
both air/Walk projections and pending-entry state; private Foot marked-byte swap
coexists with main's Mark(2)/Techno radio callback. Snapshot290 composes those
changes with the two independent native air caches. Production RNG reset is
unchanged. The [summary](summary.json) preserves actual run identities.

## Rust and retail results

All Cargo commands used the shared build owner; every test selected `--lib`.
Required retail INI/assets environment and selected map overrides come from the
existing [invocation plan](../checks/invocation-plan.json).

| Check | Actual result | Binding |
| --- | --- | --- |
| [Full library](checks/full-r1.json.gz) | 9,629 passed / 0 failed / 231 ignored; all four new native comparisons pass | c685 source before four fixture-test repairs |
| [Clippy library](checks/clippy-r1.json.gz) | Exit0, 725 repository warnings | Same unchanged production/Python files |
| [Python tools](checks/python-p2.json.gz) | 575 tests / 0 failures / 5 optional skips | Same unchanged Python files |
| [Selected retail sweep](checks/primary63-r2.json.gz) | 63 passed / 0 failed / 0 ignored / no fixture skips | Clean fef candidate, all1,962 RS/PY sources exact |
| [Four shared-fixture consumers](checks/g8.json.gz) | 4 passed / 0 failed / no fixture skips | Final test bodies; later address-comment typo only |
| [Normal release build](checks/release-r1.json.gz) | Exit0 | Clean fef candidate, all1,962 sources exact |
| [Simulation field ratchet](checks/field-ratchet.txt) | 2,505 versus incoming main2,513 | Authority did not widen |

Only four test leaves changed after full/clippy/Python. The
[final fixture proof](binding/final-fixture-proof.json) retains every changed
leaf, the exact remaining1,958 Rust/Python files, full diff and g8 binding.
The one later comment typo changes no Rust body. The full suite was not repeated
for these focused test-only repairs. Current production consumers were unchanged.

Native evidence was rebound after main, not re-executed: the
[identity check](binding/native-bindings.json) retains all six active corpus files
and26 producer pins unchanged. The [v5 completion](../native/v5/completion-receipt.json.gz)
records the actual original execution, selected constructors/readers,16 controls,
487 steps, full three-stream RNG and raw call/memory evidence. Its declared
supplied world, transport substitutions and whole-call limits still apply.
The [single critic](../review/fresh-critic.md) ran once before late-main integration;
no repeat review was run after conflict resolution and fixture repairs.

## Fresh release production witness

[Discovery](runtime/discovery/run.json.gz) and [Move/Stop](runtime/move-stop/run.json.gz)
each contain6,000 steps/6,001 observations. Both capture checks and strict offline
validators are VALID. The [runtime proof](runtime/proof.json) binds unchanged
source1,962 files, binary SHA256
`cf3da041cc36bebc19ef2e157b9e195d92744b5d5628f30fd772107173d50bfe`,
and retail AnyTown `XMP03T4.MAP` payload SHA256
`7a390de363f79743dd54897a49302869a795f839f3387ff03e8c0b70a519e17e`.

SHAD1516 is created in limbo4601, exits5196, releases its factory tether5415 and
settles5497 in Cell31,93. The [input selection](runtime/input-selection.json)
uses this measured identity, stock type registry and idle/untethered state.
Move5600 is admitted5601 toward Cell30,93. Stop5620 arrives during phase1,
moving and untethered; NavCom clears5621. Its selected landing coordinate equals
the retained destination `[7808,23936,416]`. Grounded idle completes5679 in
Cell30,93 with both caches NativeNull, no tracker bucket and no slot holders,
and stays idle through6000. The path has17 distinct horizontal positions and
maximum physical Z791. Actual sample cost76,054 includes three terrain probes.

The first5,601 declared observed actor/house/clock rows equal discovery. This is
not a whole-world per-frame hash comparison. Terrain probes report level4,
slope0/no bridge/no overlay and `walkable=false`; no native land-type or entry
parity is inferred. Native comparator and pixel parity certification are NONE.
The [actual GPU bytes](runtime/move-stop/child-output/frame.bgra.gz) and
[lossless inspection PNG](runtime/move-stop-view.png) are retained; owner visual
inspection found the normal tactical/base output without a diagnostic failure.
The earlier cff SHAD1528/timings remain separate historical evidence.

Use the existing [map observation owner](../../../map_observation.md) with the
saved [discovery profile](runtime/discovery/profile.json) or
[Move/Stop profile](runtime/move-stop/profile.json), current retail config and a
fresh output directory. The normal binary is resolved from the build manifest;
config, executable and retail assets are not copied into this archive.

## Preserved failed diagnostics and corrections

The [first retail sweep](checks/primary63-r1-failed.json.gz) failed57/6, at fixture
spawn before mechanism assertions. [r2debug](checks/r2debug.json.gz) reproduced
those failures. [Hills causal evidence](causal/hills-engineer.md) distinguishes
missing live registry on legal placement from an already-resident Rock/Foot0
cliff negative control; the existing placement scope is closed before snapshot.
Passenger/Engineer behavior assertions remain intact.

[Rocketeer placement evidence](causal/rocketeer-placement.md), the preserved
[r3 as-run source](causal/r3inputs-as-run.rs.gz) and actual input logs record
power buildings left by earlier failed JJ candidates. The corrected fixture
selects an empty flight corridor first, uses the registry and places two distant
plants through the existing building-foundation admission owner.

The [r4 fire witness](causal/fire-event-witness.json) records seven actual
SimFrameOutput fire events, not timer guesses. [Original caller/pose evidence](causal/rocketeer-fire-pose.md)
shows firing latch clear before FireAt, followed by the movement suffix:
phase3 below native0.8 can end in Hover; phase2 retains FireFly. Saved executable
leaf controls also establish these poses. Existing production owners already
matched. Tests retain FireFly, real discharge/target, range, speed, kill and
landing coverage. G6/G7/R3/R4 failures remain failures in the receipts.

The [first analysis script](runtime/failed-analysis/proof-first-as-run.py.gz)
incorrectly required Stop to select a different destination. Capture and offline
validation had passed. The [failure receipt](runtime/failed-analysis/receipt.json)
and [original Stop instructions](runtime/stop-native-static.json.gz) preserve the
correction: the successful54B5DF..54B683 path calls MoveTo with the selected
nearest cell without a retained-goal inequality condition. The corrected proof
records the actual false destination-change boolean and still requires NavCom
clear, retained motion, physical progress and complete landing cleanup. This
static reading is not a newly executed native comparison for the capture.

The original111 entries were exact before extension; their
[manifest](before/archive-sha256.json.gz) and [README](before/README.md.gz) are
preserved byte-for-byte. The new [archiver as run](archive-as-run.py.gz) records
this extension. All1,962 Rust/Python files stayed exact while archiving.
The [parent contract](../README.md#remaining-boundaries) retains #689's living
Fly/queued/object-order adapters and the inherited landing-on-crate effect/RNG
mechanism. No exhaustive movement/height audit completion is claimed here.
