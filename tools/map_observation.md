# Chosen-map production observation

`python -m tools.map_observation` launches the normal production loader through
`--tactical-capture map-observe-v1`, advances the requested exact simulation steps,
and retains a hidden-window GPU readback. This is a production observation, not a
native comparator or a gameplay/pixel parity certification.

The explicit profile schema belongs to
`src/app/diagnostics/tactical_capture/map_observation.rs`. Start from
[`map_observation.example.json`](map_observation.example.json), whose launch is the
existing Rust `SkirmishLaunchSession` DTO. Change `launch.selected_map_file` to the
chosen retail map. Keep explicit countries, colors and distinct start slots, and
keep `pre_fill_house_roster` consistent with the opponents. Rust validates launch
admission; Python does not maintain a second launch parser or synthesize defaults.

Build through the shared Cargo owner, then run from the checkout with an existing
`config.toml` pointing at retail assets. Set `graphics.upscale = false` for the
native-resolution capture. All supplied paths must be absolute without symlink
ancestors; use canonical temporary paths on macOS. The output parent must exist,
and the output directory itself must not exist.

```sh
python -m tools.cargo_run -- build -p vera20k --release --bin vera20k
env -u RA2_DIR python -m tools.map_observation \
  --profile /absolute/checkout/tools/map_observation.example.json \
  --contract /absolute/checkout/src/app/diagnostics/tactical_capture/contract.v2.json \
  --cwd /absolute/checkout \
  --output /absolute/evidence/new-map-observation
```

The default executable is the unchanged release `vera20k` recorded for this
checkout by `tools.cargo_run`; it never guesses a target directory. Recorded byte
identity does not establish source freshness: build the intended revision first.
`--executable /absolute/path/to/vera20k` selects an explicit binary instead.
For preserved builds, `--build-label LABEL` asks the shared Cargo owner for that
label's verified host release `vera20k`; the two selectors are mutually exclusive.
It checks the artifact path, target/profile classification, ambiguity and actual
executable SHA, with no fallback to the latest build. For example:

```sh
python -m tools.cargo_run --label before-map-change -- build -p vera20k --release --bin vera20k
python -m tools.cargo_run --resolve vera20k --profile release --from-label before-map-change
env -u RA2_DIR python -m tools.map_observation \
  --build-label before-map-change \
  --profile /absolute/checkout/tools/map_observation.example.json \
  --contract /absolute/checkout/src/app/diagnostics/tactical_capture/contract.v2.json \
  --cwd /absolute/checkout \
  --output /absolute/evidence/before-map-change
```

The supplied contract must match the repository contract bytes. Every environment
variable in its denylist must be absent, even if set to an empty string. The
wrapper reports denied variables and does not silently change the environment.
After sourcing the native development environment, explicitly remove `RA2_DIR`
for this command as above; asset loading uses the working directory's config.

A new wrapper v5 bundle contains sealed `profile.json`, `config.toml` and
`contract.json` copies, plus `stdout.log`, `stderr.log`, `run.json` and the child's
atomically published `child-output/{capture.json,frame.bgra}`. Runtime still reads
the supplied original paths; retaining copies does not redirect the game loader.
Original files and retained copies must remain unchanged during capture.
`run.json` records exact input hashes, command, child PID/status, timeout, receipt
validation and capture artifact identities. A valid observation requires unchanged
profile/config/executable/contract files, matching profile and contract receipts,
a v5 child manifest with resident UnitAtlas statistics, a checked presentation-clock
transcript and neutral-input evidence, zero initial tick/frame/time,
the requested final tick/frame and endpoint step
receipts, a loaded loose/MIX map digest, hidden unfocused rendering without input
violations, and correctly sized/hashed BGRA bytes. Simulation time must advance
for nonzero steps; its scheduling formula remains owned by Rust. Zero steps must
retain the initial fingerprint. Map hashes attest the bytes reported consumed by
the loader; this wrapper does not independently extract MIX entries or reimplement
the loader. Compare deterministic fingerprints separately from presentation pixels.

The v4 `render.presentation_clock` records actual consumed presentation times:

```json
{
  "policy": "map-exact-step-presentation-v1",
  "origin_ms": 0,
  "interval_ms": 22,
  "draws": [
    {"completed_steps": 1, "radar_ms": 22, "tooltip_ms": 22, "message_ms": 22}
  ]
}
```

