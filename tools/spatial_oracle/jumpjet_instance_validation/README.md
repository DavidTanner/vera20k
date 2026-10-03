# Jumpjet synchronous callback and Foot air ownership

Stock SHAD's State1 occupied-slot callback now completes Unit `741970` → Foot
`4D94B0` → Jumpjet `54B1C0` before Process applies its remaining Mark/phase work.
The installed instance is published before the callback and reacquired afterward.
State3 reads the destination that callback left behind. An absent movement
projection no longer manufactures another Stop order.

The [Jumpjet owner](../../../src/sim/movement/jumpjet_movement.rs) keeps its six
instance fields private and owns the nested flight kernel. The
[world host](../../../src/sim/world/jumpjet_cruise.rs) delegates through that
instance instead of recording effects for later replay or committing individual
fields from an older copy. [Foot air ownership](../../../src/sim/movement/foot_air.rs)
keeps independent native `+560` tracker and `+564` slot caches, spatial enter
order, Cell slot notification and selected destruction cleanup. Same-bucket
crossing updates the cached cell while retaining list order. Slot replacement
and release follow the native notification ordering, including same-owner and
empty-slot cases.

Object SetZ `5F6060` uses the shared
[marked height owner](../../../src/sim/movement/ground_pose.rs); it does not
substitute Foot SetLocation. Unit touchdown executes its class destination,
PerCell, slot and tracker callbacks before retiring phase4. Consumers use the
owners' getters. Snapshot290 saves both independent caches and the complete
instance; the hash preserves prior feeds and adds distinct non-null cache feeds.

## Current merged candidate

[Late-main results](late-main/README.md) bind clean candidate fef1b0e7 after
incoming barracks main c19e544581: full library9,629, selected retail63/no skips,
Clippy exit0 and Python575. Four fixture-test repairs preserve all1,958 other
Rust/Python files. Fresh normal release discovery/Move/Stop each validate6,000
steps/6,001 observations with measured SHAD1516 and completed landing/cache cleanup.
Snapshot290 follows incoming main289. The single critic remains the original
one pass; no second review was run. The cff results below are preserved history.

## Native body, caller and data evidence

The [existing producer and coverage contract](../jumpjet_states.md) execute
original `gamemd.exe` SHA256
`1cdd1180e49024fbda8ad568caac2e86e856063ff67ab38f62b7d2c7bb84298c`.
Original constructors and selected layered type readers establish the supplied
SHAD inputs. The composed controls execute Process, class setters, Mark,
location/height, Cell slot, tracker, active-game UnInit and the selected cached
slot destructor block. All supplied boundaries and substituted services remain
declared in that contract.

The [v5 completion receipt](native/v5/completion-receipt.json.gz) records actual
write/check success and unchanged Object24/CMIN50 auxiliary checks. Its sixteen
controls contain 487 steps, 974 compact boundaries, 3,703 raw boundaries, 3,865
matched native calls and eleven RNG draws. All fifteen prior controls, including
their full raw boundaries, and the ten legacy rows remain exact. The active
expected projection and full raw archive have one publisher beside this folder:
[expected JSON](../jumpjet_states.json), [metadata](../jumpjet_states.meta.json),
[raw gzip](../jumpjet_states.raw.json.gz) and
[raw receipt](../jumpjet_states.raw.meta.json).

The supplied snapshot history reaches the scatter result before Scenario Load's
original `689470` → `683560` → Random Seed `65C6D0(0)` handoff. V5 executes that
existing Seed owner and 59 following visits. Full Foot/Jumpjet bytes and Main/
Mapgen RNG state remain unchanged across the reset. The
[Rust comparisons](../../../src/sim/world/jumpjet_cruise_native_tests.rs) save
before Load, assert full loaded Seed0 plus retained Main/Mapgen, align the
uninterrupted source through existing `SimRng::new(0)` and compare strict complete
hashes, caches, motion and all three RNG streams through touchdown. Production
RNG reset behavior is unchanged. This does not execute whole native Save/Load or
COM class persistence.

## Historical cff Rust and production validation

[Summary](summary.json) and [byte identities](archive-sha256.json) bind all saved
logs/receipts. The earlier [focused G5 comparisons](checks/focused-g5-receipt.json.gz)
passed 83 tests, with zero failed and four ignored, before main integration:
HEAD `7e932b968e9b901142d80944095af68fe178ebdc` plus dirty source SHA256
`43212a575deff6570dadb14aa02b4956a2f596805157af15ecbd977884c43d01`.

Required-retail checks on merged candidate
`cff0947c329f25947c7eeba1056ff5d94d31375e`, after incoming main `cf4317b334`, are:

- [Selected retail integration sweep](checks/primary63-receipt.json.gz): all 63
  passed, zero failed/ignored and no fixture skips.
- [Full library](checks/full-receipt.json.gz): 9,590 passed, zero failed, 230
  ignored; every Cargo test uses `--lib`.
- [Clippy](checks/clippy-receipt.json.gz) and
  [normal release build](checks/release-receipt.json.gz): exit0.
- [Python tools](checks/python-receipt.json.gz): 559 tests, zero failures, five
  optional skips.
- [Field ratchet](checks/field-ratchet.txt): 2,505 crate-visible simulation fields
  versus 2,513 on incoming main.

