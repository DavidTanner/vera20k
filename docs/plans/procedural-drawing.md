# Procedural gameplay drawing

Goal: match active-retail `gamemd.exe` across all required procedural gameplay drawing.
The examples are not scope limits. Missing behavior or unproven required consumers
keep the goal open; recording them never completes it. One common gameplay chain
per PR, with native execution, production pixels/performance and one fresh critic.

## Current checkpoint

- Worktree `/Users/halvor/.codex/worktrees/procedural-drawing/vera20k`, branch
  `feature/procedural-target-lines`, HEAD/base `7229c2df25` (latest documentation/manual-CI integration).
  Implementation/evidence remains uncommitted; no PR yet. The one fresh critic
  completed; its single selection-scaling finding is fixed and revalidated.
- First chain: selected local factory rally, Tactical6DA9D0 through DSurface4C0750.
  Reverse selection order; live/selected/local/HasRally/ArchiveTarget gates;
  GetCoords foundation center; target ground/bridge projection; black/color/color
  patterned rows; signed frame phase; sequential clipping; complementary A passes.
  No RNG draws, timer writes or detach. ArchiveTarget remains simulation-owned.
- Required input/cleanup: Building443860 FNPC preparation, selection dispatch order,
  Building455D50 Stop clearing, and shared Dummy terrain-speed admission. Existing
  owners now cover these; rendering uses an isolated NativeCellQuery identity.
- Shared prerequisites: native House RGB through existing PaletteLight (also radar);
  stock SHROUD raster/selector; one CPU/GPU A source with per-blitter palette rows.
  Fullscreen destination multiplication is removed. Rally uses two pooled UI draws
  around the object/effect passes, bypassing depth. Camera is published after clamp.
- Native image SHA256 `1cdd1180e49024fbda8ad568caac2e86e856063ff67ab38f62b7d2c7bb84298c`.
  Ghidra selector `gamemd.exe`; no shared database edits. Reproducible evidence lives
  in `tools/procedural_drawing_oracle/` and uses Unicorn2.1.4/FPCW0E7F/RGB565.
- Corps:65 whole rally cases,4 overlap-order cases,4 retained clipping controls,
  360 leaves and2 production crops;21 house colors;27 input cases plus2 unsafe-frame stack controls and
  28 direct real/Dummy passability controls;110 destination/Stop cases;73 SHROUD
  rasters,256 masks,16 flags and9 whole native shroud scenes.
- Whole-scope baseline censuses: `docs/research/procedural-drawing-{native,rust}-audit.md`.
  They identify23 families and are not completion certificates. Later work includes
  action lines, selection/range/placement, lasers/bolts/waves, psychic/planning links,
  light/distortion/particle/radar/beacon paths and required producers/lifecycles.

## Actual validation

- Solid-line regression failed first. Focused retry:219 passed,3 failed. The two
  empty-INI fixtures and bridge voxel zoom-padding edge are now corrected; the
  subsequent optimized run passed their checks, plus rally GPU comparison.
- Optimized run:21 passed,1 unrelated tree benchmark failed on `last > first`
  timestamp assertion. Existing main code; recorded as tooling/refactor issue1037.
  Log `logs/procedural-drawing/release-validation.log`. Own measurements completed.
- Native raster/GPU comparisons cover all65 rally inputs, BGRA/RGBA sRGB, both
  passes, zoom0.75/1/1.5/2, nonzero camera and poisoned hardware depth; native Z
  remains unchanged. Shared A composition and6 ordinary native shroud scenes passed.
- All28 real/Dummy comparisons,25 defined input clicks,24 native Stop rows,
  connected input-to-archive-to-Stop,21 house colors, selection ordering and retail
  terrain-cost construction passed. Python map-observation95 checks and full580-tool suite passed (five optional skips).
- Simulation field ratchet2505 versus2505 at origin/main. Five production captures
  and portable native comparison passed. Full retail lib found one stale draw
  assertion (9648 passed,238 ignored); its fix passed the44 rendering checks.
  Clippy and122 final cleanup checks passed. After the critic fix, full retail
  lib9651 passed/238 ignored, Clippy passed, Python580 passed/five skipped and
  the field ratchet2505/2505 passed. The final release capture/native archive also passed; publication remains.

## Measured performance

Apple M4/Metal, optimized20-sample means. Rally builder1/64/1024 factories:
0.0044/0.1146/2.3192ms; upload/encode0.0255/0.1165/1.4477ms. Synthetic stress,
not whole-game FPS. GPU completion-wall means1.273/1.339/6.201ms include fence/map.
Timestamp intervals are mostly unavailable; surviving samples are not a valid mean.
The20k-instance A-blitter completion bound was3.835ms neutral versus3.780ms live A.

Shroud CPU raster cost before caching:1.061ms at1280x720 unrevealed,16.780ms at
5120x2880/minimum zoom; frontier0.562/8.759ms. Immutable row spans now reduce those raster costs to0.146/2.487ms unrevealed
and0.153/1.444ms frontier. All13 focused native/scene/GPU checks passed after the
change. Original native pixel payloads are unchanged; the shared-fixture metadata
was regenerated and independently checked after the stock type extension.
Reports, compressed logs and hashes are preserved under
`tools/procedural_drawing_oracle/validation/`. Release capture last60-frame wall
means were8.75/8.77ms clear and8.55/8.42ms shrouded (rally/Stop), including the
observer/simulation/pacing; not isolated GPU time or ordinary play FPS.

## Current workers and next safe action