For a positive budget N, exactly N game draws occur at completed steps 1 through
N. A zero-step observation has one draw at step 0, with all three times zero.
Every row must contain the exact radar tick, tooltip poll and adjusted message
management times consumed by that draw; each must equal its completed step × 22.
The budget is bounded at 100000 steps. Unknown fields/policies, missing or extra
rows, reordered/repeated steps, wrong integer types and any time discrepancy fail
validation. The wrapper retains the validated transcript as
`capture.presentation_clock`; it does not reconstruct a missing transcript.

The required v4 `render.neutral_input` has exactly
`{"static_default_cursor":true,"camera_input_idle":true}`. Rust verifies these
prerequisites and render readiness on every draw before publishing the final
receipt; the wrapper checks their declared types/values and retains them as
`capture.neutral_input`. This bounds the capture to neutral input and a static
default cursor. It does not establish arbitrary animated-cursor reproducibility.

This supplied clock is a diagnostic reproducibility policy, not native wall-time
cadence. Ordinary play and other capture profiles retain their existing wall
clocks. The clock does not change simulation scheduling or provide evidence for
native pixels, audio playback, menus or outcome timing. Full-frame comparison
remains exact: no radar masks, channel tolerances or skipped pixels are applied.

## War Miner Attack return observation

[`map_observation.war-miner-attack.example.json`](map_observation.war-miner-attack.example.json)
uses stock Russia/Battle/AnyTown, ordinary MCV deployment and building production,
and the refinery's free War Miner. At step2500 it ForceAttacks a stationary
friendly conscript. Actor and terrain samples show Attack, combat target removal,
the idle return and later ore consumption. The conscript belongs to the local
house and remains stationary. The command bypasses UI click resolution,
using the existing synchronized command scheduler.

Numeric handles are tied to that launch and should be rechecked after population
changes. This observes production behavior; it does not certify native combat,
scheduler, RNG or rendering parity. See the [mission ownership notes](../src/sim/miner/README.md)
and [original executable packet](spatial_oracle/harvest_attack_return.md).

## Unloading miner and parked aircraft bodies

[`map_observation.war-miner-unload.example.json`](map_observation.war-miner-unload.example.json)
is the War Miner launch above without the attack, stopped at step3775 with the
camera on the refinery pad. In this launch the free War Miner's Unload mission
dumps from step3762 until Harvest resumes at step3794, so step3775 draws its
body from `UnloadingClass=HORV`.
[`map_observation.docked-aircraft.example.json`](map_observation.docked-aircraft.example.json)
uses stock France, builds GAPOWR, GAREFN, GAAIRC and one ORCA by ordinary
production and stops at step5400 with the Harrier parked on the Air Force Command.

Both frames show whether a body is drawn from the sprites its model is seeded with
(`draws_turret_parts` in [`voxel_frame_catalog.rs`](../src/sim/voxel_frame_catalog.rs)).
Release captures of `6dab3753` drew neither body, only its health pips. With the
turret split following the drawn model both are drawn: the simulation fingerprints
are equal and the frames differ only inside x287..349/y246..298 (miner) and
x295..335/y264..293 (Harrier). The same holds in flight (x296..339/y65..101): add
`{"Move": {"entity_id": 1489, "target_rx": 44, "target_ry": 98, "queue": false}}`
at step5200 and stop at step5300 with the camera on (41,95). These observe
production output; no gamemd frame was compared. Numeric handles are tied to
each launch.

The [validation receipt](map_observation.unit-body-draw.validation.json) records
the source and executable identities, frame hashes, changed-pixel bounds and
test results. The step3745 frame immediately before dumping is byte-identical.
Original runs, raw frames, profiles, native disassembly and logs are retained
in the receipt's local evidence archive; executables remain with the shared
build owner. Siege Chopper deployment is tracked separately below; aircraft
shadows remain open.

## Siege Chopper deployment observation

[`map_observation.siege-chopper.example.json`](map_observation.siege-chopper.example.json)
uses a normal Russia/Battle launch and an authored map containing one local SCHP.
It keeps stock rules and assets. The ordinary command schedule moves actor 1 to
(50,50) at step 10, issues `DeployMcv` at steps 200 and 500, then moves it back to
(48,48) at step 650. The legacy command name also carries simple deployment and
undeployment. The run ends at step 750. An enqueue receipt establishes neither
admission nor completion; this route also bypasses self-click and deploy-key input.

