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
| E1 / GI | AI Guard-family deployment, existing Stop/action/completion, deployed reacquisition | Garrison admission/occupant order, AI Hunt/Capture occupation, human deployed Move override, fatal ReceiveDamage selectors/particles; full creation, vision, transport, weapon-rank and lifecycle coverage still needs a closure audit | Complete shared garrison/Hunt/Capture lifecycle after building completion dependencies |
| E2 / Conscript | Ordinary infantry control; correctly refuses GI auto-deploy | Shared garrison/Hunt/Capture and fatal receiver gaps; remaining whole-object requirements not yet exhaustively audited | Reuse each corrected infantry mechanism; audit its ordinary weapon and death route |
| GGI / Guardian GI | Reuses the GI automatic-deploy mechanism and existing deployed weapon owner | Whole-object lifecycle is not certified; GGI-specific antiair/weapon ranks and crush behavior still require their own coverage | Audit after the initial basic objects |
| MTNK / Grizzly | Selection audit pending | No whole-object certificate or exhaustive required-behavior audit yet | Trace primary/elite weapon, projectile/warhead, movement, production and fatal cleanup |
| GACNST / Construction Yard | Construction → Grand_Opening implementation under validation | Engineer repair, capture, sale/crew/destruction and AMCV undeploy remain required; whole-object closure audit pending | Finish the current shared opening chain, then engineer repair |
| GAPOWR / Allied Power Plant | Reuses the current opening chain; native health-power corpus exists | Engineer/capture/sale/destruction coverage, plus ordinary drain/spy/powered-art consumers need closure | Complete ordinary power lifecycle |
| GAPILE / Allied Barracks | Reuses the current opening chain | Engineer/capture/sale/destruction coverage; factory delivery radio, infantry output and spy effects need closure | Complete infantry factory output |

Rows name confirmed gaps and unaudited coverage separately. Nothing in this
table claims that unlisted behavior is already equivalent.

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

## Current chain: Construction → Grand_Opening

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
was inspected; publication is now ready.
No whole-object or loaded-native-scenario certificate follows from these bounded
comparisons.

Ordinary engineer repair is a subsequent required chain: active
PerCellProcess519630 calls inherited EngineerRepair701410 through vt+40C at
519FF0 after engineer/contact/allied or occupiable-owner admission. It restores
Health and EstimatedHealth, stops repair, updates damaged art, then processes
the tag and engineer teardown. VERA currently has no ordinary allied production
producer. The friendly click, arrival, full-health terminal Scatter, repaired sound
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
