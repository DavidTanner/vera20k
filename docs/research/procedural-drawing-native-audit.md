# Procedural gameplay drawing: native census and open chains

Read-only audit, 2026-10-03, against `origin/main` `c19e54458` before the
procedural-drawing work. This is an open scope inventory, **not** a parity or
completion certificate. No Cargo, native emulation, game capture, or performance
run was performed by this audit. Native arithmetic below has been read, not
executed. Current production validation remains a requirement for every chain.

Native image: retail `gamemd.exe`, base `0x00400000`, SHA-256
`1cdd1180e49024fbda8ad568caac2e86e856063ff67ab38f62b7d2c7bb84298c`.
The live Ghidra selector was `gamemd.exe` (reported project path
`/gamemd.exe`). Function names are navigation aids; body, caller and field-flow
checks below distinguish them from old annotations. Original bytes were also
read with `tools.native_inspect` for sensor-range selection and the tactical
psychic/capture/airstrike dispatch.

## Scope discovered from production roots

The gameplay roots are `TacticalClass_Draw 0x006D3D10`, its object-drawing
virtuals, and the gameplay radar draw. After the object stage, Tactical's direct
dispatch includes IonBlast, Spotlight, LaserDraw, EBolt, LineTrail, RadBeam,
radial indicators, beacons, band box, planning paths, factory rally lines,
placement, psychic action lines, capture links, Boris designation and pixel FX.
The two repeated planning/rally calls surround separate rendering stages and
have distinct draw arguments; treating them as an accidental duplicate is unsafe.

Procedural geometry, point effects, generated masks and framebuffer distortion
belong in the census. Asset-only SHP/VXL sprites are dependency boundaries unless
their producer is required by a procedural chain. A native-looking sprite or
readable geometry helper does not establish the submitted result. In particular,
Lightning Storm's authored bolt SHPs should not be mistaken for EBolt's recursive
Tesla geometry merely because both have bolt names.

