# Superweapons: current checkpoint

Goal (user `/goal`, 2026-10-06): all 12 retail `[SuperWeaponTypes]` work for players
and AI like active-retail gamemd, traced from sidebar and AI through launch, effects,
EVA and recharge, with native comparisons and production validation; label what is
confirmed in Ghidra; commit, publish and merge validated chains.

## State

1. Nuclear Missile, player path: merged (YuriPlanet/vera20k#1085). Residuals at their
   owners (`superweapon/nuke.rs`, `fire.rs`, `world/techno_ai/building_missile.rs`).
2. Computer houses fire their superweapons: merged (YuriPlanet/vera20k#1087).
   `superweapon/ai_fire.rs` ports AI_TryFireSW `0x5098F0` and its target pickers;
   evidence in `tools/superweapon_oracle.py` `ai_*` sections.
3. Chronosphere + Chrono Warp, player path: branch `feature/superweapon-chronosphere`
   (worktree `vera20k-worktrees/vera20k-superweapons`). Launch cases 3/4
   (`superweapon/chronosphere.rs`), Fire_SW's PostClick pairing (`fire.rs`), the
   Teleport locomotor's Chronosphere states (`movement/teleport_chrono.rs`), the local
   selection writes (`app/match_runtime/super_selection.rs`: case 3/4 and the
   revoke/suspend pass `0x50B181`). Oracle sections `chrono_process`,
   `chrono_update_position`, `chrono_destination`, replayed in
   `superweapon/chronosphere_tests.rs`; production observation
   ([map_observation.md](../../tools/map_observation.md#chronosphere-observation)).

Unported Launch arms (7, 8) refuse the click and keep the charge
(`fire::launch_ported`); `ai_fire.rs` RESIDUALS lists the AI-side gaps (preferred
target writers, AI_FindTeamTarget `0x50D170`, AI_Fire_PsyDom, building cloak stage).

## Next chains (one PR each, after chain 3 merges)

3b. The AI's Chronosphere: team script `0x6EFE60` and the ChronoWarpTo paths
   (`0x4DF7F0`, `0x522FE0`).
4. Psychic Dominator: Launch case 7 `0x6CCDBD`, PsyDom::Start `0x53AE50`/Update, and
   AI_Fire_PsyDom `0x50A150` (its oracle must run `0x561910` too).
5. Spy Plane: Launch case 8 `0x6CD66F`, SendSpyPlanes `0x65EAB0`, spy-plane missions.
6. The other superweapon team-script actions (`0x6EFC70`, `0x6F0130`; identities
   unverified, Iron Curtain likely) and the existing seven's gaps (Deactivate's
   start = -1, the offline-provider hold `+0x660`).

Launch jump table `0x6CDE44`: 0 `0x6CDA67`, 1 `0x6CCE64`, 2 `0x6CCD3F`, 3 `0x6CC3B9`,
4 `0x6CC4B2`, 5 `0x6CD2EE`, 6 `0x6CD537`, 7 `0x6CCDBD`, 8 `0x6CD66F`, 9 `0x6CD7E7`,
10 `0x6CD072`, 11 `0x6CD70C`.