Create the map and a profile copy from the checkout, using new absolute paths.
The existing fixture owner supplies the map encoding and clear Temperate terrain;
the edits below author map objects and Houses without changing rules or runtime
state. This is the portable recipe retained in the local evidence archive's
`captures/fixture-reproduction.md`.

```sh
python - /absolute/evidence/siege-chopper.map /absolute/evidence/siege-profile.json <<'PYGEN'
from pathlib import Path
import json, re, sys
from tools.render_depth_fixture import build_fixture, pack_lcw

map_path, profile_path = map(Path, sys.argv[1:])
assert all(path.is_absolute() and not path.exists() for path in (map_path, profile_path))
mission = build_fixture(walls_only=True)
mission = mission.replace('Name=Depth Continuation - Walls and Cliff',
                          'Name=Siege Chopper Deployment Observation')
mission = mission.replace('Americans', 'VERA-OBSERVER')
mission = mission.replace('Country=VERA-OBSERVER', 'Country=Russians')
sections = {
    'Structures': '',
    'Infantry': '',
    'Units': '0=VERA-OBSERVER,SCHP,256,48,48,64,Guard,None,0,-1,0,-1,1,1',
    'OverlayPack': pack_lcw(bytes([255]) * 262144),
    'OverlayDataPack': pack_lcw(bytes(262144)),
}
for name, body in sections.items():
    mission, count = re.subn(
        r'(?ms)^\[' + re.escape(name) + r'\]\n.*?(?=^\[|\Z)',
        lambda match: f'[{name}]\n{body}\n\n', mission)
    assert count == 1, (name, count)
map_path.write_text(mission, encoding='ascii')
profile = json.loads(Path('tools/map_observation.siege-chopper.example.json').read_text())
profile['launch']['selected_map_file'] = str(map_path)
profile_path.write_text(json.dumps(profile, indent=2) + '\n')
PYGEN
```

The recorded map SHA-256 is
`40972558716058e1f0f5b1481824457b5643bc5425e3fa3a09689abe7f1d5cf9`.
Recheck L0 actor IDs after changing any launch or fixture input. The recorded
fixture has SCHP 1 at (48,48) and an opposing MCV far away; it does not induce
combat. Run the copied profile with the build, environment and path requirements
at the start of this document. Retain a full cycle first, then choose phase
captures from its observed trajectory. For a shorter capture, set `ticks` to the
chosen completed step and keep only commands whose `issue_after_step < ticks`.
Each run retains the final GPU frame; earlier trajectory rows alone are not images.

New actor rows include optional `unit` observations: raw deployment bytes
`deployed_6e0`, `deploying_6e1`, `undeploying_6e2`, the landing request
`landing_for_deploy_134`, owner `stage_f8`, persistent `body_counter_538`, and
`deploy_anim_130`. The last is null or contains the retained stable ID and a
nullable `live` body with type, frame and owner identity. A retained ID with no
live Anim stays explicit. Non-Unit rows use null. The wrapper checks types and
ranges, accepts older rows without this extension and preserves them unchanged;
it does not turn these values into gameplay or parity assertions.

The baseline ignores the two deploy inputs and still draws the flying SCHP at
step 400. The candidate release loads the fixture through the production app
and draws each phase below. All eight wrapper receipts are valid; each phase
trajectory matches the corresponding prefix of the full cycle.

| Phase to inspect | Actual completed step and retained GPU frame |
| --- | --- |
| Airborne SCHP before deployment | 190; SCHP visible at Z=500 |
| Landing requested by deployment | 230; SCHP visible at Z=230, landing request set |
| Forward SCHPDEPL with the unit body hidden | 300; attached animation frame 5 visible |
| Deployed SCHD body | 400; deployed flag set, animation reference cleared |
| Reverse SCHPDEPL with the unit body hidden | 550; attached reverse animation frame 5 visible |
| SCHP returned to flight | 640; SCHP visible at Z=500 before the later Move order |

The [validation receipt](map_observation.siege-chopper.validation.json) records
source/build/map/profile identities, every retained frame hash and inspected
image, and the exact transition steps. Landing starts at 202, touchdown clears
the request at 253, forward animation starts at 254, and deployed state begins
at 345. Reverse animation starts at 502, undeployment assigns its nearby
destination at 584, and automatic flight restores Z=500 at 635. The final Move
reaches (48,48) by 750. A repeated full cycle matches all 751 observation rows,
initial/final fingerprints and final BGRA bytes exactly.

