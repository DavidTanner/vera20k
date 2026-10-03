# Hills Engineer spawn failures — causal note at 87705976

Inspected clean HEAD `87705976019d4003e98ccfbae879c19e71c04d44`. This is read-only debugging assistance: no Rust changes, Cargo, game, original-instruction execution/emulation, Git/GitHub or Ghidra mutation. The only new artifact is this note. Static original-file instruction inspection used the existing `tools.native_inspect` owner; the executable SHA256 is `1cdd1180e49024fbda8ad568caac2e86e856063ff67ab38f62b7d2c7bb84298c`.

## Causal split

The [primary63 log](/Users/halvor/Documents/vera20k-dev/refactor-goal/movement-target-689-final-primary63-r1-20261003.log:35651) fails both tests at constructor convenience-call unwraps, before CaptureBuilding or zone assertions. They have different prerequisites:

| Fixture start | Saved actual cell inputs | Current rejection |
| --- | --- | --- |
| Legal ENGINEER `(66,76)`, test line319 | Overlay27, land5, level10, slope0, zone0, no bridge flags, clear ground/deck lists and raw bytes | `spawn_object` supplies no registry; class admission reaches the overlay table and reports unavailable |
| Cliff ENGINEER `(69,74)`, test line189 | No overlay, Rock3, level10, slope0, zone6, tile69/subtile4 | Ground Foot speed is zero; class admission returns nonzero. Adding a registry alone cannot admit it |

Both actual cells are retained in [hills-walk-canonical.mark0-112-frozen.json](/Users/halvor/Documents/vera20k-dev/bridge-target-evidence-20260927/engineer-entry-followup/hills-walk-canonical.mark0-112-frozen.json). This is an older Rust-produced input witness, not a new current capture. The legal test's current pre-spawn assertions still check zone0/level10/unblocked ground/no building and actual ENGINEER SpeedType=Foot.

## Owner/body/caller evidence

[spawn_object](/Users/halvor/Documents/vera20k-worktrees/vera20k-height/src/sim/world/world_spawn.rs:779) supplies sampled terrain level to `spawn_object_at_height`, whose existing context call passes registry=None at830. The canonical Infantry frontend at1390/1413 invokes `infantry_unlimbo_position` (original51DFF0→floor578080→Cell481180), then [constructor_unlimbo_placement](/Users/halvor/Documents/vera20k-worktrees/vera20k-height/src/sim/world/world_spawn.rs:1478) calls the shared `foot_can_enter` with direction/height=-1 and previousCell=None. Exact zero alone admits; errors and nonzero codes refuse and the convenience owner discards the transient constructed object.

The [shared entry owner](/Users/halvor/Documents/vera20k-worktrees/vera20k-height/src/sim/world/object_entry.rs:1476) requires registered flags only when a target overlay exists. Missing registration errors at1480. Its later ground speed gate at1799 returns7 when the type's actual Foot row is zero. Query-local bridge/list results do not turn this nonbridge cliff into a deck placement.

Fresh **static original bytes**, decoded from the known function entries via existing native_inspect, confirm the relevant native gates:

- Object5F4F1B reads A8E7AC; nonzero branches directly to5F4F4A. Counter0 resolves the requested Cell, invokes virtual+1AC at5F4F3C, then rejects nonzero EAX at5F4F42..44.
- Infantry51C78B reads independent+418/list-plane guards, then target Cell+EC and Type+67C at51C7A5..BC. Original51C7C3..CE compares the loaded row to zero; zero reaches `EAX=7; RET14` at51C7D3..DC.
- Selected source spans: 5F4F1B/47bytes SHA256 `018e2537cd7f37151c9eefd37513af9fbb283870d2b6852eb06d1c4ef4d22642`; 51C78B/84bytes SHA256 `45a7ebeb36a9ddef300e0ed70804d4cc24ef204f3ffc00e77785fd9837881208`. Static decode alone does not prove a whole caller execution.

[The saved native physical-terrain packet](/Users/halvor/Documents/vera20k-dev/bridge-target-evidence-20260927/engineer-entry-followup/hills-physical-terrain-native.json) joins actual Cliff21.tem bytes (SHA4940e181…, terrain15→native Rock3/slope0) with original Rules674000 execution across RULESMD, absent LANGRULE, Battle and Hills. Every applicable layer retains Rock speeds `[0,0,0,0,1,0,0,0]`, including Foot0. Its executed reduced-class producer483C80 returns6; that is a distinct boundary, not a captured Infantry Unlimbo return.

