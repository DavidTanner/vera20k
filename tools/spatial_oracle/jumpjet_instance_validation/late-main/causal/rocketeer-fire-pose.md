# Rocketeer discharge and end-frame pose — 2026-10-03

Read-only diagnosis at owned HEAD `87705976019d4003e98ccfbae879c19e71c04d44`, branch `feature/jumpjet-synchronous-scatter`. Native evidence is original instruction/body order plus previously executed saved leaf controls. No new native VM, whole-shot execution, Cargo, game, repository or issue mutation was performed.

## Conclusion and current witness boundary

A true discharge can end the same Infantry turn in **Hover while Jumpjet remains in phase3 and its target is alive**. Infantry FireAt clears the firing latch; the subsequent locomotion-action tail reads it live and requests Hover at speed fraction <=0.8. The target's health is not a gate in that tail. Phase2 has a different stationary tail that retains existing FireFly26. Thus an unconditional live-target -> end-frame FireFly assertion confuses the firing action with the later moving pose selection.

Root's g7 completed1pass/1fail: parked Rocketeer passes; the flight test fails its retained end-frame FireFly assertion with phase3, fixed speed fraction6553, Doing23/Hover, altitude500 and a living target. At this note's freeze r4fire expected-red session28307 is still active, with the assertions retained. The diagnostic records actual `SimFrameOutput.fire_events`, old/new rearm, firing latch, target and pose. The **specific discharge remains pending that diagnostic**; timer-change false-positive was not established or assumed. The as-run source is preserved below.

## Original call chain (explicit `gamemd.exe`)

- `51BAB0` whole Infantry AI suffix: `51BF59` calls `5206B0` FireAtTarget; after Object+90 gates, `51BF6A` calls sequencer `520AE0`, then `51BF7B` calls movement actions `520F40`.
- At the admitted fire frame, `5209DE` queries live GetFireError; numeric0 at `5209E4..5209E6` reaches virtual+3CC call `5209F5`. Infantry vtable base `7EB058`, slot address `7EB424`, original DWORD bytes `60df5100` -> **51DF60**.
- `51DF70` writes byte0 to Infantry+68D, **before** `51DF77` calls Techno FireAt `6FDD50`. The clear/call bytes are `c6868d06000000e8d4fd1d00`. A successful shot and a launch subsequently refused by Techno both clear this latch; a timer or pose alone is not a universal shot witness.
- `521161` asks active ILocomotion+A8. Exact Jumpjet ILocomotion vtable `7ECD68` has+A8 at `7ECE10`, bytes `504c4b00` -> **4B4C50**. That base receiver calls ILocomotion+80 at `4B4C57`; slot+80 at `7ECDE8`, bytes `d0d05400` -> **54D0D0**. This corrects the earlier shorthand that equated the+A8 slot directly with54D0D0.
- `54D0D0..54D0E9` reads phase at interface+4C (complete Jumpjet object+50); only phase0/2 return false. Phase3 therefore enters the moving branch independently of the separate Moving byte.
- `52116F..521226` requires type JumpJet+D94 and actual Jumpjet class GUID. `521228..521230` retains the action when+68D is nonzero. With it clear, `521236..521247` compares Foot+578 against original double `7EB5C8`, bytes `9a9999999999e93f` =0.8. Ordered speed above0.8 requests Fly24; at/below0.8 requests unforced Hover23 at `521267/52126B`.
- The original FireFly action record26 at `7EAF7C +26*4 =7EAFE4` is `01010001`: interruptible byte0=1, so the unforced Hover request can replace it. DoAction owns admission and the resulting Doing/stage/timer.
- Stationary `5212AF..521313` only maps current Walk3/Fly24/Hover23 -> Ready, Crawl6 -> Prone, Swim17 -> Tread. Current FireFly26 is skipped. This is why the earlier held phase2 shot could retain FireFly while the approaching phase3 shot legitimately ends Hover.

## Saved executable controls, zero-based identities

`tools/spatial_oracle/jumpjet_infantry_actions.json` has531 rows. The existing producer uses original Jumpjet queries and whole movement520F40 with Guard/NavComNULL to exclude its preceding destination recovery, original DoAction, supplied Rocketeer sequence strings transported through original ReadSequenceData523D00, FPCW0E7F, flat controlled world/height500 and Health125. These are selected action bodies; neither whole retail object/type loading nor whole Infantry shot/AI is claimed.