| Family / native owner | Live caller or gate established by reading | Rust owner and unresolved required behavior |
| --- | --- | --- |
| Factory rally: `0x006DA9D0` | Directly called twice by `0x006D3D10`; selected, active local building with rally-capable virtual and ArchiveTarget. See exact gate below. | `app/presentation/target_lines.rs`, `render/build_instances.rs`. Existing pixel generation ignores the computed dash phase; first chain owned by the main task. |
| Selected mobile orders: Foot `0x004DC060` -> ActionLines `0x007049C0` | Tactical local/human-house selected branch calls virtual `+0x438`; global UnitActionLines gate. Techno default `0x00459E60` is empty. | `target_lines.rs`. Needs native source FLH/led-target vs navigation endpoint, timer, bridge projection, clipped pixels and production comparison. Factory rally has a different dispatcher and leaf. |
| Psychic detector enemy orders: `0x0043B150` -> `0x004DC340` | Tactical non-human branch, non-allied victim, online psychic building whose radius covers target or navigation destination, Foot bit and not limbo. | No production presentation owner found. The name `DrawRadarActionLines` is misleading: the body draws on the tactical composite. House color, endpoint squares and time-based dash/color phase are separate from selected-order lines. |
| Building selection brackets: Techno `0x006F60D0`, `0x006F5190`, corner `0x006F5EF0`, Tactical line `0x006DBB60` | Object DrawBehind/DrawExtras virtual passes; selected building branch. | `selection_brackets.rs` explicitly suppresses native edges at lines 458-475 because its occlusion approach did not work. Retain all native passes with the shared A/Z authority; this is a required unresolved chain. |
| Health/status geometry: `0x006F64A0`, `0x00709A90`, DrawExtras | Selection/hover admission and owner/alliance/type-specific paths. | `ui_overlays.rs`, `render/selection_overlay.rs`. Most pips are asset blits; generated backgrounds/fills and their admission still need procedural coverage. Existing residuals name missing control-group/spawn/self-heal status consumers. |
| Drag selection rectangle: `0x006DA180` | Tactical band-box start is nonzero; canonicalized endpoints include `+1` width/height; THEATER.PAL index 15, surface `+0x54`. | `SelectionOverlay::build_drag_rect` and input selection state. No native executable pixel comparison located by this audit. |
| Selected radial indicators: Building `0x00456750` -> shared `0x00456980` | Tactical current-object loop requires type `+0x238` then virtual `+0x130`; Building checks range > 0, online `+0x660`, human owner. Concentric mode reads wall time. | `ui_overlays::build_building_radius_ring_instances` uses a hard-coded green segmented ellipse and a different range resolver; no native radial indicator keys found in RuleSet. See required differences below. |
| Placement/SW radial previews: `0x006DBE20` -> `0x00456980` | Held building or armed SW, target cursor state; SW type `+0xF8/+0xFC`; building branch can redraw nearest same-type existing buildings within overlap range. | Existing ring function only visits selected structures. Preview pulsation, SW rings and neighboring-building admission are not supplied there. Same radial owner must serve these consumers. |
| Planning paths: `0x006DAD60` | Non-map-editor branch walks 12 House planning paths, waypoint order, selected-path style and shroud-side draw argument. | PlanningMode is no-op in `app/input/dispatch.rs`; current Shift queue is explicitly VERA-specific. Native path state/input/cleanup is a large required prerequisite, not a line-style-only fix. |
| Mind-control links: CaptureManager `0x00472160` -> Object `0x00704E40` | Tactical visits controller and controlled-object routes; `0x00472640` admits selected controller/target or node visibility timer. | `sim/capture_manager.rs` explicitly omits `MindControlAttackLineFrames` node timer and link drawing. Target height, controller FLH selector, 32-part animated color curve, lifecycle and detach must follow the existing capture owner. |
| Ordinary/prism lasers: LaserDraw ctor `0x0054FE60`, draw `0x00550260`, special `0x005509F0` | Ctor direct callers: SpawnLaser `0x006FD210`, support beam `0x0044ABD0`, DiskLaser AI, Railgun AI. SpawnLaser callers include FireAt and Bullet shrapnel `0x0046A310`. | Weapon keys exist, but no LaserDraw runtime/drawer found. `building_missions.rs` records invisible support and main beams. Shared laser owner must preserve all callers' colors, width, lifetime, blend/A/Z semantics. |
| Tesla/EBolt: constructor `0x004C1E10`, init `0x004C2A60`, recursive draw `0x004C1F20`, manager `0x004C2830` | FireAt -> SpawnElectricBolt `0x006FD570` -> CreateElectricBolt `0x006FD460`; shrapnel also calls CreateElectricBolt. Tactical manager updates source from owner FLH, draws, shifts lifetime mask, and clears owner `+0x6DC` on expiry. | `damage_consequences.rs` creates target spark systems, but no EBolt geometry/lifetime owner found. Particle creation is not bolt rendering. Include alternate color/DrawBoltAsLaser and source detach. |
| RadBeam and eruption: `0x006591B0`, straight `0x00659650`, sine `0x00659CA0` | FireAt -> SpawnRadBeam `0x006FD620` / SpawnRadEruption `0x006FD800`; draw manager uses frame rate and RNG and clears matching Techno `+0x510` links on deletion. | Parsed flags are not consumed by a RadBeam renderer. Must distinguish Desolator shot, eruption and temporal-related beam choices by original FireAt gates and data. Eight eruption endpoints and their RNG are native producer behavior. |
| Floating Disc ring/terminal shot: DiskLaser ctor `0x004A7A30`, AI `0x004A7340` | FireAt has its own DiskLaser branch; per-tick manager creates two moving LaserDraw ring segments, then terminal laser and damage/report. | `combat/world_receiver.rs` explicitly routes live discs through ordinary fire and records DiskLaser delivery as unported. Ring timing/damage/target tracking is a prerequisite mechanism, not an isolated visual effect. |
| Sonic/Magnetron wave distortion: Wave DrawIt `0x0075F9F0`; type 0 `0x0075FA90`, 3 `0x007602E0`, 1/2 `0x007609E0`; geometry `0x00761640` / `0x00762070` | FireAt constructs Wave `0x0075E950`; object virtual draw permits either endpoint visible when scenario fog flag is active. | `sim/wave.rs`, `render/wave_geometry.rs`, `fire_effects.rs`, `instances/overlays.rs`. Production emits white polygon edges; comments admit no framebuffer-distortion input and skip types 1/2. Geometry vectors do not certify visual output. |
| Boris airstrike designator: unnamed `0x00705860` | Tactical `0x006D47FB..0x006D48F1`: Techno `+0x294` manager, manager owner `+0x4C` is this object, target `+0x50` is Building; source FLH and wall-time/trig target point feed the draw. | No Airstrike manager producer; `aircraft/attack_mission.rs` records this absence. Native beam uses main RNG color, alpha/Z-clipped lines and quarter sections. Distinct from ordinary laser, temporal beam and Disc drain. |
| Spark/Railgun particle points: Particle DrawIt `0x0062CEC0` | Particle type `+0x314` values 3/4 enter point path, with detail/FPS/fog admission, A-buffer and Z test, color interpolation. | `instances/particles.rs` skips Spark and Railgun system kinds. `sim/particles/spark*.rs` contains simulation work; rendering cannot be certified by that. Railgun activity requires an all-layer retail census before exclusion. |
| Searchlight/particle light masks and beam edges: BuildingLight `0x00435BE0`; ParticleSystem `0x0062E280`; Spotlight `0x005FF850` | Building child needs live parent, operational gate and not deactivated; ParticleSystem light flags/count gate. Spotlight ctor also called from Spark AI and WarheadFlash `0x0048A620`. | `render/building_light.rs` has mask/blend helpers, but `build_instances.rs` intentionally submits an empty spotlight vector because evolving child coordinates/angle are absent. Recheck active retail/map reachability; never equate helper presence with production coverage. |
| Combat light masks: Spotlight `0x005FF850`, persistent manager `0x005FFFA0` | WarheadFlash and Spark light producers; option/detail/fog gates affect generic light path. | Existing `app/presentation/combat_lights.rs` + `render/combat_light.rs` own persistent light lifecycle and RGB565 compositor. Reuse them. Current whole-family native pixels/performance not revalidated by this audit. |
| IonBlast shockwave distortion: ctor `0x0053CB10`, manager `0x0053D850`, draw `0x0053D580` | Ctor callers include PsychicDominator `0x0053B080`, trigger IonBlastAtWP `0x006E3380`, TriggerAction. High-detail draw displaces existing framebuffer pixels under Z, rather than drawing a ring outline. | No IonBlast distortion owner found. Superweapon SHP effects are not this framebuffer operation. Trigger/action and retail Dominator producers need closure. |
| Water/ore pixel FX: `0x006D7840`, PixelFX init `0x00631D40`, update `0x00631E50`, timer `0x00631EE0` | Direct late Tactical call, with cells/visibility and species parameters. | `render/pixel_fx_sparkles.rs` deliberately uses stateless SplitMix hashing and a 2500ms bucket in place of native persistent per-cell RNG/timers. Existing tests establish that approximation, not native cadence/output. |
| Gameplay radar vector overlays: DrawViewportRect `0x00660540`, event `0x00660050` | Radar draw and event manager; event type chooses colors, generated surface geometry, phase and gradient leaf. | `native_radar_viewport.rs`, `radar_events.rs`, radar composition owners exist. Their coverage must be reconciled with native pixels; `radar_events_tests.rs` currently includes handmade outline assertions. Avoid assuming every event draw leaf is covered by native timing vectors. |
| Beacon label geometry: `0x00430250` | Allied/visible beacons via `0x00430AC0`; asset icon plus clipped black label background and owner-colored border. | Beacon command/input is unsupported. SHP itself is outside procedural scope, but a required gameplay label border/background cannot be dismissed because it accompanies a sprite. |