- All research agents remain read-only. Release build v3 passed, SHA256
  `d5b05d06e77cb108ee1bee37d3a14852e4bc23cc51a5324f953faa7f992fd8d5`.
  Its fifth1500-step clear capture matches every frame byte and all recorded
  state boundaries from v2; portable native comparison passes all5 captures.
  The action-line order now follows both rally passes (native6D4750). Clippy
  passed after replacing redundant cell-query temporaries and grouping the
  private shroud constructor dimensions. Two cfg(test) constructor callers
  needed the same argument migration; all122 focused rerun checks passed.
- Preserved stash b4474c90468940ace26bfe3a84fefb682b767abe holds pre-integration
  work. All58 nonoverlapping owned paths were byte-identical after integration;
  the single Jumpjet conflict retains incoming main and canonical cell query.
- The first v1 capture failed at1351 because Select made its center-map cursor
  animated. Evidence remains in `logs/procedural-drawing/rally-clear-capture`.
  Optional sealed cursor[720,556] fixed the harness without relaxing static
  cursor or idle-camera checks. All95 focused Python checks passed.
- Clear/Stop and shrouded/Stop v2 pairs are VALID. Stock GAPILE1477 is3x2,
  source[8064,21888,416], target35,89 level4/slope0/no bridge. Camera[-2100,1426],
  zoom1, frame1500. Native crop origin[415,296] contains106 original stores;
  all76 changed pixels and62 declared unobstructed floor stores match RGB565,
  with no changes elsewhere among480,000 frame pixels. All1501 actor/terrain
  observations differ only by ArchiveTarget after Stop. Covered30 stores do
  not certify native object occlusion; shroud history is not certified.
- Portable originals and comparison receipt are under
  `tools/procedural_drawing_oracle/production-validation/`; recheck with that
  package's `rally_production check`. No game executable is copied into Git.
- Full retail lib:9648 passed,1 stale source-order assertion failed,238 ignored.
  The obsolete fullscreen-shroud anchor is corrected with the action ordering;
  44 focused rendering checks,122 cleanup checks and Clippy passed. The critic then found quadratic selection lookup/front insertion in the
  shared input owner. Membership indexes/deque insertion preserve native order
  and pending authority. Full retail9651, Clippy and Python580 passed after the
  fix. The20k-mobile acquisition+rally mean fell16.58→2.10ms; missing-ledger
  recovery34.51→4.38ms. Raw20-sample reports and the critic disposition are in
  the validation directory. Final release v4 `c3aaa9ba6e930ca359cbddc1f2b7a7569e148f6b28365d6947fa95a830b72404`
  produced `rally-clear-v4`: all480,000 frame pixels and1501 state boundaries
  match v2, and all native mask/floor checks pass. Runtime Rust is frozen.
  Next: commit, one PR and integration. No second critic. Then action lines.
- Cargo uses `/Users/halvor/Documents/vera20k-dev/.venv/bin/python -m tools.cargo_run`.
  An earlier admission was blocked below16GiB; later admission succeeded without
  manual deletion. Freeze all source/metadata during Cargo. Preserve ignored
  logs, native inputs, owned labels and shared build ownership.

## Limits that remain explicit

Minor visual residuals: native chop53 versus host-nearest clipping can shift one
edge pixel; fractional VERA camera/zoom can expose one neutral far-edge A row/column.
FrameFFFFFFFF FNPC accesses before the selected candidate pool; comparisons cover
nonnegative signed frames, with a documented deterministic Rust extension afterward.
Enabled fog/dynamic AlphaShapes, EMP input state, ConstructionYard repack and planning
are broader required mechanisms; this ordinary stock-factory chain does not close them.

Research ahead only: action lines run after both rally passes, forward TechnoArray
at6D4750. Unit6F3D60 uses a body TurretOffset anchor; navigation uses target+48, not+4C.
Reuse CdTimer and the existing lead/coordinate owners. Inline probes are leads until
saved as reproducible goldens; no second implementation chain has started.

Read-only next-chain handoff (inline probes, not saved goldens yet): choose a
selected local MTNK fresh ordinary Move, TarCom null, NavCom Cell, empty queue.
4DC1AA uses raw Location;6F3D60 TurretOffset is attack-only. Reuse ground_pose
object_location/target_get_coords with isolated queries, TargetLineState plus
CdTimer.remaining(binary_frame)>0, and extend the private House50B6F0 read owner
in world_orders rather than duplicate admission. Original PaletteReader supplied
N53 middle26 PALETTE.PAL (SHAe85c535002573b5f95f7c361c4bdb95ee43e4914ddc926afda9ace995c54242b):
index3=0540, index8=A800. Existing Rally/PaletteReader arenas, actual Unit vtable,
original70D150 and4DC060 produced135 green pixels for Location[2688,5248,0] to
Cell14,20 at frames100/124 after restart100, none125/126. 7049C0 calls two filled
3x3 endpoint rectangles421B60 and solid7BA5E0→7BA610; no A/Z/RNG. Preserve an
executable corpus before implementing; queue/repeated destination preprocessing,
TechnoArray order, load-reanchored timer and broader input triggers remain unproven.

Latest read-only research is preserved under ignored logs: selected-unit-move-line-expanded
probe/handoff contains57 original cases (32 geometry/control,25 timer), all octants,
clipping/rejected lines and timer wrap/sentinel boundaries. Original control135
pixels matches; normal timers expire at elapsed25, startFFFFFFFF is the native
sentinel. It is prepared-input research, not production parity. Both native and
render research agents stay idle during measured workloads.
