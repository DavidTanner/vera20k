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
3. Chronosphere + Chrono Warp, player path: merged (YuriPlanet/vera20k#1089). Launch
   cases 3/4 (`superweapon/chronosphere.rs`), Fire_SW's PostClick pairing (`fire.rs`),
   the Teleport locomotor's Chronosphere states (`movement/teleport_chrono.rs`), the
   local selection writes (`app/match_runtime/super_selection.rs`: case 3/4 and the
   revoke/suspend pass `0x50B181`). Oracle sections `chrono_process`,
   `chrono_update_position`, `chrono_destination`, replayed in
   `superweapon/chronosphere_tests.rs`; production observation
   ([map_observation.md](../../tools/map_observation.md#chronosphere-observation)).
4. Psychic Dominator, player path: merged (YuriPlanet/vera20k#1090). Launch case 7,
   PsyDom::Start, Process and MindControlArea (`superweapon/psychic_dominator.rs`) and
   the Dominator lighting. Oracle sections `psydom_*`, `update_lighting`,
   `ambient_step`, `dominator_lighting_read`, `relight`; production observation
   ([map_observation.md](../../tools/map_observation.md#psychic-dominator-observation)).
5. Psychic Dominator, computer path: merged (YuriPlanet/vera20k#1091). AI_Fire_PsyDom
   `0x50A150` (`superweapon/ai_fire.rs`) and All_To_Hunt's Dominator arm
   (`house_strategy.rs`). Oracle section `ai_psydom` and the
   `tools/ai_strategy_oracle.py` `all_to_hunt` rows; production observation
   ([map_observation.md](../../tools/map_observation.md#computer-psychic-dominator-observation)).
6. Spy Plane, player and computer paths: Launch case 8 and SendSpyPlanes `0x65EAB0`
   (`superweapon/spy_plane.rs`), Mission_SpyplaneApproach/Overfly `0x4155F0`/`0x4157C0`
   (`aircraft/spyplane_mission.rs`), AircraftClass::AI's off-map removal `0x414F47` and
   its predicate `0x41B890` (`aircraft/leave_map.rs`). Oracle sections
   `spy_plane_launch`, `send_spy_planes`, `spyplane_missions`, `aircraft_leave_map`;
   production observations
   ([map_observation.md](../../tools/map_observation.md#spy-plane-observation)).
7. Iron Curtain and Chronosphere, computer path: the team script actions that fire
   them, 55 `0x6EFC70` and 57 `0x6F0130` (`team_script_vm/super_actions.rs`), with
   the AI trigger conditions' readiness test (`superweapon::super_nearly_ready`).
   Oracle section `team_super_actions`, replayed in `super_actions_tests.rs`;
   production observations
   ([map_observation.md](../../tools/map_observation.md#computer-iron-curtain-observation)).
   Action 56 `0x6EFE60` is not ported: only the retail campaign map SOV02SMD.MAP
   uses it (four scripts).

`fire::launch` now dispatches every Launch arm; none refuses a click. `ai_fire.rs` RESIDUALS lists the AI-side gaps (preferred
target writers, AI_FindTeamTarget `0x50D170`, building cloak stage).

## Next chains (one PR each)

- The Iron Curtain's and Force Shield's unit tint: TechnoClass::AI (`0x6F9EAF`) steps
  each curtained techno's tint stage (`+0x1A4`, TechnoClass::UpdateIronTint `0x70E5A0`),
  which draws the scenario RNG once per techno (`RandomRanged(-5, 5)` at `0x70E694`),
  so VERA's scenario stream drifts after every Iron Curtain or Force Shield (read, not
  executed). The draw path has no stage to feed `0x70E380` (a residual in
  `render/draw_state.rs`).
- The nuke's screen flash (`NukeFlash`) and its lighting arm (`+0x3570`/`+0x3574`,
  a residual in `sim/light_sources.rs`).
- Script action 56 `0x6EFE60` for the campaign's Chronosphere teams (SOV02SMD.MAP),
  with `Find_Best_Target_Building 0x6EEBD0`, which actions 46 and 47 share.
- The existing types' gaps: Deactivate's start = -1, the offline-provider hold
  `+0x660`, the paradrop plane's Retreat exit `0x415A50` (a residual in
  `aircraft/paradrop_mission.rs`), and the player tails of Launch cases 0, 1, 2, 9, 10
  and 11 (a residual in `app/match_runtime/super_selection.rs`).

The ChronoWarpTo paths (`0x4DF7F0`, `0x522FE0`) are map-trigger only; they stay a
residual in `superweapon/chronosphere.rs`.

Launch jump table `0x6CDE44`: 0 `0x6CDA67`, 1 `0x6CCE64`, 2 `0x6CCD3F`, 3 `0x6CC3B9`,
4 `0x6CC4B2`, 5 `0x6CD2EE`, 6 `0x6CD537`, 7 `0x6CCDBD`, 8 `0x6CD66F`, 9 `0x6CD7E7`,
10 `0x6CD072`, 11 `0x6CD70C`.