## Exact distinctions that prevent wrong shared ports

Factory rally `0x006DA9D0`: reverse current-object traversal; native instructions
at `0x006DAA65..0x006DAAB8` require `WhatAmI()==6`, byte `+0x90`, selected
`+0x83`, owner `+0x21C==PlayerPtr`, virtual `+0x284`, and nonnull ArchiveTarget
`+0x218`. Source is virtual `+0x48`. Target is its virtual `+0x48`, with ground
height and cell flag `0x100` bridge adjustment. The phase is
`(0x7fffffff-frame)%15`. Pattern `0x00842930` goes to surface `+0x4C`, three
lines: background-colored shadow at y+2 and house-colored strokes at y+1/y.
Root task owns original execution and final semantics; this reading is not a
pixel golden. The unrelated `ObjectClass__CreateRadialIndicator 0x005F5C20`
label is unreliable: its body is factory lookup delegation, not radial drawing.

Planning `0x006DAD60` uses pattern `0x00842940` and surface `+0x4C` for the
selected path, surface `+0x50` for other paths, plus MOUSE.SHA markers and
number labels. It must not silently inherit rally style or duration.

Building range source `0x004566B0` was checked against original instructions:
PsychicDetectionRadius first; then GapGenerator chooses normal/super radius
using the building's live `+0x268`; then SensorArray/CloakGenerator uses signed
`CloakRadiusInCells` at type `+0x1707`; finally valid current weapon range at
`weapon+0xB4`, truncated to cells. Current UI instead always prefers authored
super-gap radius, uses SensorsSight for sensor/cloak buildings, omits current
weapon range, and ignores the online/human/concentric/type radial gates. This
is a state/input mismatch as well as an ellipse-raster mismatch. Shared native
`0x00456980` calls surface `+0x20` twice and optionally draws spokes; it is not
the existing segmented polyline.

