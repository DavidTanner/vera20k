# Procedural drawing: Rust and production audit

Read-only source audit, 2026-10-03, baseline `c19e544581aff37e9eff5a6a9976ccd78983b6e0`.
Line numbers below refer to that baseline. No Cargo, native execution, gameplay,
GPU test, or performance measurement was run by this audit. Existing test names
and evidence are routes to reuse, not fresh passing results. Native statements in
old comments/reports require the mechanism owner's current executable check.

The first implementation chain is **selected factory rally drawing**. Selected
unit order/action lines, including their FLH/lead endpoint prerequisites, are a
later chain. This inventory does not close the whole procedural drawing scope.

## Factory rally chain: owners and production boundaries

- `app/presentation/target_lines.rs:159-198` reads selected local structures,
  `rally_cell()`, `ObjectType::has_rally_line()`, current position, bridge/terrain
  height and the house color ramp. Its `emit_rally_line:378-387` discards the
  calculated phase and emits one solid line. There is no RNG in this Rust path.
  Timer/phase semantics need the current native draw witness, not its comment.
- The selected-object vector is **not** LogicClass's live-object vector.
  `app/input/state.rs:50-54` owns `selection_order` and its pending bit;
  `app/input/dispatch.rs:1945` exposes `selected_stable_ids_in_order`.
  `apply_selection_mutation:2045-2118` updates that order immediately;
  `reconcile_selection_order_after_sim:1974` handles committed selection,
  lifecycle departures and transfers (`match_runtime/sim_tick.rs:994`).
  Positive-damage Primary actors prepend; other actors append (`dispatch:2138`).
  The retained native evidence identifies `A8ECBC/A8ECC8` as current selected
  techno entries (`skirmish-ui/2026-09-12-keyboard-command-evidence.md:15-25`).
  `sim/world/mod.rs:4227` instead returns `substrate.logic.snapshot()`.
- `render/build_instances.rs:597-639` builds action and rally separately, then
  clones the rally vector for its two draws. `render/mod.rs:368-375` uploads both
  copies. One immutable uploaded buffer can serve the two native call positions;
  retain the ordering relative to action lines rather than collapsing draws.
- `render/draw_passes.rs:411-454` currently sends rally/action/radius pixels
  through `draw_pooled_no_depth`, whose name is misleading: it calls the
  LessEqual, depth-write-enabled pipeline (`render/batch.rs:799-833`). This can
  affect the subsequent depth-tested building bracket redraw.
- Existing `draw_pooled_passthrough_texture` (`draw_passes:958-967`) calls
  `BatchRenderer::draw_with_buffer_passthrough` (`batch:1843-1857`). It binds the
  world camera, `vs_main`/`fs_main`, alpha blending, Depth32Float **Always**, and
  **depth writes disabled** (`batch:845-880`). Its opaque pixels overwrite color
  without changing Z. `draw_with_buffer_ui_passthrough:1886` uses the same
  pipeline with the fixed UI camera. Choose the native surface contract.
- Every baseline solid-line pixel is a full `SpriteInstance`, and emission has
  no viewport input (`target_lines:321-365`). The raster walks the complete
  world-space segment even when most/all pixels are offscreen. GPU scissor is
  already tactical (`draw_passes:408`), but does not avoid CPU/vector/upload work.
  Clip before emission while preserving native pattern phase and raster ties.
- Generic sprite vertices expand each quad by 0.5 screen pixels on each side
  at non-1x zoom (`render/batch_shader.wgsl:63-72`). A procedural 1-pixel primitive
  inherits that expansion. Establish native 1x pixels, then test fractional zoom
  separately; do not infer exact line coverage from CPU points alone.

## Remaining scope inventory