The first candidate exposed a missing atlas refresh: its simulation advanced
the transition but the selected palette frames were absent, making SCHPDEPL
invisible. The final build uses the existing atlas owner to service live Anim
palette demands on committed ticks. Against that failed rendering candidate,
all eight runs retain identical simulation observations and fingerprints; only
the forward/reverse frames change, each by 1,561 pixels inside the chopper's
bounds. Both candidates and the baseline remain in the local evidence archive.
These are production observations; no gamemd raster comparison is claimed.

The [native evidence](spatial_oracle/unit_simple_deploy.md) covers the original
transition and Stage bodies, Jumpjet handler branches, deployment admission,
body selection and selected-HVA frame arithmetic. Its
[fixture limits](spatial_oracle/unit_simple_deploy.meta.json) describe supplied
callbacks and the component timeline; they do not establish a full native flight
or world scheduler. Aircraft shadows, type-specific custom palettes,
viewer-dependent disguise palette selection, the forced Magnetron source and
EMP admission remain outside this stock observation. Full native animation
lifetime, audio playback and raster parity are also unclaimed.

## Natural ore-spread observation

[`map_observation.ore-spread.example.json`](map_observation.ore-spread.example.json)
loads stock AnyTown in Battle mode and advances one ordinary simulation frame.
Its 213 terrain observations are initially empty cells next to raw-map ore,
selected from the same `XMP03T4.MAP` payload recorded by the production loader
(SHA256 `7a390de363f79743dd54897a49302869a795f839f3387ff03e8c0b70a519e17e`).
Coordinates were read with the existing shrapnel-repair map-facts decoder; the
profile does not create ore, alter queues or provide gameplay results. The final
integrated release run witnessed 16 new ore cells at density 3. This is a
production integration observation, not a gamemd gameplay/pixel comparison.
Original executable queue/RNG/Mark comparisons and the final run identities are
in [`spatial_oracle/ore_queue.md`](spatial_oracle/ore_queue.md) and its
[`validation receipt`](spatial_oracle/ore_queue.validation.json).

## Ordinary scheduled commands and actor trajectories

[`map_observation.engineer-repair.example.json`](map_observation.engineer-repair.example.json)
uses ordinary Battle/AnyTown orders to deploy the MCV, construct GAPOWR/GAPILE,
deliver an ENGINEER, damage the power plant by MTNK ForceAttack, stop the tank,
enable paid repair and send the Engineer through CaptureBuilding. The
[production receipt](spatial_oracle/engineer_repair.production.json) records the
1650-step release capture and exact repeat: all fingerprints, complete observed
trajectories and full Metal BGRA bytes MATCH. At frame1602, actual health changes
304→750, damaged slot1581 is replaced by healthy slot1742 and Engineer1557 is
absent after deferred cleanup. Separate frame1601 and1650 GPU images were inspected.
These are observed production timings/IDs, not native goldens. The profile's
numeric handles/IDs belong to this exact stock launch; discover them again when
changing launch inputs. Native fixtures and the physical retail input test cover
the health sample, House consumers and sound request that the map observer does
not expose. Native whole-clock, pixel and audible output remain outside this run.

For TIBTRE observation, use
[`map_observation.tibtre.example.json`](map_observation.tibtre.example.json).
It loads retail AnyTown (`XMP03T4.MAP`), advances 800 simulation steps and looks
at the two TIBTRE02 cells `(74,32)` and `(76,27)` plus their eight neighbors.
Requested terrain rows also retain `overlay: {id, density}` and
`terrain_object: {name, frame, active}` when those owners exist. A nonanimated
object has null frame/active. These are immutable reads of the live owners;
they do not inject ore or advance an animation. The validator accepts old rows
without these fields and requires both fields together on new rows. Native
cadence and admission comparisons are recorded in
[`spatial_oracle/tibtre.md`](spatial_oracle/tibtre.md).