- Row376: `{"input":{"doing":26,"fraction":0.0,"kind":"movement","owner":{"moving":true,"phase":3}},"output":{"doing":23,"recorded":[],"stage":0,"timer":[1000,3,3]}}`.

- Row383: `{"input":{"doing":26,"fraction":0.8,"kind":"movement","owner":{"moving":true,"phase":3}},"output":{"doing":23,"recorded":[],"stage":0,"timer":[1000,3,3]}}`.

- Row483: `{"input":{"doing":26,"firing":false,"fraction":1.0,"kind":"movement","owner":{"moving":true,"phase":3}},"output":{"doing":24,"recorded":[],"stage":0,"timer":[1000,1,1]}}`.

- Row484: `{"input":{"doing":26,"firing":true,"fraction":1.0,"kind":"movement","owner":{"moving":true,"phase":3}},"output":{"doing":26,"recorded":[],"stage":0,"timer":[17,91,92]}}`.

Rows376/383 directly prove moving phase3 + currentFireFly26 + default firingfalse at fractions0/0.8 -> Hover23/stage0/timer[1000,3,3]. Row484 retainsFireFly26 and timer[17,91,92] with firingtrue; row483 contrasts clear latch at fraction1 ->Fly24. No interpolated native outputs were created. The tested6553/65536 fraction is far below0.8; the documented 2^-16 truncation residual near0.8 in the existing Rust comparator does not decide this branch.

`infantry_ai_order.{py,json,meta.json}` separately contains22 whole51BAB0 caller-order controls invoked from original Logic loop55B5FF..55B61B. FootAI, FireAtTarget, sequencer and movement-action entries are observed return substitutions. That corpus establishes order/gates, **not** a complete discharge. Together with the raw FireAt/latch instructions and executed action rows it explains the allowed pose; it is not relabeled as new whole-shot parity.

## Current Rust consumers and intended fixture correction

`movement/infantry_action.rs:397` owns the Infantry post-Process action turn; Fire (`:436`) precedes sequencer and movement (`:454`). `world/object_turn.rs:1108` calls that owner in the live Infantry object visit. `combat/world_receiver.rs:3460` is existing `emit_admitted_fire`; Infantry latch clear is `:3472`. `movement/infantry_action.rs:718–725` reads the live latch and requests Fly/Hover through existing DoAction. `movement/motion_query.rs:71` owns+A8 composition; `movement/locomotor_ready.rs:97` owns the distinct Jumpjet+80 phase predicate. `rules/infantry_sequence.rs:205` supplies the one native action-record owner. No copied pose decision or production change is needed.

`movement/infantry_action_tests.rs:162` (`jumpjet_infantry_actions_match_the_native_bodies`) consumes the saved native rows. It compares497 controls, records30 intentional near0.8 truncation cases, and excludes4 walker firing-arm controls here; those belong to the existing ground firing corpus. This note did not rerun it. `combat/infantry_fire_facing_tests.rs:237` covers the distinct fire-frame refusal ->Hover path; it is not proof of this successful discharge.

Root will use actual discharge events and assert the native final pose by moving-now/latch ordering, retaining pending FireFly observation, range and maximum firing-speed checks, subsequent kill and return-to-hover checks. Final phase2 retains FireFly; moving phase3/latch-clear/low speed ends Hover. Preserve these distinct observation times instead of fitting an after-state or weakening admission. Root owns the fixture change/validation; r4fire results are pending at freeze.

## Original static byte receipts

The SHA-pinned image was read through existing `native_oracle.image_bytes/file_span`; this performs no emulation. Explicit Ghidra reads also targeted `gamemd.exe`, x86LE32 at image base00400000; metadata path is `/C:/Users/enok/Documents/Command and Conquer Red Alert II/gamemd.exe`. Original image SHA256: `1cdd1180e49024fbda8ad568caac2e86e856063ff67ab38f62b7d2c7bb84298c`. Ranges below are half-open; each SHA is exact original file bytes.

