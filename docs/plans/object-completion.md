# Ordinary-match object completion

The active 2026-10-01 goal requires three objects with no unresolved required
behavior, established by original active-retail executable comparisons and
production validation. Completing one shared mechanism does not complete an
object. **Certificates issued: 0 / 3.** Publication and auto-merge of validated
mechanisms are authorized.

Native reference: `gamemd.exe` SHA-256
`1cdd1180e49024fbda8ad568caac2e86e856063ff67ab38f62b7d2c7bb84298c`.
Original instructions, active callers and production-selected retail data govern
requirements. Ghidra names and old investigations remain leads.

## Initial objects and required residuals

| Object | Current mechanism | Required behavior still open | Next audit or chain |
|---|---|---|---|
| E1 / GI | AI Guard-family deployment, existing Stop/action/completion, deployed reacquisition; bounded paid barracks output has native comparisons and repeated release validation | Garrison admission/occupant order, AI Hunt/Capture occupation and fatal ReceiveDamage selectors/particles; full creation, vision, transport, weapon-rank and lifecycle coverage still needs a closure audit | Complete shared garrison/Hunt/Capture lifecycle after building completion dependencies |
| E2 / Conscript | Ordinary infantry control; correctly refuses GI auto-deploy | Shared garrison/Hunt/Capture and fatal receiver gaps; remaining whole-object requirements not yet exhaustively audited | Reuse each corrected infantry mechanism; audit its ordinary weapon and death route |
| GGI / Guardian GI | Reuses the GI automatic-deploy mechanism and existing deployed weapon owner | Whole-object lifecycle is not certified; GGI-specific antiair/weapon ranks and crush behavior still require their own coverage | Audit after the initial basic objects |
| MTNK / Grizzly | Selection audit pending | No whole-object certificate or exhaustive required-behavior audit yet | Trace primary/elite weapon, projectile/warhead, movement, production and fatal cleanup |
| GACNST / Construction Yard | Construction → Grand_Opening merged in#992 | Engineer repair, capture, sale/crew/destruction and AMCV undeploy remain required; whole-object closure audit pending | Validate shared Engineer entry/repair, then ordinary destruction |
| GAPOWR / Allied Power Plant | Reuses merged opening, Engineer repair/House consumers#995 and bounded ordinary fatal chain#1014 | Whole-object closure remains open for sale, incoming drain/spy and further power/art requirements; legacy EMP/power-toggle leads require active stock producer evidence | Complete ordinary sale after the current infantry output chain |
| GAPILE / Allied Barracks | Reuses merged opening; paid infantry output, automatic clearance and rally handoff have bounded native comparisons and repeated release validation | Engineer/capture/sale/destruction coverage, specialized factory paths and spy effects still need closure | Confirm output PR#1034 merges, then reuse shared sale and infantry lifecycle work |
| ENGINEER / Allied Engineer | Ordinary damaged-GAPOWR entry/repair and required arrival-time capture/House consumers merged in#995, with native, strict Rust and repeated production validation | Live Tag/Trigger, MultiEngineer damage, Hospital/grinder and other specialized entry routes; full-object closure audit pending | Reuse merged repair/capture owners, then trace remaining entry and Tag prerequisites |

Rows name confirmed gaps and unaudited coverage separately. Nothing in this
table claims that unlisted behavior is already equivalent.

A sealed next-chain power audit (`gapowr-drain-spy-residual-research-20261003`,
manifest`6a9e7d3b224230a9caa1e73f8af8a68ea0493d5471383a46f217bae9084ba57b`)
executes stock Spy action9→Capture8→physical Walk/PerCell→Building4571E0→
House50BC90→power508C30→normal UnInit/deferred disposal with warm native Cell
startup. Spy action/arrival is absent in production. Drain already has shared
reciprocal link/update/expiry/power owners; required DISKRAY attachment, loop sound
and retirement are missing. SpyPowerBlackout's native default0/signed reader and
CombatDamage drain defaults100/0/NULL differ from current readers. Full travel,
blackout expiry, live Tag/radar/EVA and stock DISK install/stop still need native
comparisons. These required routes keep GAPOWR closure open; no implementation
is included in the current infantry-output chain.

Shared Infantry constructor admission also exposes the existing GeneticConverter
compatibility residual: mutation retains enemy corpse membership and immediately
attempts BRUTE creation. Ordinary Infantry51BF90 refuses those enemy owners (7 for
unarmed,5 for armed), so the immediate replacement fails; the victim kill still
commits. Friendly retained corpses with free raw slots can admit the attempt.
Native AnimToInfantry caller timing, placement and corpse cleanup remain required
for this specialized mechanism and InfDeath9 lifecycle coverage. The infantry-output
chain does not invent a Scenario scope to restore the old eager replacement.

## Merged chain: automatic GI deployment