Profile v1 remains accepted and retains its original JSON projection. It cannot
declare the following extension fields, even as empty arrays. Profile v2 adds
optional `commands`, `observe_owners`, `camera_cell` and `terrain_cells`; omitted
fields stay omitted in the sealed request, and explicit null is rejected.
The existing v1 example is unchanged. Start retail bridge discovery with
[`map_observation.bridge-response.example.json`](map_observation.bridge-response.example.json):
an accepted Battle launch on stock AnyTown (`XMP03T4.MAP`), with ordinary starting
forces, an allied Computer1 and hostile Computer2. It requests zero steps, observes
both AI Houses and centers the camera on the concrete low bridge near (87,53).
The physical span occupies x86..88/y51..57 at level4 with LOBRDB overlays;
it has no structural elevated deck stamp. Verify those facts in the L0 terrain
receipt before choosing orders. A high-deck response needs a separate map site.
Its disabled shroud is an explicit launch option for this diagnostic observation.
No actor IDs or gameplay state are granted by the profile.

First retain L0 to discover the actual generated AMCV, MTNK and E1 stable IDs and
positions. A short ordinary deployment probe then discovers the actual GACNST
created by the AI MCV. Seal the final profile with those IDs and observed timings:

```json
{
  "commands": [
    {"issue_after_step": 0, "owner": "Computer1",
     "payload": {"DeployMcv": {"entity_id": 120}}},
    {"issue_after_step": 40, "owner": "Computer1",
     "payload": {"Move": {"entity_id": 121, "target_rx": 87, "target_ry": 53, "queue": false}}},
    {"issue_after_step": 200, "owner": "Computer2",
     "payload": {"Attack": {"attacker_id": 130, "target_id": 140}}}
  ],
  "observe_owners": ["Computer1", "Computer2"],
  "camera_cell": [87, 53],
  "terrain_cells": [[87, 53]]
}
```

These IDs and timings illustrate syntax; obtain actual values from the production
probes. Command payloads are the existing Rust serde `Command`, with no separate
order translator. Supported orders are Move, Stop, Attack, ForceAttack, Guard,
DeployMcv, ForceAttackCell, QueueProduction, PlaceReadyBuilding and
CaptureBuilding (the resolved Engineer repair/capture mission) and ToggleRepair.
Rust rejects
ignored payload fields or argument
types. Python checks diagnostic structure and the exact typed request/receipt;
it does not duplicate the gameplay command parser or admissions.

Child/run v5 records `observations.rule_types` at L0. These are the actual
rules-owned Infantry, Unit, Aircraft and Structure list names, their categories,
and their existing `interned_id` numeric handles from the loaded simulation.
The observer never interns a name. Obtain the intended type's handle from that
same launch before sealing a production profile; handles are local to that
world's interner and must not be copied between different launches or fixtures.
The numeric `type_id` below illustrates serde syntax, not a stock handle:

```json
{
  "commands": [
    {"issue_after_step": 100, "owner": "Human1",
     "payload": {"QueueProduction": {"type_id": 41}}},
    {"issue_after_step": 500, "owner": "Human1",
     "payload": {"PlaceReadyBuilding": {"type_id": 41, "rx": 87, "ry": 53}}}
  ]
}
```

Use the actual loaded House name, discovered type handle, placement site and
observed ready timing. These orders use the ordinary production queue and
placement admission. Recording an enqueued command does not imply the House
could build it, had sufficient credits, had a ready object, or accepted the cell.

Rows must be nondecreasing by `issue_after_step`, retaining input order for ties,
and occur before the final step. An issue at step N calls the ordinary
`try_schedule_command` producer at simulation tick N, before advancing frame N+1.
The producer owns encoding and queuing; the diagnostic adds no input-delay offset.
Move carries an already resolved semantic destination, as synchronized/replay
orders do; it does not claim to reproduce the preceding cell-click resolver.
The receipt records enqueue success, not gameplay admission or completion.
Ordinary actor/House checks remain in the command drain; verify their actual effects
in the trajectory. Nonlocal-House envelopes are explicit diagnostic commands,
not authority for the local player's UI to control an AI opponent.

`observations.frames` contains L0 and every committed simulation frame. Each
requested House contributes all its represented entities, including inactive
objects. Rows retain stable IDs through capture and record disappeared IDs instead
of silently rebinding them. Actor rows contain physical lepton XYZ, cell, bridge
layer, health/lifecycle, raw Mission current/queued/suspended/effective/handler and
timer state, exact tagged target/ArchiveTarget/NavCom, Foot +688/+68D, Infantry
Doing and the existing virtual +4C coordinate owner's result. An unavailable
coordinate is explicit; observation does not invent one or initialize gameplay.
Foot fields are null for structures. Terrain rows read only allocated real cells,
report current tile/subtile, presentation tile, level/slope and bridge fields, and
report unallocated cells explicitly without stamping the shared Dummy.

