# Procedural gameplay drawing

Goal: match active-retail `gamemd.exe` across all required procedural gameplay
drawing. Examples are not scope limits. Missing or unproven required behavior
keeps the goal open. Complete one common gameplay chain per PR, with original
execution comparisons, production output/performance and one fresh critic.

## Current checkpoint

- Worktree `/Users/halvor/.codex/worktrees/procedural-drawing/vera20k`, branch
  `feature/procedural-unit-attack-lines`, base `58f3fd9dbac162eed267850edb1e8cfe5d714028`.
  Move [PR1047](https://github.com/YuriPlanet/vera20k/pull/1047) merged at that
  commit; its implementation is d5b667327 and final evidence/review is8fd0dd330.
  Ratchet CI passed. Issue689 now preserves only the remaining queued, Jumpjet
  object and living Fly work. Attack implementation passes focused validation;
  production capture, final full checks and the fresh critic remain pending.
- Current common chain: selected local stock MTNK attacking a ground Unit,
  planning off. Extend the existing action-line and combat coordinate owners:
  native source6F3D60 is the body TurretOffset pivot; target70BCB0 leads moving
  Unit targets from their live motion/speed/facing and the current weapon.
  FireAt is an affected consumer and must retain one shared query. Original
  event/target lifecycle, ART reads, pose inputs and actual in-game output are
  prerequisites, not optional follow-ups. Research lives under ignored
  `logs/procedural-drawing/attack-research/` until promoted through existing owners.
  Native action and prerequisite independent checks pass:31 Attack raster cases,
  8 retained aim queries,2 full FireAt controls; all289 prior cases unchanged.
  Shared combat aim, distinct FireAt argument speed, expiry full RNG/timer and
  retail reader checks pass. First optimized run62 passed/5 fixture failures;
  fixed the synthetic rules chronology and legacy test interner, then all27
  rendering tests passed, including1256 action-line GPU draws and rally consumers.
  Python map-observation tests pass105. Workload report is
  `logs/procedural-drawing/unit-attack-workload-first.json`:20k selected moving
  target Attack construction11.909ms, CPU staging0.0357ms, completion1.540ms,
  34 spans/4216bytes. Synthetic overlap,20 samples; GPU intervals remain null.
  Clean release v1 from`ee20ca9fe`, exeSHA4b4a83ceed0f074bab7dd0461c2bfef33f6e26a55c5842e8f43601eaad06e18c,
  passes five captures. All188 stationary/101 moving native stores match; both
  deselected controls draw zero stores. Native source-origin controls cover21
  stable slopes plus one zero-angle active transition; differing-slope translation
  clears have instruction-level proof, not an exhaustive interpolation corpus.
  All320 prior rows stay unchanged. Independent native checks pass;14 portable
  production tests pass. The archive retains40 gzip files, native input bindings,
  both matched pairs and a480000pixel/1299boundary observer-off comparison.
  The shared archive now hashes only the native image and consumed case set;
  original Move payload provenance is retained and all compressed Move captures
  stay unchanged. No native/GPU/capture work remains active. Next integrate the
  fetched docs-only main update, run final full retail lib/Clippy/Python checks,
  then one fresh read-only critic and the mechanism PR. No Attack review yet.
- Prior factory-rally chain [PR1042](https://github.com/YuriPlanet/vera20k/pull/1042)
  merged at2135df0e33536ab11fddbdcdb8dae3a3b5f316f9. Its native comparisons, production archive,
  performance reports and one critic disposition are in
  `tools/procedural_drawing_oracle/`. The whole-scope goal remains active.
- Active-retail image SHA256:
  `1cdd1180e49024fbda8ad568caac2e86e856063ff67ab38f62b7d2c7bb84298c`.
  Ghidra selector `gamemd.exe`; no database edits. Two new decompilation requests
  timed out/reset; original-byte `tools.native_inspect` and Unicorn remain usable.

## Integrated Unit Move evidence

Selected local ground Unit, ordinary Move, planning off: input dispatch restarts
70D150; Tactical6D4750 walks forward TechnoArray with House50B6F0, selected and
UnitActionLines gates; Foot4DC060 admits positive CdTimer remaining and reads raw
Location, NavCom or last queue target+48. In-Size flag100 targets use578080 ground
plus Foot's initialized416 deck height. 7049C0 emits two point-minus2 three-pixel
endpoint squares, then solid7BA5E0/7BA610. Lines read/write neither A nor Z, use no
RNG and perform no detach. The shared timer has signed wrapping frame arithmetic,
paused sentinel and successful-load reanchoring, not saturating u64 tick age.

Native evidence now has 277 prepared-input controls plus 12 production rows.
The original 277 remain unchanged after extending the existing surface fixture
with bounded optional dimensions; independent Rally/shroud payloads remain exact.
Production rows bind capture/profile/frame/executable hashes to unclipped 160×160
native crops. All source inputs, map Size80×85, target terrain, PALETTE.PAL and
coordinates have explicit native/retail provenance; no whole Scenario claim.

The Rust owner shares solid/clipping with rally and radar, and rectangle421B60
with map playfield. It reads physical PALETTE.PAL through PaletteLight, uses an
isolated terrain query and CdTimer/binary_frame, and consumes the existing Unit
setter for ordinary Move(clear_queue1). Empty band release and consumed empty
context dispatch restart the timer; successful load reanchors retained state.
Status/brackets now finish before effects, bandbox and selected action lines,
as the original6D8DB0/6F5190 caller sequence requires. Range rings remain for the
next range chain, rather than being certified by the ordinary MTNK path.

Validation: final integrated optimized88 tests passed. The CPU producer compares120inputs
(115exact,5documented1pxclip controls),6mixed opaque orders,77solidleaves,
45intersections,11timer prerequisites andpalette256row. GPU compares126inputs
at2sRGBformats×4zooms with unchanged depth (1008draws);40clip-control draws are
exact CPU-raster presentation, not exactnative claims. Native destination/input,
capture-schema, map and radar checks are included. Logs:
`unit-move-main-render-checks.log`, `unit-move-main-full-lib.log`,
`unit-move-main-clippy.log`, `unit-move-main-python-runner.log`.

Production release label `procedural-unit-move-production-v1`, executable
SHAdc06728fb081e18054a28e9e9aa61e5ca6e7e3e82a20acc9052ac2471e6653dd.
Twelve immutable XMP03T4 captures exercise real Select/Move gestures, timer1/0,
reselect, empty bands, Stop and arrival. Every native opaque store matches.
Matched-time Move/Stop/reselect controls change only165/165/151native pixels;
arrival/reselect changes zero tactical pixels and resets only the sidebar tooltip.
Archive: `tools/procedural_drawing_oracle/unit-move-production-validation/`.
The current release also reproduces every480000pixel and1501state boundary of
retained rally v4; see `validation/unit-move-rally-consumer.json`.

Final release v2 was built from clean implementation commitd5b667327 (57.82s).
Executable SHA776a416bc6645aa0da3137b207012fd5cf51a3516e99a81fb4da79a5e7d9c4c5.
All12 original scenes replay exactly through the ordinary renderer and real input
gestures; the archive now retains both sets (192gzip files). Each pair passes
the shared map-observation comparator, including input bytes, diagnostic clock,
atlas, final frame and all385 state boundaries. Active/stopped/reselected/arrived
frames were visually inspected. The final rally replay matches v4 completely.
Logs, manifest and comparator results: `validation/unit-move-checks/receipt.json`.
Initial capture-wrapper errors concerned a missing ignored output parent and
launched no child; creating that directory was the only repair, with errors saved.

Performance: native opaque composition in Techno order reduces the20k overlapping
Move stress from979995spans/121519380bytes to63spans/7812bytes. Mean CPUbuild
14.422→8.266ms; CPUstaging10.939→0.025ms; completionwall27.244→1.538ms (AppleM4,
20samples, optimized, not whole-gameFPS or GPUduration). GPUtimestamps are null.
Before/after samples are retained under `validation/action-line-workload-*.json`.
The final integrated optimized run is retained separately as
`validation/action-line-workload-final.json`.

After main integration, native Move/Rally/shroud payload and metadata checks pass
unchanged. The final full Python suite passed594 with5 optional skips; the
extended archive replay comparator passes6 focused controls. Its final-candidate
mode retains the twelve original native-bound captures plus twelve new complete
frame/state replays, rather than silently replacing the original observations.

The single fresh read-only critic and owner disposition are retained in
`tools/procedural_drawing_oracle/validation/unit-move-review.md`. No runtime
changes were needed after validation. PR1047 is merged; selected Unit Attack
anchors and their required consumers are the next chain. Planning, other Foot paths and the wider
procedural census remain required; documenting them does not close this goal.

## Integrated rally evidence

Factory6DA9D0/4C0750:65 whole cases,4 overlap cases,4 retained clipping controls,
360 leaves and2 production crops;21 House colors;27 input cases plus2 unsafe-frame
controls;28 Dummy/passability cases;110 destination/Stop cases;73 SHROUD rasters,
256 masks,16 flags and9 prepared shroud scenes. Shared CPU/GPU A composition uses
per-blitter palette rows; camera uploads after clamp; rally passes straddle objects.

After the critic's selection-scaling fix: full retail lib9651 passed/238 ignored;
Clippy passed; Python580 passed/5 optional skips; field ratchet2505/2505. Native GPU
comparisons covered BGRA/RGBA sRGB, both passes, zoom0.75/1/1.5/2, camera and poisoned
depth. The final release v4 clear capture matches every480000 pixel and all1501
state boundaries of v2. Five immutable portable captures and native crop checks
pass. Covered stores do not establish full native object occlusion or shroud history.

20k selected mobile acquisition+rally build mean16.58→2.10ms; missing-ledger recovery
34.51→4.38ms (M4, optimized20 samples). Shroud immutable row spans reduced5120×2880
CPU raster16.78→2.49ms unrevealed and8.76→1.44ms frontier. GPU timestamp intervals
mostly unavailable; no GPU-speedup/FPS claim. Existing unrelated timestamp benchmark
issue [1037](https://github.com/YuriPlanet/vera20k/issues/1037) remains open.

## Preservation and next safe action

- Keep comparison label `procedural-rally-production-v2` (SHAedb373c72f58c45a6b79cdd4d74d7923425efecb5c03e70cbb56f52f3ae45e31)
  and final v4 (SHAc3aaa9ba6e930ca359cbddc1f2b7a7569e148f6b28365d6947fa95a830b72404).
  Exact v1/v3 retirement preview and application passed, removed63492096 allocated
  bytes with no errors; owner receipt `owned-builds/label-retirements/1791067591733474000-8ce080cc9d7446589027b086d205ab97.json`.
  Post-merge owner cache trimming passed: no eligible files, zero removed,39.0GB
  available. Receipt `owned-builds/retention/1791068207876860000-5784b2d7.json`.
  Original capture directories remain untouched.
- Preserved stash `b4474c90468940ace26bfe3a84fefb682b767abe` and all ignored native,
  capture/failure/performance logs remain. v3 portable archive is preserved under
  ignored logs; tracked archive keeps the four v2 controls plus final v4.
  Integration stash `4b759e701ea67bbc4ff229e4427dd543aceda55b` is also preserved;
  its tracked and untracked contents were applied to the new main base.
- Cargo only through `/Users/halvor/Documents/vera20k-dev/.venv/bin/python -m tools.cargo_run`.
  Source/evidence freeze is required. Use the narrowest useful check; no baseline
  full suite. Platform workflows are manual and not requested; only ratchet is an
  automatic merge requirement.
- Keep both Move v1 (original native-bound captures) andv2 (final candidate)
  labels as active evidence; do not retire either or their original runs.
- Move post-merge retention preview passed with no errors or removals; receipt
  `owned-builds/retention/1791078381102958000-3371161c.json`. All four retained
  rally/Move labels still support active comparisons; none is superseded.
- Next: finish original Attack prerequisite/consumer evidence and implement its
  shared coordinate queries, then native/Rust/GPU/production validation and one
  fresh critic. The Move archive and final Python rechecks pass.

## Whole-scope residuals

The baseline native/Rust censuses in `docs/research/procedural-drawing-*-audit.md`
identify23 families, not completion. Attack/other Foot paths, selection/bandbox,
range/placement/SW indicators, laser/Tesla/RadBeam/Disc/Wave/Spark effects, searchlight,
ion distortion, psychic/capture/planning links, beacons/pixelFX/radar and their
required producers/lifecycles still need complete chains. Planning, enabled fog,
dynamic AlphaShapes, EMP input and ConstructionYard repack are required broader
mechanisms. Attack-only source/aim stays an explicit residual in target_lines until
its existing native owners are connected in the next chain. Minor chop/nearest clip
and fractional far-edge A differences remain one-pixel visual residuals.