## Minimal faithful fixture correction

For the legal start, call the existing `spawn_object_with_overlay_registry` with `runtime.resources.overlay_registry`. Keep the current type/terrain assertions, ordinary CaptureBuilding, real approach/repair/consumption and dual-restored-future checks.

For the cliff negative control, retain its intended **already-resident supplied actor on illegal terrain** explicitly. The smallest existing-owner setup is to bracket only that registry-aware constructor/Unlimbo call with `with_object_placement_scope`, then assert the scope is inactive before saving or issuing CaptureBuilding. This supplies the native authored/escape A8E7AC exception; it does not claim ordinary runtime placement on Rock is legal. The current initial-map caller already brackets this state at world_spawn.rs348/351; Object's original common bypass is shown above. Priority changes floor-slot selection/RNG input, so describe this as a declared fixture prestate, not exact reproduction of the old ordinary-spawn history.

Preserve the cliff terrain, the Capture rejection/zone queries, both live/restored snapshots and false connectivity assertions. Do not change Rock costs, choose a legal replacement cell, or reduce the test to a failed spawn assertion: those would remove its existing zone/restore negative-control coverage.

[Saved Hills native validation](/Users/halvor/Documents/vera20k-dev/bridge-target-evidence-20260927/engineer-entry-followup/hills-native-validation.json) executes full Foot/Map prechecks on the old supplied cliff actor and returns false in both live/reconstructed cases; it explicitly excludes native actor loading/placement, AStar, Infantry CanEnter, Capture and repair. The eight Infantry frontend placement controls and18 Gate controls likewise stop at a supplied Foot4D7170 handoff. No saved complete ENGINEER-on-Hills-cliff Unlimbo execution was found; the proposed exceptional fixture bracket is justified by the original common counter gate and current authored caller, not an invented successful ordinary Unlimbo result.

Root's current debug rerun can establish the actual Rust rejection/error at both starts. No new validation was run here. HOLD after this report.

## Inspected identities

```text
src/sim/world/techno_ai/bridge_engineer_entry_tests.rs 194f36857495026c3711be0bf629f4c63462d3a0188c02c737fb85d2020c4d5f
src/sim/world/techno_ai/bridge_target_layer_tests.rs db36d876baed1212a2b03aa4cd6955834d86e9df10e0b9aa1db596232f4d71d6
src/sim/world/bridge_test_evidence.rs 0626034c01ca62fb94c43b70591ad2b1e1b01fe530c5367d36c8804fad666915
src/sim/world/world_spawn.rs 974d0fee19a1164a3f246ac89f549b5b8ceb7bc2de97bb400f1b04b2c91d1457
src/sim/world/lifecycle.rs 9847fa8323a4221cda907a7ea1afc0a46079e1f41795b7e2cae3d1458a9b3a6f
src/sim/world/object_entry.rs 44a614f9fb9494b8958e856dc17f0a677aa291fa8162ec76d82521b6359a9071
src/sim/movement/infantry_entry.rs f57ea9eac9c5bcd2301127edbc67df4b75fc0a295baa3bb2e53ef006172bbdc9
tools/spatial_oracle/_factory_infantry_output/gate.py 554a897da80583b255b1c40144efe51ef45ad633b72d5f855e37d811940bf5f9
tools/native_inspect.py 12038060971ec7a41da24e13a9851e23858fba4542c6e9a9f9ca17f3a8252087
/Users/halvor/Documents/vera20k-dev/bridge-target-evidence-20260927/engineer-entry-followup/hills-walk-canonical.mark0-112-frozen.json 8a1ad1f97109e8d7371667791ee98108ec600d5d083d341e0dbb423931f9a5d1
/Users/halvor/Documents/vera20k-dev/bridge-target-evidence-20260927/engineer-entry-followup/hills-physical-terrain-native.json 000f31ad7b592e806156512aa039b3b73fc8a44d9ed8b3c6736fab728f368e1b
/Users/halvor/Documents/vera20k-dev/bridge-target-evidence-20260927/engineer-entry-followup/hills-native-validation.json 36b3cbb0eb34e304ab30dac809a88be9a1d6d89c16265d9f23556c9bd0a18ede
/Users/halvor/Documents/vera20k-dev/bridge-target-evidence-20260927/engineer-entry-followup/hills-foot-entry-native-receipt.json 5ecf5d5323e1a8a3f89da8d28506b1ff9f44d1c973d41d6cac43c38e73ab77d3
```