V5 actor rows add `building`, null for Foot objects. Structures read their sole
body state, queued body request, construction control, complete serialized
StageClass, ready latch, ActuallyPlaced and the last sampled operational byte.
No diagnostic field advances a timer or calculates a replacement gameplay state.
`building.animation_slots` contains occupied slots in native slot order, with
their actual animation stable ID and the live animation's canonical type name,
interned type ID, native object ID, world coordinates, attachment, Logic membership
and complete retained runtime/timer. A stale occupied slot retains its ID with
`animation: null`; it is never silently omitted or replaced. These rows allow
construction completion and the following operational visit's allocation,
replacement and cleanup to be inspected through the normal match path.

The camera calls the existing presentation centering/clamp owner once at L0.
`render.camera` records its requested cell and actual final top-left/zoom. The
existing neutral-input and exact draw requirements remain in force. A terrain
receipt establishes CPU state; the separate retained BGRA frame establishes the
production rendered output. Neither is a native comparison.

Profiles are bounded to 1024 commands, 30 observed House names and 256 terrain
cells. Captures retain at most 100000 combined actor, missing-ID and terrain
samples, including occupied animation-slot samples in v5. Child/run/report JSON
is limited to 128 MiB on read and publication;
the shared JSON owner keeps its 16 MiB default for other tools. Observe only the
Houses needed by the experiment and choose a bounded step budget. Comparison checks
the complete command/actor/terrain trajectory and camera as well as fingerprints
and pixels; large transcripts remain referenced in their sealed source bundles
rather than being copied twice into the comparison report.

For the response chain, attack Computer1's **deployed GACNST** with Computer2's
ordinary MTNK. The yard exercises Building ReceiveDamage's sourced positive-hit
prelude (`44227E..4422BC` -> `708080`); retail GACNST and AMCV both lack
`ToProtect=yes`. The separate generic Techno damage response uses that flag,
with retail protected miners as controls. Computer1's own E1/MTNK defenders are
the response candidates; an allied local House does not substitute for that
ownership gate. For this map site, verify an actual low-bridge defender and an
ordinary ground control, yard damage, Rescue/AreaGuard dispatch, archived victim,
movement and cleanup. Validate an elevated deck separately. Do not presume
deployment clearance, bridge arrival or random mission selection from this example.
The runtime profile establishes production integration, not native whole-world
equivalence. Visible reproduction uses the normal release shell and the same
Battle choices; this capture route remains hidden. `RA2_QUICKPLAY` has no ordinary
AI starting forces and is unsuitable for this response fixture. Campaign-map
sandboxes remain separate from campaign startup validation.

The shared `tools.child_process` owner bounds spawn/wait/cleanup and collects
finite output snapshots. On timeout it kills only its exact child. Existing
outputs are never overwritten. A failed child, failed manifest, missing artifact,
changed input or invalid receipt produces an invalid report when publication is
possible. Exit codes: `0` valid observation, `1` retained invalid observation,
`2` invalid inputs or publication failure. This tool never claims native parity.

Portable validation:

```sh
python -m unittest tools.tests.test_map_observation tools.tactical_certification.tests.test_core tools.tests.test_child_process
```

The saved [validation receipt](map_observation.validation.json) records the checked
release binary and source snapshot, repeated MIX-map state, loose-map and zero-step
captures, missing-map diagnostics, and the explicit coverage limits. It is a run
summary, not a native oracle golden.

## Validate and compare saved observations

Use the same owner to recheck an existing run or compare before/after captures.
These commands read evidence and write one new JSON report outside the run
directories; they do not launch a game or change the captures.

```sh
python -m tools.map_observation validate \
  --run /absolute/evidence/before-map-change \
  --output /absolute/evidence/before-validation.json
python -m tools.map_observation compare \
  --before /absolute/evidence/before-map-change \
  --after /absolute/evidence/after-map-change \
  --output /absolute/evidence/comparison.json
```

Validation reads the fixed artifact paths under the supplied run, checks actual
profile/config/contract/frame/log bytes and child receipt semantics, then
cross-checks the wrapper's copied evidence and original input identities. It does
not trust a stored `VALID` verdict or matching digest strings. The executable must
still exist at its recorded original path and match its recorded length and SHA;
a labeled preserved build makes that requirement durable. The report marks this
as `EXTERNALLY_REVALIDATED`. It cannot validate a deleted or replaced executable
from its old receipt alone.