Airstrike identity does not rest on the unnamed draw body: constructor
`0x0041D380` stores its owner at `+0x4C` (`0x0041D3FF MOV [ESI+0x4C],ECX`);
Techno manager initialization is its direct caller, and the Tactical consumer
reads the matching owner/target. This corrects the tempting but unsupported
interpretation of `0x00705860` as a temporal or drain beam.

## Evidence and validation obligations

The existing strongest reusable drawing comparator located here is
`tools/projectile_oracle/line_trail.py` / `line_trail.json`, with Rust CPU and
GPU readback consumers in `render/line_trail*_tests.rs`. Its own report explicitly
excludes later RadBeam/other-effect overlap. Extend shared primitive owners only
after tracing every affected caller; native surface slots are not interchangeable.
Named tests were located, not run by this audit.

`g_MainRng` at `0x00886B88` is used by render-time laser/bolt/radiation and other
cosmetic paths. Preserve its relevant draw order without injecting it into
lockstep simulation RNG. Separate committed-logic laser aging (`0x00550150`,
called from `0x0055AFB0`) from draw-time EBolt/RadBeam mutation. Pointer expiry,
owner back-references, scenario teardown and save/load behavior need an explicit
decision for each state owner. `0x00534450` clears effect registries during world
teardown; dead owner IDs are not a substitute for native detach semantics.

Each chosen chain needs original binary execution for arithmetic/RNG/timers,
original RGB565 pixels for its renderer, final production screenshots/readbacks,
and measured CPU/GPU cost with representative visible counts. Include camera,
viewport and sidebar clipping, zoom policy, shroud/A-buffer, native packed Z,
bridges, same-frame overlap and lifecycle disappearance. Measure allocation and
submission growth as well as frame time; per-pixel SpriteInstances currently
occur in several owners, and correctness does not establish scale.

Fresh all-layer retail `ini-get` was unavailable without building an asset binary:
`cargo_run --resolve asset --profile release` reported no recorded executable in
this worktree or main. This audit did not build one. Therefore no family is
declared dormant/unreachable from guessed key absence. Before closing a family,
query production RULESMD/LANGRULE/mode/map layers and fixed ARTMD, then check the
native constructor/read/default/order/post-pass. Existing prose about stock keys
is a lead, not renewed retail evidence.

## Next chains and closure status

1. Factory rally, already selected by the owning task, including its shared
   patterned leaf, projection, two-stage composition and live producer state.
2. Selected mobile order lines and band box; these are frequent and reuse bounded
   clipping/pixel operations without depending on a new combat effect registry.
3. Complete selected building brackets; fix shared depth/A-buffer consumption
   rather than restoring visually wrong edges without occlusion.
4. Radial selected/placement/SW consumers under one range/raster owner.
5. Prism laser chain, then Tesla bolt chain, each including original emitters,
   ownership, lifecycle, pixel compositor and production validation.
6. RadBeam/eruption, mind-control link, Disc, wave distortion, Spark, and the
   remaining gameplay families above in visibility/frequency order.

This is a broad native-root and Rust-owner census, not a proof of exhaustive
primitive call coverage. Remaining audit work includes original vtable receiver
resolution for every gameplay procedural primitive, active retail/map reachability
for inherited families, complete special-weapon/trigger consumers, and the
production capture/performance matrix. No unresolved row can be closed merely
by retaining this document. Large prerequisites (planning, Airstrike, DiskLaser)
remain required work for the user goal.