All 1,939 Rust/Python sources stayed exact across final validation and review;
the full library run executed all four new native-comparison tests successfully.
Focused/native prerequisite work before main integration has its own source pins.
The later [coverage annotation proof](review/annotation-proof.json) records one
module-header comment addition, byte-identical Rust body and 1,938 unchanged
other Rust/Python files. No production behavior or active native payload changed.
Adding this evidence archive changes the repository's full source identity.
The [archiver as run](archive-as-run.py.gz) preserves the original 107-entry
byte-preserving publication; later review/documentation entries extend its manifest.

The [release proof](runtime/proof.json) records normal AnyTown discovery and
Move/Stop runs, each 6,000 steps/6,001 observations, with capture and strict
offline validation both VALID. Binary SHA256 is
`dc819fd4ad01fc4b6735b11ee0ba4e9f9a68f6b8fba4c62ee1defa1d8c00a021`.
SHAD1528 appears in limbo4601, exits GAWEAP5195, releases its factory tether5419
and first settles5501. Ordinary Move5600 is admitted5601. Stop5620 occurs during
phase1 while moving and untethered, changes retained destination5621 and lands
at5725 in Cell32,93 / XYZ `[8320,23936,416]`, with both caches NativeNull, no
tracker bucket and no slot holders. The post-Move path has 73 distinct horizontal
positions and maximum physical Z866. The first 5,601 observed actor/house/clock
rows equal discovery; the 5,998 retained partial-retry rows equal the final run.
These comparisons have declared fields rather than whole-world per-frame hashes.

The actual [GPU frame](runtime/move-stop/child-output/frame.bgra.gz) and
[inspection PNG](runtime/move-stop-view.png) are retained. Native comparator and
pixel parity certification are NONE. One SHAD does not realize a foreign
occupied air slot; the composed native/Rust comparisons cover that callback.
End-frame observations cannot establish internal callback PCs/order or individual
RNG draws. The two terrain probes report slope0/level4/no bridge and
`walkable=false`; no Clear land-type or native terrain admission equivalence is
inferred from them.

## Reproduction and preserved failed attempts

Use the [native producer instructions](../jumpjet_states.md) for executable
comparisons and [chosen-map capture owner](../../map_observation.md) for release
observations. Build through `python -m tools.cargo_run -- build -p vera20k
--release --bin vera20k`, with retail config and `graphics.upscale=false`. Run
the ordinary owner against the saved [discovery profile](runtime/discovery/profile.json)
or [Move/Stop profile](runtime/move-stop/profile.json), supplying absolute profile,
contract, checkout and fresh output paths. Configure the installed retail assets;
machine-local config and the executable itself are not copied into this archive.
The [invocation plan](checks/invocation-plan.json) and
[measurement script as run](checks/measurement-as-run.py.gz) preserve actual
retail environment, selectors and source bindings; the plan alone is not evidence.

[Red-r0](regression/red-r0.json.gz) is a compile failure and is not bug proof.
[Red-r1](regression/red-r1.json.gz) is the actual failing regression: after
`process_50`, Rust retains `[3712,2688,0]` where original execution yields
`[3456,2944,0]`. G1/G3/G4 are superseded diagnostic runs. V3 omitted Object CRT
startup and left `AC13C8=0`; its [cold corpus](native/v3/cold-object-prerequisite/)
is preserved. V4 reuses the existing full fourteen-entry CRT initializer and
pins original scalar104. The water fixture also required an empty IsoTile catalog
with cached Water over base Clear, not a changed base terrain. G4's remaining
snapshot failure compared intentional Load Seed0 against an uninterrupted
Scenario31 history; V5/G5 correct that test assumption while retaining strict
hashes and production behavior. Earlier native receipt-assembly and auxiliary
input errors are preserved, not promoted to successful comparisons.

The [first](runtime/failed-budget-r0/) and
[second](runtime/failed-budget-r1/) Move/Stop captures exceeded the existing
100,000-observation budget with thirteen/seven terrain probes. The second stopped
three frames short. They are not VALID/GPU evidence. The final run preserves the
same gameplay inputs with two probes and actual sample cost70,053.

## Remaining boundaries

Issue [#689](https://github.com/YuriPlanet/vera20k/issues/689) remains open for
Jumpjet object-order `Some(goal)` reconciliation and living Fly/queued adapter
consumers, their admission/lifetime and hash risks. NavCom identity, sampled XYZ,
paid head, Foot path queue and NavQueue retain their distinct owners. The selected
direct Process controls include declared retained/unmarked supplied states; they
do not establish stock reachability or whole native Unit-AI/lifecycle-wrapper
equivalence. Larger existing crate/PerCell mechanisms, stock-false balloon/
deployment branches, real Cell00/NativeNull aliasing and whole destructor behavior
remain bounded by their existing evidence contracts. No whole movement/height
audit completion or exhaustive gameplay parity is claimed here.

Native touchdown calls Cell PickupCrate `481A00` at `54C9F6` after phase0.
Movement crate pickup/effect dispatch is an existing separate unfinished
mechanism: landing on a crate skips its selection/effect/RNG chain in Rust.
The composed controls and release landing cells contain no crate and cannot
establish that branch. This inherited residual stays with the current
[crate owner](../../../src/sim/crates/pickup.rs).

The [single fresh read-only critic](review/fresh-critic.md) found no blocking
implementation defect in this bounded chain. Its focused-G5 attribution finding
is corrected above, and the inherited crate gap is now also documented beside
the host. The report preserves the reviewer's original machine-path references;
current owners and evidence are linked in this README. No repeat review was run.