Trigger: a due Guard, Sticky or AreaGuard dispatch of an undeployed
computer-owned Infantry with Deployer, DeployFire, UndeployDelay ≤ -1, NULL
NavCom, expired difficulty delay, an optional ArchiveTarget in the same cell,
and ImmuneToRadiation=false. Retail E1 and GGI use Walk and take this branch.

Original `5214F7..5216BF`, called by `51F620` / `51F640`, compares the wrapping
signed sum of mission start and difficulty delay with the signed current frame.
It truncates Object virtual+48 XY / 256 and narrows to WORDs for the archive
comparison; Z is ignored. Stationary dispatch requests unforced DoAction27 and
returns raw signed Deploy count. Moving dispatch runs Stop first, then writes
Infantry+6E4=1 and draws one Scenario(0,2) after the mission-rate conversion.
Only return -1 resumes the appropriate Foot handler, after any shim effects.

Ownership: MissionCom owns mission start/dispatch timer; HouseState owns the
difficulty vector lookup; MissionLeaf owns Doing and pending6E4; Walk owns the
paid head and Stop; the existing action/Stage and deploy completion owners
produce animation, sound, crush and passive-scan timer effects. No competing
deploy countdown or state field is introduced. Orders and limbo add no pending
reset: native ctor517ABC, producer52167C and callback521B52 establish the byte's
observed lifecycle. Limbo/fatal Stop reaches that existing callback.

Required consumer correction: deployed `51F330` calls SelectWeapon(NULL)
`5218E0`, then CanFireAt `6F77B0` / InRange `6F7220`; it does not retain a target
merely because it is alive. An empty rescan assigns NULL and clears firing68D.
The idle exit uses the existing JumpJet field (+D94, default false,
reader7151E5..715200). Both Guard5214BD and human Attack51F500 use this same owner.

Native comparisons are additive to the existing
[deploy/action fixture](../../tools/spatial_oracle/infantry_deploy_action.py):
`--automatic-guard` saves 90 producer/caller controls and 10 deployed selection,
range and cleanup controls. One AreaGuard archive-mismatch control ends at the
shared shim; return-to-post navigation belongs to the separate Foot owner.
Weapons are absent in the producer controls to isolate that branch and its
empty-scan fallback. Reacquisition controls supply actual weapon slots, unchanged
native bodies/vtables and empty registered scan storage. These are bounded
comparisons, not loaded-native-scenario or populated-scanner certification.

Reproduce with the shared Cargo owner and original retail executable:

```sh
python -m tools.spatial_oracle.infantry_deploy_action --automatic-guard --check
python -m tools.spatial_oracle.infantry_deploy_rules --check
VERA20K_REQUIRE_RETAIL_INI=1 python -m tools.cargo_run -- test -p vera20k --lib sim::world::techno_ai::mission_handlers::
```

Final candidate validation: strict-retail full lib suite **9407 passed, 225
ignored**, and strict-retail lib clippy completed successfully (repository warnings
remain). The simulation field ratchet is **2597 → 2597**, and edited leaf-file
formatting passes. Native automatic100, reader42 and preserved action355 outputs
match checked original execution. The required Python tooling suite passes
(435 tests, four skipped). Release-map production observation and one
fresh critic are not object certificates. Release production observation is now
valid: the ordinary Battle/AnyTown starting forces were advanced 250 exact steps
through the app loader and Metal renderer. Five stationary computer-owned GIs
entered Deploy27 at frames113–119 and reached Deployed28 at128–134. All three
human GIs remained undeployed. Another computer GI retained a live NavCom and
correctly refused this branch. The captured frame shows the deployed sprites;
no native pixel or whole loaded-scenario parity is claimed. The one fresh critic
found no logic or ownership defects and independently passed the100 native
controls. Its minor native-address comment correction was checked against the
original disassembly and fixed. A repeat matches all 250-step trajectories,
fingerprints and final GPU bytes. The compact
[production receipt](../../tools/spatial_oracle/infantry_auto_deploy.production.json)
records the durable sealed bundle and literal state transitions.

## Merged chain: Construction → Grand_Opening

Native Building Update43FB20 runs animation43FE22, ready/commence, shared
Techno AI43FE56, then ready/commence43FF91, repair/power4401B6 and factory AI.
MissionConstruction449A50 belongs inside the individual live Logic visit.
The former late sorted `tick_building_up` delayed completion effects until after
the object pass and bypassed MissionAI's Health > 0 gate. The current candidate
runs animation, both ready checks, Construction dispatch and queued body changes
through that individual visit. Guard remains queued until the next native
dispatch boundary.