| Family | Current owner and active consumer | Required work / evidence boundary |
| --- | --- | --- |
| Selected unit action/order lines | `target_lines.rs:112-155,200-269`; `context_order.rs:322`; selection timer in `input/dispatch.rs:2613` | Later chain. Endpoint source uses rendered object position; selected attack target wins, otherwise live NavCom/NavQueue. A float DDA uses rounded pixels. Selection/command timer is 25 sim ticks. Empty band drag misses the native timer restart, explicitly recorded at `dispatch:493-502`. Audit FLH, target lead, admission, source/target deletion, options and native timer/raster before replacing it. |
| Building selection brackets | `selection_brackets.rs:334`; three buckets in `render/build_instances.rs:67-69`, drawn before bodies and after shroud | `selection_brackets:458-476` deliberately emits 9 of native-commented 21 stubs because ground-mark occlusion is unresolved. Integer line walker, interpolated height/depth and CPU ABuffer tint are separate from target lines. Port the established removal/depth mechanism before restoring omitted stubs. |
| Sensor/gap/range rings | `ui_overlays.rs:1183-1329` -> `building_radius_rings` pass | Selected structures only; radius picked from psychic/gap/sensors keys. Fixed green ellipse is approximated by 96..384 trigonometric segments and rounded DDA pixels. Native admission, center/radius/color/raster, powered/super-gap transitions and other range families remain unproved. |
| Drag rectangle and placement indicators | `render/selection_overlay.rs:368-434,661-807,1420-1469` | Drag rectangle is four quads; placement combines retail assets and generated diamond fallback. Include raster endpoints, tactical clipping, input capture and zoom in the later selection chain. Health/veterancy/cargo pips mostly use retail SHPs (`ui_overlays.rs`) rather than procedural geometry. |
| Ordinary/support lasers | `rules/weapon_type.rs:92-118,183-204`; `sim/world/techno_ai/building_missions.rs:85-93` | IsLaser/colors/duration are parsed but have no laser drawing owner. Prism support and main shot beams are explicitly invisible. Support beam lifecycle, FLH/target snapshots, House LaserColor, thickness and PrismSupportDuration must reach a presentation owner. |
| Tesla/EBolt | `sim/world/damage_consequences.rs:320-393`; `sim/particles/spawn.rs:106-150` | Fire events create spark systems; bolt rendering is explicitly absent (`damage_consequences:351`). Native-commented Init cosmetic RNG is omitted. Endpoint retrieval currently has a named unchecked Cell-height fallback. Elite shrapnel producers are an explicit missing path. |
| Radiation beam / DiskLaser / temporal links | Parsed flags in `weapon_type.rs`; temporal gameplay in `sim/temporal.rs`; Disk arm in `sim/combat/world_receiver.rs:3533-3539,4015-4017` | No RadBeam or DiskLaser drawing consumer found. Live DiskLaser delivery itself is an explicit unported prerequisite; gameplay presently uses the ordinary path. Temporal gameplay does not establish a drawn erase beam. Trace each active-retail producer before defining its presentation owner. |
| Sonic/Magnetron/WaveClass | `sim/wave.rs` -> `presentation/fire_effects.rs:43-99` -> `instances/overlays.rs:982-1007` -> `render/wave_geometry.rs` | Types 1/2 are dropped (`fire_effects:56-58`); types 0/3 become neutral white polygon outlines instead of destination framebuffer distortion (`fire_effects:93-96`). Render geometry is recalculated with float trig separately from sim type-0 geometry. Existing embedded “executable” endpoint/vertex tests do not establish framebuffer pixels or production traversal. |
| Spark/Railgun particle drawing | `sim/particles/*`; `presentation/instances/particles.rs:63-86` | Renderer explicitly skips both families. SHP Smoke/Gas/Fire remain the only drawn particles. Spark systems can now be live, so the old “spawn filter should have caught this” warning is stale. Required Spark colors/lines and Railgun spiral/laser must consume actual simulation state, with native timing and RNG evidence. |
| Projectile LineTrail | `presentation/line_trails.rs` + `render/line_trail.rs` + `render/terrain_line_trail.rs` | Existing strong reusable owner: launch/detach lifecycle events (`sim_tick:1011-1024`), render-composite history, load clear, clipped integer raster, ordered RGB565 destination edits, no Z write. `tools/projectile_oracle/line_trail.{py,json,meta.json,md}` and CPU/GPU comparisons exist. Audit corpus limits and all active producers before calling whole family closed. |
| Water/ore PixelFX | `render/pixel_fx_sparkles.rs` -> tactical tail `draw_passes.rs:632-656` | Explicit stateless SplitMix/cycle-bucket approximation (`pixel_fx:6-10,74-79,317-345`) replaces retained native timers/RNG. Existing native-table comments and gate tests are not native full-effect parity. |
| Searchlights / generated masks | `render/building_light.rs`; `render/build_instances.rs:297-301` | Production type-16 list is always empty because authoritative child position/angle is missing. Beam edge builder is test-only (`building_light:256`). Native mask/CPU blitter tests exist, but production state and destination RGB565 path remain unresolved (`batch:1860-1863`). |
| Combat flashes/lights, radiation light | `presentation/combat_lights.rs`, `render/combat_light.rs`, `sim/radiation_light.rs` | Existing presentation runtime, generated mask bank, reverse insertion order and RGB565 renderer are reusable. Audit each native admission/update/expiry producer and GPU/capture coverage rather than replacing owners. |
| Radar event/viewport outlines | `render/radar_events.rs:436-495,534`; `native_radar_viewport.rs:194-320`; minimap transaction | Existing procedural drawing owners. Event raster is its own Bresenham implementation; viewport uses axis-aligned rectangles. Preserve their coordinate/timer contracts; reconcile shared native leaves only after address/behavior evidence. |
| Other active procedural families | Planning waypoint/relationship lines, AlphaShape, sprite material effects | No Planning Mode draw owner or AlphaShape producer/renderer found in this scan. These are native-reachability audit candidates, not proven dormant. `render/draw_state.rs:233-235` leaves invulnerability tint disconnected from missing authoritative mode; keep material effects in the whole-scope census where their native procedural behavior is required. |