| Role | VA span | Bytes | SHA256 |
|---|---|---:|---|
| Infantry caller suffix | `0051BF57..0051BF80` | 41 | `fb915e942a7045f84dceb8a2f27a99ad40634cd3e4e964d6b8a9d6848e5ba1ba` |
| Admitted fire-frame call | `005209E4..005209FD` | 25 | `b5b0f6afd859b9770158e22cd85bd56fe53daf13652e733249ea090e9887f4d6` |
| Infantry FireAt through true RET8 | `0051DF60..0051DFE1` | 129 | `d63128af39d9b37105263addcf8eef39662cb06cc08776e1a367152865dbde2b` |
| Base+A8 delegation through true RET4 | `004B4C50..004B4C60` | 16 | `57e0540ee4996ad1c7d2dacf3f00aac2c2e2c216e12b9a16f368f8dec6fd4b26` |
| Jumpjet+80 query through true RET4 | `0054D0D0..0054D0EA` | 26 | `c5178893a6df55a40ac4f19ad2186af3e700b6e9902d0e46f02926f842dd9743` |
| Moving Jumpjet action arm | `00521144..00521277` | 307 | `f28d429290ac67a2c086f2edd8cf69b7c4f07c619fde289b3207a1757c4aa81f` |
| Stationary action arm/true RET | `005212AF..00521313` | 100 | `6ac3a783130e5527c1c704f3af06899cc7e4fa3a228ef858d6e1e9955dd0c8db` |

## Source/payload pins

Hashes are raw SHA256 at the inspected/frozen source, not claims of fresh replay. Existing corpus metadata supplies its historical execution/provenance limits; the current producer SHA is independently recorded.

- `src/sim/world/jumpjet_infantry_tests.rs`: `18b6f4e678d5d4577d118bae4250a906ae54df809372c6396d303d79a24d4c21`.
- `src/sim/world/object_turn.rs`: `60b6862cca7fdde0a7b27aad36e8a96dab723eda636535b95e07e3f72b157eef`.
- `src/sim/movement/infantry_action.rs`: `60d523a952be6c6f0e85c5c057e183f9d694f60ec5230ede58e1fd9f1b79ffbc`.
- `src/sim/movement/infantry_action_tests.rs`: `a9b043eb491eeb15cb9f6a974ef7ab6d3f7064a1789484e3c409aed4a379d37d`.
- `src/sim/movement/motion_query.rs`: `a875dbcd14942be4b01ede576ea3eb4c45b5ef3730bf8457124c0e35e494d710`.
- `src/sim/movement/locomotor_ready.rs`: `3a4fd31da474ff719b0ee6ee379822475b5e1a5189618b63eb738f6e2d8adc33`.
- `src/sim/combat/world_receiver.rs`: `8e019684e6f73b59d50a2be6483fd89bc11c1903f21b9c350d09f077395c091c`.
- `src/sim/game_entity/gunner.rs`: `954397a2624a3545d789871c96e2255bc47a3add13bce32538e9cc57d20886c7`.
- `src/rules/infantry_sequence.rs`: `8dfa09b13f9e2ab0ca85ba98396e474c2df2466acedaeb9ade5b6468dac0f8cc`.
- `tools/native_oracle.py`: `141fc5f3981b40d41869643c0350e3988e40be2e470cba8a0b4a77bb7df39b27`.
- `tools/spatial_oracle/jumpjet_infantry_actions.py`: `3d250ec779fa8bf9bfc99cad524ed3a9b471e2f3444150bec37a449233da20d6`.
- `tools/spatial_oracle/jumpjet_infantry_actions.json`: `0e02d307e45fc49054ccf0f3e9a774ddd383b5dee4b3400d69c5816e3945d724`.
- `tools/spatial_oracle/jumpjet_infantry_actions.meta.json`: `e11dd8be155fea0f23b8f358b3429615dff1a37f64aa975a54ad7485aa896fa5`.
- `tools/spatial_oracle/infantry_ai_order.py`: `451d89c2442a382f7b2cfeb652b6fee105cbe171f384e07e9d68f584e237cab4`.
- `tools/spatial_oracle/infantry_ai_order.json`: `cdf8a6505d05f78f97086af9f4d34070b29d98cddbe663c94db16fd1bee52d29`.
- `tools/spatial_oracle/infantry_ai_order.meta.json`: `ab755e9255d8d28b341d1553d7468c53b7a18e22f8ddd07f23fa161968f50b7f`.
- Preserved Root diagnostic `/Users/halvor/Documents/vera20k-dev/refactor-goal/movement-target-689-rocketeer-r4fire-as-run.rs.gz`: gzip `c4663f0008361f62242e94c547afe89f7d6d9357f5bc9812e99f7e47c7356ef5`; decoded Rust `3ab19ce869f775c8810808c3b0356fd9b25480ff605d09a996e810869cfe4af9`.