A new wrapper v5 run validates its `SEALED_COPY` inputs without requiring the
original profile, config or contract files to remain available. It validates the
retained contract's v2 rules without requiring today's checkout to have identical
contract bytes. Offline checking does not apply the current process environment
denylist because it does not launch a child.

Both runs must validate before comparison. Profile, config and contract **bytes**
must match, including formatting; differing inputs make the comparison `INVALID`.
Executable bytes may intentionally differ, and both verified identities appear in
the report. Comparison checks complete initial/final fingerprints, map source,
exact steps, presentation-clock policy and consumed schedule, resident atlas
statistics, surface format and actual BGRA bytes. Clock policies must match; a
legacy wall-clock observation and a diagnostic-clock observation are `INVALID`
together even when their pixels match.
Differences name precise field paths and before/after values. There are no pixel
tolerances or omitted atlas fields.
Current v5 pairs also compare the complete command, rule-handle inventory,
actor/building/animation/terrain transcript and camera. Historical v4 pairs
compare their original command and actor/terrain transcript and camera, while
v3 pairs retain their original comparison fields.

`MATCH` means these checked observations are exactly equal for the compared
fields; it neither establishes independent execution nor certifies native parity.
`MISMATCH` means valid, comparable observations differ. `INVALID` means an input,
artifact, identity or receipt check failed. `native_comparator` and
`parity_certification` remain `NONE`. Compare exits `0` for `MATCH`, `1` for
`MISMATCH`, and `2` for `INVALID` or publication failure; validate exits `0` for
`VALID` and `2` otherwise. Reports are never overwritten.

Run bundles currently must remain at their original absolute location: command
output and wrapper artifact identity paths are cross-checked against the supplied
run. A copied or relocated bundle is rejected, and validation never follows its
stored artifact paths to read some other bundle. Same-directory and symlink-alias
comparisons are rejected. Keep reports outside both evidence directories.

### Historical captures

Offline `validate` and `compare` reject old wall-clock observations by default.
`--allow-legacy-clock` permits child v2 with sealed wrapper v2, preserving its
original `run.capture` projection. Validation identifies its clock separately as
`legacy-wall-clock`; it does not invent a diagnostic transcript or neutral-input
guarantee. Live capture accepts only child v5 and never offers this override.
Wrapper and child generations must correspond: v5/v5, v4/v4, v3/v3, v2/v2, or v1/v2.
Historical v4 remains readable with its original observation policy and profile
v2 trajectory, without production orders, rule-handle inventory or building
fields. V4 and v5 comparisons are invalid because their observation policies
differ. No historical receipt is upgraded or supplied with missing state.
Sealed wrapper/child v3 remains readable without a clock override, with profile
v1 only and its original projection. It cannot declare v4 actor/command/camera
receipts. Comparisons between v3 and v4/v5 are invalid because their observation
policies differ; no missing trajectory is reconstructed.

Wrapper v1 also retained only `profile.json`. It additionally requires
`--allow-legacy-inputs` to reread the original config and contract paths and check
their actual lengths/hashes against the old receipt. These inputs are reported as
`EXTERNALLY_REVALIDATED_UNSEALED`, never as retained copies. Missing or changed
originals fail validation; the profile copy and original executable are checked
as above. Neither legacy flag grants the other permission.

For a comparison between two old wall-clock captures, where one uses wrapper v1:

```sh
python -m tools.map_observation compare \
  --before /absolute/evidence/old-wrapper-capture \
  --after /absolute/evidence/sealed-wall-clock-capture \
  --allow-legacy-inputs --allow-legacy-clock \
  --output /absolute/evidence/historical-comparison.json
```

Same-policy legacy runs may compare when their actual input bytes match. A
legacy/v3, legacy/v4 or legacy/v5 comparison remains `INVALID` with both flags,
including when frame
bytes are equal. Child v1 remains unsupported historical evidence: it lacks atlas
statistics. No command rewrites historical receipts or adds evidence they did not
record.

Focused portable checks, including malformed/tampered packages, input provenance,
exact mismatches, legacy policy and binary selection:

```sh
python -m unittest tools.tests.test_map_observation tools.tests.test_cargo_run
python -O -m unittest tools.tests.test_map_observation tools.tests.test_cargo_run
```

