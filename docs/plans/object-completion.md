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
| GACNST / Construction Yard | Construction and Grand_Opening integration selected next | Per-building completion ordering, duplicated ready/mission authority, shared opening effects; engineer repair, capture, sale/crew/destruction and AMCV undeploy remain required | Complete the shared Construction → Grand_Opening chain first |
| GAPOWR / Allied Power Plant | Shares Construction/opening; native health-power corpus exists | Same completion/engineer/capture/sale/destruction gaps, plus ordinary drain/spy/powered-art consumers need closure | Reuse Construction; then complete ordinary power lifecycle |
| GAPILE / Allied Barracks | Shares Construction/opening | Same completion/engineer/capture/sale/destruction gaps; factory delivery radio, infantry output and spy effects need closure | Reuse Construction; then complete infantry factory output |

Rows name confirmed gaps and unaudited coverage separately. Nothing in this
table claims that unlisted behavior is already equivalent.

## Current chain: automatic GI deployment

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

## Next chain: Construction → Grand_Opening

Native Building Update43FB20 runs animation43FE22, ready/commence, shared
Techno AI43FE56, then ready/commence43FF91, repair/power4401B6 and factory AI.
MissionConstruction449A50 belongs inside the individual live Logic visit.
VERA's late sorted `tick_building_up` delays completion effects until after the
object pass and bypasses MissionAI's Health > 0 gate. Do not move the currently
bundled `BuildingUp::frame` wholesale: that can dispatch Guard too early.

Consolidate BuildingUp's duplicate mission/done state with MissionCom and
MissionLeaf's ready byte, migrate predictive production/gap-generator consumers,
and retain live append visitation. Radio +274 is Transmit_ToFirst65ACB0, not
broadcast. Construction sends 0xB initially, then 0xC and 3 on completion.

Grand_Opening445F80 has construction449AD4 and discovery44D68A callers with
argument0, and ownership448CEF with argument1. Share one owner for all three.
AlreadyPlaced+6E4 gates initial art/counters/sensors, independently of the
argument. AlreadyPlaced=true with argument0 returns immediately; argument1
continues to later activation. A constructing/unplaced ownership change can
still run the initial effects. FreeUnit separately requires argument0.
Follow art/sound detachment, animation construction RNG, power, house rechecks
and appended-animation consumers on the ordinary GACNST/GAPOWR/GAPILE path.
Production retail keys and original defaults leave FreeUnit, ProduceCash,
healing, purifier, Helipad and RevealToAll branches inactive for all three.
Those large conditional mechanisms remain later chains with explicit residuals;
their full consumers do not block this ordinary completion path. Stored cost+300
and owner counter/capture ordering will be required when those branches are ported.
Existing native construction/repair/health-power corpora do not certify this
future scheduler and opening integration.

Ordinary engineer repair is a subsequent required chain: active
PerCellProcess519630 calls inherited EngineerRepair701410 through vt+40C at
519FF0 after engineer/contact/allied or occupiable-owner admission. It restores
Health and EstimatedHealth, stops repair, updates damaged art, then processes
the tag and engineer teardown. VERA currently has no production producer.

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
