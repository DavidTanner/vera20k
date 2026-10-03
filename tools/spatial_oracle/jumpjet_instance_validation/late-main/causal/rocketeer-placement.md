# Dustbowl Rocketeer fixture admission — 2026-10-03

Read-only causal note at owned HEAD `87705976019d4003e98ccfbae879c19e71c04d44`, branch `feature/jumpjet-synchronous-scatter`. Root owns the fixture correction and validation; this note makes no current-chain critic or native-execution claim.

## Recorded failure and inputs

`movement-target-689-regression-green-g6-20261003.log` completed 4 passed / 2 failed. Both failures occur at the E2 `spawn_object_with_overlay_registry(...).expect("conscript")`, before the attack, flight or fire assertions. The separate two-selector r3inputs run reproduced those failures with the original helper and an input-only diagnostic; its as-run source is retained in `movement-target-689-rocketeer-r3inputs-as-run.rs.gz`.

The r3inputs log records the same cells in both tests:

| Role | Cell | Ground raw | Ordered ground list |
|---|---|---:|---|
| JUMPJET source | 71,40 | 0x04 | Entity2243 |
| Parked E2 target (x+11,y) | 82,40 | 0x80 | Entity2211, Entity2216, Entity2221 |
| Flight E2 target (x+16,y) | 87,40 | 0x80 | Entity2236, Entity2241 |

All three cells are in playfield, level1, Rough, Foot cost100, slope0/zone0, with no overlay, bridge or tube. Thus the two target failures are not established playfield, terrain-Foot-cost or missing target-overlay-registry failures. Earlier candidate JUMPJET constructors log `repair overlay receiver lacks registered type` for Foot2208/2213/2218/2223/2228/2233/2238. The later source Foot2243 succeeds.

The diagnostic did **not** print the listed entities' type, owner, placement origin or retail foundation dimensions. Assigning each ID to a particular GAPOWR/NAPOWR, or asserting a measured stock foundation size, remains unobserved. The raw building bit and linked lists are the direct input evidence; the helper sequencing below identifies the source of retained candidate placements without inventing typed identity receipts.

## Existing owner chain and cause

The preserved helper's lines71–95 run a mutating `find_map`: after only same-level/PathGrid-walkable checks, it successfully places GAPOWR at `(x-4,y-1)` and NAPOWR at `(x+15,y-1)`, then attempts **rules-only** JUMPJET placement. A failed later call declines that candidate without undoing the earlier successful plants. Each repeated attempt can retain two buildings.

`world_spawn.rs:779` supplies no registry; `:796` is the existing registry-aware spawn owner. Its failed constructor is discarded at `:907`, but this only removes that constructor, not previously created independent plants. The newly canonical Infantry admission reaches `constructor_unlimbo_placement` (`:1478`) → shared `foot_can_enter`; an overlay without its registered flags returns the actual diagnostic at `object_entry.rs:1480`.

Building publication has one current owner: `lifecycle.rs:1306` links each foundation cell, then marks building raw0x80 at `:1369`. Terrain walkability does not validate that live linked-list/raw occupation. The shared Cell481180 selector (`bump_crush.rs:230,294`, original `481180/48126B..48128A`) ignores building0x80; passing it does not grant class admission. The separate native Infantry +1AC/51BF90 ground-object Building arms remain in `object_entry.rs:1618–1707`; constructor placement accepts only numeric0 (`world_spawn.rs:1503–1511`). These existing decisions explain why selecting a subcell in a terrain-walkable but building-occupied target can still yield spawn refusal. No new selector or placement bypass is warranted.

## Root's bounded fixture correction

The read current helper (`jumpjet_infantry_tests.rs:41`) first selects the flight rectangle through existing height-aware playfield (`:78`), actual Foot cost, PathGrid and raw-empty (`:85`) owners. It then attempts JUMPJET through the registry-aware owner (`:91`). Plants are selected **after** successful flight placement, independently, at least24 cells from the hold location (`:105–113`). A declined single-object candidate has no earlier successful sibling to leak. This restores the fixture's intended open flight/fight input and supplies the already-available registry. It neither bypasses admission nor manually clears occupation.

E2 positions x+11/x+16, the hold x+8, command gestures, range/approach/phase/fire/crash assertions remain intact. The old east-plant anchor lies adjacent to the flight target; whether its exact stock foundation covered that target need not be guessed to justify keeping power plants outside the whole fight.

Four live consumers share this helper and require validation:

