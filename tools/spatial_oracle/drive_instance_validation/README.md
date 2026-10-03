# Drive/Ship instance ownership validation

The private [class payload](../../../src/sim/movement/drive_locomotion.rs) owns
slope history and retained destination, paid head, track progress, validity, turn
latch, target speed and occupation projections. Drive also owns END permission.
GameEntity no longer carries separate retained class Options. Foot speed, NavCom
and path state retain their distinct owners. Installed class selection and saved
occupation traversal do not dispatch a suspended locomotor.

Snapshot 288 stores the complete class object through the existing piggyback owner
and rejects the prior layout. Each retained Option hashes once inside its class.
The [replay attribution](../drive_instance_replay/README.md) recovers the incoming
Global, Bridge and Slice6 pins with the old feeds in the same executable; complete
601/201/17 observations agree except state_hash, including all three RNG states.
The temporary control is removed. This is bounded Rust attribution.

The [original CMIN controls](../cmin_dock.md) add three histories with 13 steps,
121 boundaries and 97 completed native returns: fresh construction, reuse,
moving/permission/swap END refusals, Stop, END, final Release and another fresh
construction. No RNG calls occur and all three complete RNG states remain equal.
The prior 50 rows are exact. The fixed allocation, callback, docking and zone seams
are declared; these controls do not certify paid Drive Process, Ship behavior,
generic nested BEGIN or whole native persistence.

The actual v1/v2 receipts and logs are in [native/](native/). v1 decoded slope
bytes incorrectly; v2 reads full dwords at Drive+1C/+20, supplying current 7 and
previous 9. v1 is preserved to explain the correction and is superseded as input.
The [Rust child fixture](../../../src/sim/world/cmin_instance_tests.rs) reuses the
existing CMIN scene and recorded native before-inputs. BlockagePathDelay 22 comes
from the native fixture's Rules+1768 input, read through the production [AI] reader;
no native after-state supplies inputs. Native Stop promotes float32 0.3. The
existing SimFixed fraction and early Foot-zero/Stop unwind policies are unchanged.

The actual [integrated final checks](main-integration/final/receipt.json), after
main 9f631a2a, are required-retail lib 9,572 passed, zero failed/227 ignored;
Clippy exit 0; normal release exit 0; and 63 selected ignored retail tests passed,
zero failed/ignored, with no fixture skips. All 1,911 frozen Rust/Python files
remain exact. Repository Python checks succeed with 552 tests and five optional
skips; skill mirrors match. The [prior candidate](final/receipt.json) passed
9,575/0/227 before main replaced its paradrop tests. Clippy exposed 18 inspection getters used only by tests;
those now compile under cfg(test). [Prior API checks](final-api/receipt.json)
record the subsequent 30 focused owner/CMIN/all-three-replay tests, Clippy and
rebuilt release, each exit 0. That guard changes no production logic. The field
ratchet is 2,513 <= 2,532: 19 fewer writable simulation fields.

The [ordinary release profile](../../map_observation.cmin-instance.example.json)
uses the established AnyTown power/barracks/refinery route: refinery queue at
step 1,300 and placement at 2,800, using captured retail interned type 273.
[Integrated runtime evidence](main-integration/runtime/scope-result.json) records
6,000 steps and 6,001
observations. CMIN 1453 appears at step 2,851: 3,150 samples, 1,137 distinct
positions, maximum cargo 20 bales, 1,806 harvesting samples, 96 unloading samples
and three return hops. Capture and offline validation both report VALID. The
actual GPU BGRA frame and PNG inspection view are retained. Complete observations
and final GPU bytes equal the [prior accepted run](runtime/scope-result.json). This is production
integration evidence; native comparator and parity certification are NONE.
The first profile omitted the refinery commands and never spawned CMIN; its
missing-coverage result remains preserved and is superseded by the corrected run.

[first-full/](first-full/) preserves the initial 9,567/8/227 result. Six orphaned
fixture setups were corrected without changing assertions; two hash pins were
updated only after actual old-feed attribution. [space-blocked-attempt/](space-blocked-attempt/)
preserves an attempt that stopped before Cargo, without comparison evidence.
Space later rose to 22.7 GiB and work resumed without goal cleanup. Binding-only
receipts preserve source-pin updates through the central publisher: no new VM
generation, all 497 original native payloads and nonbinding provenance unchanged.
After main integration, [six binding-only calls](main-integration/binding/receipt.json)
refresh three comment-only INI-reader source stamps while all 522 tracked JSON
files and nonbinding metadata stay exact. Two pre-binding harness mistakes are
preserved in that archive; neither changed repository files or native outcomes.

The single independent [read-only critic](review/critic.md) found no actionable
defect in this ownership migration. Its reviewed source, original byte checks,
actual saved evidence and coverage limits are recorded. Main integrated cleanly,
and the owner completed the combined candidate validation above. This review
does not certify wider locomotion.
