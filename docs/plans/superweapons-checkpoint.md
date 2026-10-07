# Superweapons: current checkpoint

Goal (user `/goal`, 2026-10-06): all 12 retail `[SuperWeaponTypes]` work for players
and AI like active-retail gamemd, traced from sidebar and AI through launch, effects,
EVA and recharge, with native comparisons and production validation; label what is
confirmed in Ghidra; commit, publish and merge validated chains.

## State

1. Nuclear Missile, player path: merged (YuriPlanet/vera20k#1085). Residuals at their
   owners (`superweapon/nuke.rs`, `fire.rs`, `world/techno_ai/building_missile.rs`).
2. Computer houses fire their superweapons: branch `feature/superweapon-ai-fire`
   (worktree `vera20k-worktrees/vera20k-superweapons`). `superweapon/ai_fire.rs` ports
   AI_TryFireSW `0x5098F0` (Strategy step 3, `house_strategy.rs`) with
   AI_FindBestRallyTarget `0x50CBF0`, AI_Fire_LightningStorm `0x509E00`,
   AI_GroundRallyPoint `0x509CD0`, AI_Fire_GenMutator `0x509F60` and the Force Shield
   arm. Evidence: `tools/superweapon_oracle.py` sections `ai_try_fire` (26 rows),
   `ai_best_rally_target` (161, Scenario Random run natively), `ai_ground_rally_point`
   (11), `ai_genetic_mutator` (23, offset table from its initializer `0x561910`),
   replayed in `superweapon/ai_fire_tests.rs`, plus a retail-rules frame test and a
   production observation ([map_observation.md](../../tools/map_observation.md#computer-nuclear-missile-observation)).
   Ghidra (saved 2026-10-07): `LightningStorm__IsActive` 0x53A100; executed-evidence
   plates on the five functions above.

Unported Launch arms (3, 4, 7, 8) refuse the click and keep the charge
(`fire::launch_ported`); `ai_fire.rs` RESIDUALS lists the AI-side gaps (preferred
target writers, AI_FindTeamTarget `0x50D170`, AI_Fire_PsyDom, building cloak stage).

## Next chains (one PR each, after chain 2 merges)

3. Chronosphere + Chrono Warp: Launch cases 3 `0x6CC3B9` / 4 `0x6CC4B2`, Fire_SW
   PostClick copy, CreateChronoAnim `0x6CB3A0`, StopPreclickAnim, Teleport piggyback;
   the AI's team script `0x6EFE60`.
4. Psychic Dominator: Launch case 7 `0x6CCDBD`, PsyDom::Start `0x53AE50`/Update, and
   AI_Fire_PsyDom `0x50A150` (its oracle must run `0x561910` too).
5. Spy Plane: Launch case 8 `0x6CD66F`, SendSpyPlanes `0x65EAB0`, spy-plane missions.
6. The other superweapon team-script actions (`0x6EFC70`, `0x6F0130`; identities
   unverified, Iron Curtain likely) and the existing seven's gaps (Deactivate's
   start = -1, the offline-provider hold `+0x660`).

Launch jump table `0x6CDE44`: 0 `0x6CDA67`, 1 `0x6CCE64`, 2 `0x6CCD3F`, 3 `0x6CC3B9`,
4 `0x6CC4B2`, 5 `0x6CD2EE`, 6 `0x6CD537`, 7 `0x6CCDBD`, 8 `0x6CD66F`, 9 `0x6CD7E7`,
10 `0x6CD072`, 11 `0x6CD70C`.
