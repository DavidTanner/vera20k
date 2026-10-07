# Superweapons: current checkpoint

Goal (user `/goal`, 2026-10-06): all 12 retail `[SuperWeaponTypes]` work for players
and AI like active-retail gamemd, traced from sidebar and AI through launch, effects,
EVA and recharge, with native comparisons and production validation; label what is
confirmed in Ghidra; commit, publish and merge validated chains.

## State

Chain 1 (Nuclear Missile, player path) is implemented on branch
`feature/superweapon-nuke` (worktree `vera20k-worktrees/vera20k-superweapons`, from
`origin/main` `035a4f259`): Fire_SW `0x4FAE50` -> ClickFire `0x6CB920` -> Launch
`0x6CC390` case 0 -> Mission_Missile `0x44C980` -> PSIWARN + NukeCarrier -> NukeMaker
`0x46B310` -> NukePayload -> NUKE; the computer houses' alert `0x4FAF00`; the
SuperAnim slots 14..17; the low-power hold (Suspend `0x6CB4D0` from `0x50AF10`); the
FirersPalette bullet scheme (Bullet+0x114); the silo's empty body draw.

Evidence: `tools/superweapon_oracle.py` (122 ClickFire, 75 alert, 15 Mission_Missile,
5 NukeMaker, 96 SuperAnim rows) pinned in Rust tests; retail-rules frame test
`superweapon/nuke_tests.rs`; production run in
[map_observation.md](../../tools/map_observation.md#nuclear-missile-observation).
Ghidra (saved 2026-10-07): `HouseClass__Super_Defense_Alert` 0x4FAF00 (was
Check_Spy_Reveal), `PsiWarning_Player_Detects` 0x43B4C0, `AnimClass__SetBullet`
0x424C90, `HouseClass__Set_Preferred_Defensive_Cell` 0x50DA20,
`HouseClass__Clear_Preferred_Defensive_Cells` 0x50DA50, `HouseClass__Update_Owned_Supers`
0x50AF10 and `HouseClass__Grant_Provided_Supers` 0x50B1D0 (were AI_ManageProduction /
AI_ResumeProduction), with plates.

Residuals of chain 1 are listed at their owners (`superweapon/nuke.rs`, `fire.rs`,
`world/techno_ai/building_missile.rs`): the one-time arm, the offline-provider hold
(`+0x660`), launch presentation of the cursor and queued EVA line, the refused-bullet
path, the missile's Trailer smoke. Unported Launch arms (3, 4, 7, 8) refuse the click
and keep the charge (`fire::launch_ported`).

## Next chains (one PR each, after chain 1 merges)

2. Chronosphere + Chrono Warp: Launch cases 3 `0x6CC3B9` / 4 `0x6CC4B2`, Fire_SW
   PostClick copy, CreateChronoAnim `0x6CB3A0`, StopPreclickAnim, Teleport piggyback.
3. Psychic Dominator: Launch case 7 `0x6CCDBD`, PsyDom::Start `0x53AE50`/Update.
4. Spy Plane: Launch case 8 `0x6CD66F`, SendSpyPlanes `0x65EAB0`, spy-plane missions.
5. AI use: AI_TryFireSW `0x5098F0` (from AI_Building_Strategy `0x4FD500`; case 10 aims
   ForceShield at PreferredDefensiveCell2, else PreferredDefensiveCell while younger
   than `[General] AISuperDefenseFrames=`), its pickers (AI_FindBestRallyTarget,
   AI_FindTeamTarget, AI_Fire_LightningStorm, AI_GroundRallyPoint, AI_Fire_PsyDom,
   AI_Fire_GenMutator) and team-script actions (`0x6EFC70`, `0x6EFE60`, `0x6F0130`).
6. Existing seven (Iron Curtain, Lightning Storm, both Paradrops, Genetic Mutator,
   Force Shield, Psychic Reveal): close player/AI gaps (Deactivate's start = -1,
   the offline-provider hold).

Launch jump table `0x6CDE44`: 0 `0x6CDA67`, 1 `0x6CCE64`, 2 `0x6CCD3F`, 3 `0x6CC3B9`,
4 `0x6CC4B2`, 5 `0x6CD2EE`, 6 `0x6CD537`, 7 `0x6CCDBD`, 8 `0x6CD66F`, 9 `0x6CD7E7`,
10 `0x6CD072`, 11 `0x6CD70C`.