- `jumpjet_infantry_tests.rs:154`: `retail_dustbowl_rocketeer_flies_hovers_and_fires_in_its_airborne_poses`.
- `jumpjet_infantry_tests.rs:311`: `retail_dustbowl_parked_rocketeer_engages_nearby_enemies`.
- `jumpjet_infantry_tests.rs:573`: `retail_dustbowl_shot_down_rocketeer_falls_and_leaves_no_body`.
- `jumpjet_cruise.rs:1087`: `retail_dustbowl_night_hawk_hovers_over_a_tank_on_its_cell` (calls the helper at1094).

At this note's freeze Root's g7 run is active for the first two real selectors; a third requested selector does not exist and cannot count as validation. No result is inferred. Existing native Cell/Infantry admission corpora support the cited owner boundaries; no fresh original Rocketeer/E2 placement execution or whole-match parity was performed for this diagnosis.

## Pins

All hashes are SHA256, raw file bytes. Production files were only read; the current fixture leaf is Root's correction, not the preserved as-run diagnostic source.

- `/Users/halvor/Documents/vera20k-worktrees/vera20k-height/src/sim/world/jumpjet_infantry_tests.rs`: `ecbe5e31a70f47f437cc4fc3660d835588f7bd36c502c3f36b961a419dfb440f`.
- `/Users/halvor/Documents/vera20k-worktrees/vera20k-height/src/sim/world/jumpjet_cruise.rs`: `fb777ae8b044510061ef08b6b53611a3d1313821573adb86e2ec6c1d8fcd2b79`.
- `/Users/halvor/Documents/vera20k-worktrees/vera20k-height/src/sim/world/world_spawn.rs`: `974d0fee19a1164a3f246ac89f549b5b8ceb7bc2de97bb400f1b04b2c91d1457`.
- `/Users/halvor/Documents/vera20k-worktrees/vera20k-height/src/sim/world/object_entry.rs`: `44a614f9fb9494b8958e856dc17f0a677aa291fa8162ec76d82521b6359a9071`.
- `/Users/halvor/Documents/vera20k-worktrees/vera20k-height/src/sim/movement/bump_crush.rs`: `c44cfb1221f317169dbce9461885d20f4c3f969b3544b2a698329cadd62b61c8`.
- `/Users/halvor/Documents/vera20k-worktrees/vera20k-height/src/sim/world/lifecycle.rs`: `9847fa8323a4221cda907a7ea1afc0a46079e1f41795b7e2cae3d1458a9b3a6f`.
- `/Users/halvor/Documents/vera20k-dev/refactor-goal/movement-target-689-regression-green-g6-20261003.log`: `bbf11225e43a110fd5ddec47f324842ef31499cce2776245afa1cd38577b3c87`.
- `/Users/halvor/Documents/vera20k-dev/refactor-goal/movement-target-689-regression-red-r3inputs-20261003.log`: `a04ecdd6edaac571d913d1eb997da8d596899f6eef0bfceb0d1b5c2682c36d1b`.
- `/Users/halvor/Documents/vera20k-dev/refactor-goal/movement-target-689-rocketeer-r3inputs-as-run.rs.gz`: `c1eb03e2defcbd5690c2592e776f510045f63038ccd83b321079bd5466f96240`.
- Decoded r3inputs as-run Rust source: `ec443ea35c69340c4da8612b1b1c3fdeed4ddcf3dbadd015f06559ab6673d740`.

## Plant clearance follow-up (Root-owned, after g7 terminal)

The first correction's far-plant searches still use convenience Building spawn; its generic Mark seam does not itself establish whole-foundation clearance. Root will call the existing `build_site::can_place_building_at` (`src/sim/build_site.rs:70`) before each plant attempt, with actual retail ObjectType, live overlay registry, packed origin and actual House owner. This is already the sole original `464AC0 → 716150 → 47C620` foundation admission composition, used by `production/production_placement.rs:473` and `ai_base_building.rs:617`. It walks the actual fixed ART foundation (`:90`) and its ordinary live-object arm rejects any ground-list member (`:352`). The second plant therefore tests against the already-published first plant instead of assuming a distant anchor establishes empty foundations. Preserve existing PlaceAnywhere/ToTile and type branches as owned; do not re-inline their decisions. No new port, caller-specific placement wrapper, raw clearing or scenario bypass is needed. This pending fixture prerequisite is not claimed as already tested by g7.

- `src/sim/build_site.rs` source SHA256: `a8f5c5e77583eb6ee1259f4670830b1555f285d3ac682ec06cb5c94c03d66965`.