The [checked comparison validation](map_comparison.validation.json) records
whole-suite checks, preserved-label selection, revalidated historical pairs and
new sealed production captures for this workflow. The earlier capture validation
receipt above remains historical evidence for its recorded source revision.

### Checked diagnostic-clock captures

The [presentation-clock validation](map_presentation_clock.validation.json)
records fifteen independent release captures and eight exact full-frame
comparisons on Apple M4/Metal: three Allied 30-step runs, and two each at Allied
zero/200 steps, Soviet 30 steps, and message steps 1/182/183. All same-profile
pairs matched their complete BGRA bytes, simulation fingerprints, atlas evidence
and consumed clock transcript. The announcement was visible at steps 1 and 182
and absent at 183, preserving its current 4000 ms lifetime from its committed
step-1 timestamp. This is a production regression, not native trigger or message
timeout parity.

The receipt retains full profiles, fixture construction, binary/source identities
and commands. Reproduce the base map with the existing
`tools.render_depth_fixture.build_fixture(cliff_back=True)` owner; append the
receipt's literal trigger for the message fixture. Point each retained profile's
`launch.selected_map_file` at the corresponding new absolute map path, then run
and compare fresh output directories with the commands above. The trigger uses
the current announcement action without ending the scenario.

The original historical same-binary pair still reports `MISMATCH` with
`--allow-legacy-clock`: 11032 radar pixels differed. No pixels are excluded to
obtain the new matches, and historical receipts are preserved. New and legacy
policies intentionally cannot compare. The current 600-case native radar timer
check, retained-surface native check, four explicit retail GPU tests and one
retail radar-history test passed with their existing goldens unchanged.

Ordinary `radar-online-v2` live capture remains untested on this macOS host: its
sealed Windows Verdana path fails the existing POSIX absolute-path checks, and
the installed macOS font does not match its sealed bytes. The diagnostic captures
and native/GPU checks do not replace that production coverage.

## Resident unit-atlas measurement

The final rendered frame records `render.unit_atlas` in the v5 child manifest;
the validated wrapper retains it as `capture.unit_atlas` in `run.json`. The
`UnitAtlas` owner reads actual wgpu texture descriptors and resident entries.
It reports resident sprite count, the last actual build's rasterized sprite count,
and each page's extent, format, dimensions, mip/sample counts and texel payload
bytes, plus their total. Page count is the length of `pages`.

Pages are currently single-layer, single-mip, single-sample D2 `R8Uint`, so the
payload is width × height bytes, including unused page space. Unsupported
descriptors fail observation rather than retaining stale byte arithmetic. The
last-build count includes rasterized shadow sprites, can differ from admitted
resident entries, and stays unchanged on a no-op refresh.

This is resident UnitAtlas texture payload for the captured scenario. It excludes
CPU caches, `VxlPoseFrameCache`, `VxlSlopeTransitionCache`, other atlases, palettes,
driver overhead and peak allocation; it does not establish 30-player saturation.
Statistics are captured with the final render evidence before readback and kept
outside deterministic simulation fingerprints. GPU allocation may differ across
adapters even for identical simulation state.

This replaces the retired `measure-atlas` binary, which estimated a hardcoded
roster and tile sizes without constructing an atlas. The unused
`bridge-oracle-compare` binary was also retired: its trace schema had no repository
producer, and its five bin-only tests covered that abandoned comparison schema,
not active bridge behavior. Current native bridge comparison owners remain in
[`anytown_damage`](spatial_oracle/anytown_damage/README.md) and
[`shrapnel_damage`](spatial_oracle/shrapnel_damage/navigation.md); their native
outputs and Rust regression witnesses are preserved.

For a headless retail growth/no-op/fresh-pack comparison on a real GPU:

```sh
VERA20K_REQUIRE_RETAIL_INI=1 python -m tools.cargo_run -- test -p vera20k --lib --release \
  render::atlas_refresh_retail_tests::retail_atlas_refresh_costs -- --ignored --nocapture
```

Set `RA2_DIR` or provide the local config for that test. It emits the same owner's
statistics at each allocation stage; fresh and grown totals need not match.

The [atlas observation validation](atlas_observation.validation.json) records
before/after production captures and the explicit retail GPU atlas refresh test.
The older `map_observation.validation.json` remains historical v1 evidence; it
has not been retroactively given statistics or new source identities.