Do not merge all line walkers merely because they draw pixels. Target lines,
brackets, range DDA, wave edges, radar lines and LineTrail have distinct current
contracts; native identity determines which are duplicates. In particular,
LineTrail is a depth/ABuffer/destination-edit mechanism, not an interchangeable
colored line.

Cosmetic RNG needs explicit ownership before the beam chains. Existing comments
identify native `g_MainRng @ 0x00886B88` as shared by EBolt, LaserDraw, RadBeam and
audio (`sim/world/mod.rs:744-749`, `damage_consequences:334-339`). They do not prove
that the current separate Rust audio/simulation streams reproduce every draw.
Keep rendering from perturbing deterministic simulation, and preserve actual
draw count/order/cadence in a reproducible native fixture.

## Actual production and performance validation

1. Reuse `python -m tools.map_observation` (`tools/map_observation.md:21-60`).
   It runs the normal production loader, typed commands and exact steps, then
   retains actual final GPU BGRA bytes, input/binary/map identities and adapter
   information. Set `graphics.upscale=false`; use native 1x first. Its strict
   profile/controller currently admits no `Select` command or UI event schedule
   (`map_observation.rs:154-173`, Python `map_observation.py:48-52`). A profile
   that only sets rally state is not evidence for selected-factory drawing.
2. Smallest rally state/render witness: admit existing `Command::Select` into
   that diagnostic's allowlists, through existing `try_schedule_command`
   (`map_observation.rs:680-715`). The existing sim receiver owns selection
   admission (`world_commands.rs:575,2232-2288`). For multiple-factory order
   coverage, the input selection ledger must also reflect native insertion order;
   direct command injection alone does not exercise that input ledger update.
   Label this a scheduled-command witness, not a click-path comparison.
3. Full input witness: existing public `input::dispatch::tactical_mouse:170`
   with ordinary Left Pressed/Released uses the object picker, selection mutation,
   command schedule and action-line policy. Preserve neutral capture input after
   the gesture; the map controller checks idle camera and static cursor every
   draw (`map_observation.rs:984-1028`). Visible release play can cover this path
   separately. Shift+S captures the already-presented pre-cursor image through
   `render/screenshot.rs`, not a reconstructed render.
4. Compare native line leaves and the actual production GPU pipeline on all
   relevant directions, both endpoint orders, pattern phases, clipping edges,
   overlapping factories and the two native invocation positions. Check the Z
   attachment is unchanged for a non-Z surface leaf. Match packed native color
   conversion; never infer rendered output from CPU instance tests. Include
   selected/unselected/foreign-owner/ineligible factories, set/change/clear rally,
   destruction/sell/capture/load, hills and bridge endpoints as active dependencies.
5. Reuse `render/line_trail_gpu_tests.rs:225-287`'s timing method: two warm frames,
   twenty measured frames, CPU preparation then submission plus completion wait,
   reported separately with median/min/max. It is completed-submission wall
   time, not GPU-only or release FPS. Existing real GPU timestamp pattern is
   `terrain_draw_gpu_tests.rs:1153-1338` (feature-gated timestamp queries,
   resolve/map/wait and timestamp-period conversion). Use the production batch
   owner and identical visible/fully-clipped/overlapping workloads.
6. Ordinary frame cadence already has an owner: `diagnostics/dev_overlay.rs:83-124`
   keeps 60 boundary deltas; `app/frame.rs:46` samples it and `in_game.rs:508-509`
   displays mean ms/FPS. It includes simulation, idle/vsync and UI; it is not a
   render-stage profiler. `GameRenderTimes` (`render/mod.rs:73-80`) is only the
   supplied HUD clocks. Extend the existing render output/diagnostic receipt for
   needed rally counts/bytes/build/upload/encode measures instead of inventing a
   second capture runner. Avoid readback/screenshot cost in steady-state timings.
7. Baseline vectors scan all entities and emit offscreen line pixels; stress
   ordinary visible counts, dense crossing lines and 20,000 mostly-offscreen
   entities. Report selected/visible counts, emitted pixels/instances/bytes,
   adapter/backend/resolution/zoom, source and executable identity. The existing
   buffer pool reuses allocations and grows 2x (`batch.rs:385-449`); measure
   steady state separately from first allocation/atlas growth.

Required work remains open until these consumers have native evidence and actual
production/rendering validation; this audit records the boundaries only.