MissionCom owns mission/status/cadence; MissionLeaf owns ready; the existing
GameEntity StageClass owns animation timing. Private BuildingBody owns current
and queued body state and the bound buildup control. The former BuildingUp and
BuildingDown countdowns are removed; descriptors supply entry state only.
Consumers of production, power, gap, repair, fire, presentation, hashes and saves
use those owners. Live append visitation is retained. Radio +274 is
Transmit_ToFirst65ACB0. Construction sends 0xB initially, then 0xC and 3 on
completion. PLACE preserves the existing FindFactory owner's native House-order
and primary selection; reveal/capture initialize primary through448070.

Grand_Opening445F80 has construction449AD4 and discovery44D68A callers with
argument0, and ownership448CEF with argument1. All three now call one owner.
AlreadyPlaced+6E4 gates initial art/counters/sensors, independently of the
argument. AlreadyPlaced=true with argument0 returns immediately; argument1
continues to later activation. A constructing/unplaced ownership change can
still run the initial effects. FreeUnit separately requires argument0.
Opening binds active/damaged slots through the existing AnimClass owner;
completion and destructor release the object sound before removal. The existing
power, sensor, superweapon, free-unit and cash owners consume opening in place.
Production retail keys and original defaults leave FreeUnit, ProduceCash,
healing, purifier, Helipad and RevealToAll branches inactive for all three.
Those large conditional mechanisms remain later chains with explicit residuals;
their full consumers do not block this ordinary completion path. Stored cost+300
and owner counter/capture ordering will be required when those branches are ported.
The additive [joined native comparison](../../tools/spatial_oracle/building_construction.md)
executes six complete bounded routes through completion and C+1, seven factory
selection controls and two primary initialization controls. Five stock routes
use no RNG; the asymmetric HasStupidGuardMode control pins both raw advances,
including rejection. The existing primitive payload is preserved. Rust compares
every body/timer/mission/ready state, retained animation type/runtime/slot/ID,
live append order, complete RNG state and sound start/release boundaries. A
separate eighteen-route comparison runs ordinary Building object AI.
The joined corpus supplies prior map/House/Building admission and excludes the
full native scheduler, Building constructor/Unlimbo and Techno common AI. Normal
release-match MCV → power plant → barracks validation now passes through normal
commands, followed by GI production from the opened barracks. A repeat matches
all1450 retained steps, fingerprints and final Metal frame bytes; the frame was
inspected. The [production receipt](../../tools/spatial_oracle/building_construction.production.json)
retains exact identities, completion/next-operational states and explicit limits.
Final strict-retail lib checks pass9420 tests with226 ignored, and lib clippy
completes with732 repository warnings. The field ratchet is2575 versus2589 on
fetched main; edited leaf formatting and diff checks pass. Full Python checks
pass457 tests with four skips. The single fresh construction critic finished with three confirmed defects.
The separate native +ED8 facing reset, Helipad export offset and duplicate
load/live Anim constructor are corrected. Both behavior regressions failed
before the fixes. The corrected candidate again passes9420 strict-retail lib
tests and clippy (732 repository warnings), with457 Python tests passing and
four skipped. The final v2 release repeats all1450 steps and GPU bytes, and
the updated production receipt retains its source/binary hashes. The final frame
was inspected. [PR#992](https://github.com/YuriPlanet/vera20k/pull/992) merged at
35fb0944b8a46efc171be82afc8bb356b5209665; all three supported OS checks passed.
No whole-object or loaded-native-scenario certificate follows from these bounded
comparisons.

Ordinary Engineer repair merged in [#995](https://github.com/YuriPlanet/vera20k/pull/995): active
PerCellProcess519630 calls inherited EngineerRepair701410 through vt+40C at
519FF0 after engineer/contact/allied or occupiable-owner admission. It restores
Health and EstimatedHealth, stops repair, updates damaged art, then processes
the tag and engineer teardown. The friendly click, arrival, full-health terminal Scatter, repaired sound
read, damaged-art update and teardown require one shared repair chain. Tagged
arrival event1 and consumed-engineer event48 need a larger live tag mechanism;
existing tag corpus coverage does not establish a production receiver.

## Retail premises

Initial production `asset ini-get` on Battle mode1 / Hills.mmx selected RULESMD from
expandmd01.mix, no LANGRULE, MPBattleMD and the map. The selected mode/map leave
the inspected deployment and ordinary building keys unchanged. Rules bytes hash
`3d341ef8a13a4b5ab24af2eef48ac94931ac2bb87d950fe3330a07e2d25672ef`;
fixed ARTMD hash `e1f0378394313c04ebbd5073f47785ee3e46f1b3c62d65724e8f3c310ee7ba31`.
Absence of an authored key does not establish its native default.
The committed [production accessor receipts](../../tools/spatial_oracle/infantry_auto_deploy.retail-inputs.json)
contain 43 inputs on the release observation's retail AnyTown map
(`XMP03T4.MAP`). The first 35 agree with the initial Hills route; eight additional
GGI sequence and E1/GGI sound receipts establish GuardianGISequence Deploy15,
Deployed1, Undeploy2 and the distinct GuardianGIDeploy sound request.

E1 Deployer/DeployFire=yes, native UndeployDelay default -1,
ImmuneToRadiation=false, DeployFireWeapon default1. General difficulty delays
are15,25,100. GISequence Deploy is300,15,0; Deployed292,1,1;
Undeploy276,2,2. Primary M60 Range4, secondary Para Range5, elite ParaE Range6.
The production reader test exercises these physical inputs and both E1/GGI's
bound runtime transition through Deploy27 to Deployed28; human E1 and E2 are
negative controls. It does not load a retail map or certify whole object parity.

GACNST is a 4×4 construction factory, Strength1000, Power0, Crewed/Capturable,
UndeploysIntoAMCV. GAPOWR is2×2, Strength750, Power200, PoweredSpecial,
Drainable/Spyable/Crewed/Capturable. GAPILE is3×2, Strength500, Power-10,
Infantry factory/Spyable/Crewed/Capturable. Their buildup/active/damaged artwork,
ordinary prerequisites and applicable shared mechanisms remain required.

## Later prerequisite: specialized Anim pointer expiry

Native Anim destructor4228E0 broadcasts generic pointer expiry at422906 through
7258D0. Building44E8F0 includes slot18/Grinding/SpecialAnim consumers beyond the
current represented direct cleanup in `scalar_delete_building_anim`; the generic
world notification currently represents Entity/Smudge sources. Trigger: retirement
of an Anim referenced by those specialized Building paths. Frequency follows each
activation/retirement; downstream risk is stale specialized references or missed
activation/cleanup. No reached omitted consumer was established for the selected
ordinary GAPOWR fatal route, whose compared poststates match. The completed single
fatal critic records this as later prerequisite coverage, not a proven ordinary
fatal defect or universal Anim destructor parity.


## Validated chain: paid barracks infantry output

Human GAPILE -> two E1 now retains the paid limbo identity through next-prefix
PLACE, shared factory/exit/class admission, facing/idle, reciprocal contact/tether,
occupied-building clearance, Archive/rally handoff and terminal movement cleanup.
Refusal refunds/disposes through the scalar owner; absent producers retain the
head. Crew/slave consumers use that same class-entry owner. The app records the
complete admitted frame batch, including automatic PLACE after accepted SetRally.

[Original replay and coverage](../../tools/spatial_oracle/_factory_infantry_output/README.md)
retain warm Foot/Walk/Cell startup, both terminal products and full three RNG streams.
Original before/after-Strip controls demonstrate the same-frame rally ordering;
the focused Rust race failed before the prefix correction. Gate/frontend and
UnitReady clock/radar/complete PCM comparisons cover required shared consumers.
Whole native MainTick/House/unrelated actors, stock rendered pixels and hardware
latency remain outside those native comparisons.

The preserved main7e9 candidate passes9610 strict lib tests/232 ignored, clippy
exit0/725 warnings with no edited-line diagnostic,126 Python checks and2513/2513
field authority. [Same-binary Rust attribution](../../tools/spatial_oracle/factory_infantry_output_replay/main7e9/receipt.json)
restores both incoming hashes while all618 actor/command/full-RNG observations agree
except the hash; no native golden was changed.

[Production receipt](../../tools/spatial_oracle/factory_infantry_output.production.json)
records six retail AnyTown Metal captures: no rally, early rally, and rally accepted
between first completion and next-prefix PLACE, each repeated. All six validate and
all three repeat comparisons MATCH state/command trajectories, hashes and GPU bytes.
Both paid GIs preserve their identities, physically move and settle with Nav/Archive/
Walk/contact state clear; actual wallet8300/spent1700 includes both purchases.
The runtime observer does not directly export Factory5D/final Factory holders;
joined original/Rust comparisons cover those fields. Lossless PNG QA shows both GIs.
These are production validations, not native world/render or whole-object certificates.
The [single fresh read-only critic](../../tools/spatial_oracle/_factory_infantry_output/critic.md)
found no confirmed defects and judged the scoped chain ready for a PR. PR#1034
is published with auto-merge enabled; ordinary GAPOWR sale follows its merge.

Integration with newer main cf4317 preserves the incoming depot owners and combines
the capture validator's Walk and pending-entry projections. The frozen integrated
candidate passes9613 strict lib tests/231 ignored, clippy exit0/725 warnings and98
Python checks. Its six refreshed retail captures all validate, and three repeat
comparisons MATCH. Complete prior saved observations, hashes, commands, clocks and
GPU bytes remain identical after removing only the added read-only pending-entry
field. The production receipt's `current_candidate` retains source/build/run
identities and the exact comparison. Native helper identities and goldens are
unchanged; no second critic pass was run. Whole-object certificates remain0/3.
