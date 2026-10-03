# CLEG overlay input propagation closure

Scope: caller/context propagation only after the one accepted critic P1. Root owns ground_move, infantry_scatter, teleport_movement and teleport_cell_destination_tests. No state field, native port, registry cache, recovery owner or RNG/timer ordering was added by this subtask. No Cargo/native/runtime was run here.

## Production input closure

- World command Move/AttackMove/Follow/Guard/miner orders, production rally, pursuit and post-combat resumption now lend their supplied overlay registry to the existing ground order.
- Pending class destinations lend the already-supplied registry to Teleport. The existing finish_setter_destination availability/expect contract remains with Root's input validation repair.
- Unit/Aircraft unload and miner search/return helpers retain their dispatch's borrowed context through their ordinary ground orders.
- Both Mission Restore wrappers construct the existing concrete-effects provider with caller registry. Detach sweeps, pointer expiry, owner change, bridge hut/cell callbacks, cloak, capture/release, slave release and passenger ownership forward that input without changing native callback ordering.
- Existing destruction owners forward context into Crash/KillPassengers/RecordKill-and-UnInit, retiring Infantry/TechnoAI, combat Destroy/Stun, absorbed/escaping passengers, and pending per-cell/object-turn lifecycle requests.
- The live Temporal prologue already had ObjectAiCtx upstream; it now lends registry through its existing Update/Erase owner, free-slave callbacks and UnInit. No FSM state/arithmetic/order changed.
- Player sale/undeploy, computer low-credit sale and House Fire_Sale now lend existing command/ObjectAiCtx/HouseRung context to immediate Limbo/UnInit and conversion callbacks. This includes the dormant stock Firestorm branch rather than treating absence of stock reachability as permission to drop context.

## Explicit absence contexts

New None arguments are confined to existing supplied component/native-control test fixtures that have no resident overlay table, plus existing cfg(test) batch compatibility boundaries. Existing Unit native setter APIs continue to consume their represented Unit-only inputs; they did not gain a second Infantry resolver. Rules-less compatibility lifecycle facades and never-live rejected constructor cleanup keep their existing context boundaries: a fresh rejected constructor has not admitted a command-targetable object. No stored registry or visibility widening was introduced.

## Regression and validation boundary

`world::teleport_anim_object_turn_tests::restoration_callbacks_forward_overlay_inputs_before_any_destination_write` invokes both actual Restore wrappers. A supplied Clear-overlay registry must reach the restored CLEG Cell resolver. Missing and unregistered inputs must return AuthorityUnavailable before byte changes to the entity (Mission/NavCom/timers/runtime), raw occupation or full main/scenario/mapgen RNG state. This is a supplied-fixture Rust callback/transaction regression, not an original overlay admission golden. Root/native owner separately adds original executable overlay controls.

Leaf rustfmt with skip_children and git diff --check passed. Static argument audit checked 239 calls across 44 affected API names with zero old-signature mismatches. Rust compilation and execution remain Root-owned. The world and aircraft mod.rs files were edited only in owned hunks and were not recursively formatted.

Exact owned leaf hashes are in cleg-overlay-context-caller-hashes-20261003.json. All owned repository files are frozen after this receipt.
