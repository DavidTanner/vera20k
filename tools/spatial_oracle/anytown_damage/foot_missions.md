# Selected Foot Rescue and AreaGuard native continuation

Run the existing mission owner with the separate mode:

```sh
source /Users/halvor/Documents/vera20k-dev/env.sh
VERA20K_SHRAPNEL_INPUTS=/path/to/physical/shrapnel/extract \
VERA20K_ANYTOWN_INPUTS=/path/to/physical/anytown/extract \
python -m tools.spatial_oracle.anytown_damage.mission --foot-missions --check
```

The input roots follow [the existing mission owner](mission.md) and
[the Anytown attack setup](README.md). They must contain its physical INI, map, TMP and
attack SHP dependencies, plus the physical GI SHP used by the Infantry reader.
The native image is SHA256
`1cdd1180e49024fbda8ad568caac2e86e856063ff67ab38f62b7d2c7bb84298c`.
`--write` deliberately creates a new reference; it never derives outputs from
Rust. The default mission, counter, interleaving and projected native corpora
retain their bytes.

[foot_missions.py](foot_missions.py) extends `Mission` in
[mission.py](mission.py). It reuses that owner, its physical map/retail readers,
allocator and runtime boundaries. It executes original MissionControl static
construction and layered mission reads, `Rules66D150` for `[ElevationModel]`,
actual E1 type and selected weapon/child reads, original Unit/Infantry/Foot/Techno
constructors and actual Drive/Walk factories. The four actors undergo original
Unlimbo, map marking and Display/Logic registration. The preserved208 rows use explicit `51D6F0` Ready as their fixture baseline.
The additive initialization receipt executes before that explicit call and records
the native constructor/Unlimbo `Doing=-1` and original sequencer transition to Ready. Houses are explicit supplied storage and links with valid Unit
and Infantry counter vectors; full House construction is excluded.

The selected handlers use the original concrete vtables. Unit `744100` and
Infantry `51F640` call Foot `4D6AA0`. Both concrete Rescue virtuals dispatch to
`4DDF90`. Several rows enter original `5B3060`, so their timer epilogues also
execute. The full `.text` and original vtable bytes must remain unchanged.

The JSON schema retains `setup`, physical `inputs`/`world`/`mission_inputs`, constructor
and placement results, and the original82 `rows` with unchanged values. It adds
`rules_reader_receipts`,36 `retail_idle_rows`, `greatest_threat_registration` and
86 `greatest_threat_rows`. Separate additions are `initial_action_receipt`, two
`navigation_rows` and two `empty_rescue_rows`. The additive `weapon_reader_receipts`
and12 `retail_weapon_rows` preserve all208 earlier row values, including their RNG.
Each row has explicit `input`, actor `before`
and `after`, raw `returned_eax`, full three-stream `rng_before`/`rng_after`, ordered
`events`, field `writes`, inherited runtime `callback_events`, native `stop` and
`returned`. `instruction` numbers place events and writes in the same original
instruction order. `physical_getcoords` records actual +48 dispatch results;
`xyz_cell`/`cell_lookup` retain actual Map query arguments. `math` records native
range/rate/distance conversions. `chosen_leash` records the post-flag override.
`score_before_vhp` and `evaluate.score_result` are native integer scores; raw EAX
at an x87 or void return is only register data. Raw middle timer words are copied
stack data, not a simulation timer authority.

The 82 rows cover ordinary MTNK and E1 Rescue state2 cadence; state1 with and without
NavCom; direct Commence to AreaGuard followed by its new current mission rate;
existing-target Approach(false); real archive-point target scans and empty
scans; current Cell ground anchors while the actor stands deck+416; archived
entity ground/deck points; different retained locomotor head/unrelated NavCom;
scan-latch+688 reset; zero versus nearly zero coordinate sentinel; strict
1.5-range admission; AreaGuard default post, passive scan, approach, idle/cadence,
1.1 Cell leash, Foot-anchor GuardModeStray and firing/NavCom leash bypasses.
Scenario seeds0/1/31 are explicit. Raw and ranged RNG calls and final state are
recorded, including the Infantry idle draws that precede AreaGuard cadence.

Full scanner rows run original concrete wrappers, `4D9920`, `6F8DF0`,
`6F7CA0` and `70CD10` against the actually placed hostile MTNK. The separate
`supplied_scanner_return` rows stop before virtual call `4DE056`, record its
original receiver/vtable and three pushed arguments, then supply only a declared
scanner result and stack return protocol. The original caller `4DE05C` executes
its coordinates, distance, strict comparison and concrete setter unchanged.
These rows establish the caller gate, not acquisition. Every no-target/refused Rescue row in the preserved82 stops at `4DE12D`,
before the status1 write, `House500200`, home selection, destination/archive cleanup
and tail RNG. The two additive `empty_rescue_rows` execute that suffix under the
explicit House and crop premises below.

Native `4CAC40` converts the squared distance to a float and indexes table
`8650BC`; it is an approximate square root. Raw axis distance513 can become512,
2817 becomes2816, and2253 becomes2252. The corpus therefore includes several
consecutive geometric inputs across each actual leash boundary. The comparison
uses the native converted distance, not an integer square-root reconstruction.
Native `4D6E74..4D6E89` substitutes retail `[General]GuardModeStray=512` for the
1.1 range when the archived object has Foot flag14 bit2. The original reader is
`670EBD..670ED7`, using the distance parser `474620`.

For the old supplied weapon controls, the Cell leash first issues a destination at
2818 for MTNK and2254 for E1; the Foot-anchor leash first does so at514.
The original target clear precedes the destination setter. Rescue state1 with
no NavCom directly Commences AreaGuard and then reads its new rate: seed31 gives
36, versus15 when Rescue remains current. E1's AreaGuard post also gives36 after
native idle RNG draws; MTNK's post gives39. The original82 rows retain constructor
IdleActionFrequency (`736891ed7c3fb53f`, little-endian double bits): their input
receipt contained the AudioVisual key but the original idle key block had not
executed. Their idle timer values are bounded by that constructor frequency. These values are native row results,
bounded by the explicit seeds and state inputs.

The208 historical rows have a partial E1 weapon read context. Original TypeINI
allocates physical `Primary=M60`, `Secondary=Para`, `ElitePrimary=M60E` and
`EliteSecondary=ParaE`, but only M60 receives WeaponType `772080`. The M60 child
is `InvisibleLow`; its original `46BEE0` attempt returns AL0 because the fixture
selected `[Invisible]` instead. `InvisibleLow` therefore retains actual native
constructor fields: Inviso=false, AA=false, AG=true, image name `InvisibleLow`,
and false SubjectToCliffs/Elevation/Walls. SA receives the full physical Warhead
reader. Para, M60E and ParaE retain range0, damage0, ROF0, Burst1, Speed0 and null
projectile/warhead references. The lexical M60E section was cached but unread.
These old values are supplied reader premises, not full retail weapon behavior.

The new `weapon_reader_receipts` records these before fields and original slot
getters `7177C0/7177E0`. `7012C0` and `707E60` execute through original `70E140`
and retained concrete vtables. Before closure, rookie weapon ranges are1024/0
and threat modes0/1/2 return0/2048/2048. A separate supplied float2.0 veterancy
control selects the existing elite slots: ranges0/0, modes0/0/1792. That control
restores veterancy afterward and consumes no RNG; it does not prove veterancy
production. Names and numeric fields are semantic observations; pointer values
remain native fixture identities.

The receipt's `omitted_sections` names Para, M60E, ParaE, InvisibleLow and SSA.
The new closure runs actual `772080`, each original referenced `46BEE0/75D3A0`,
and `7729F0` across the declared physical layers. Exact section strings, line
numbers, read AL status, native reader calls, field writes and unchanged full RNG
states are saved. Afterward the rookie ranges are1024/1280, elite ranges1024/1536,
and threat modes are0/2560/2560 or0/3072/3072 respectively. InvisibleLow now reads
Inviso=yes, Image=none and all three SubjectTo flags. SA and SSA keep their own
native Verses and ProneDamage bits. The selected pass order is the existing
Anytown weapon/child/postpass convention; this is not a whole Rules Process,
registry initialization or weapon firing proof.

The12 `retail_weapon_rows` run after that closure and the physical idle-frequency
read. Seven original AreaGuard rows pin the2254 contrast and consecutive real
Cell-leash boundaries:2254 now returns36 without a destination,2817 still stays,
and2818 takes the original destination, moving/idle refusal and39 cadence. Three
Rescue rows pin the strict 1.5-range supplied-result gate near3840; those retain
their declared scanner-result seam. Two separate rows execute the actual Rescue
scanner prefix with only the two extra registered candidates explicitly supplied
limbo1/alive0 after prototype restoration. The original MTNK candidate uses the
declared `candidate_live` limbo control. These two rows reuse the existing
`navigation_control`, including its explicit prior path/queue/timer fields;
they stop before House home selection on an actual scanner miss. Actual outcomes,
field changes and RNG are native results;
the earlier2254=39 remains unchanged under its unloaded-Para premise.

Every new row names `input.reader_context=weapon_reader_receipts.after`,
`input.rules_context=handler_rules_after_physical` and
`input.registration_context=greatest_threat_registration.after`. Its
`registration_before` records all five actual registry arrays in native order.
`candidate_liveness_before` records each Techno entry's pointer, limbo/alive bytes,
physical XYZ and House immediately before entry. This avoids inferring liveness
from the label or an earlier outside-row write: the regular leash and supplied-gate
rows restore placed extra candidates limbo0/alive1, while the two real scanner
rows apply the declared inactive controls after that restore. Actual scanner
events retain `result_eax`; the miss prefix is not a full House continuation.

`reader_contexts` separately records AudioVisual chronology. The old82 `rows`
and `initial_action_receipt` use constructor IdleActionFrequency;36 idle,
86 threat, two destination, two home and12 new weapon rows use the later physical
read. `[AudioVisual]IdleActionFrequency` is the relevant key. A partial-reader
comparison must reproduce both the weapon omission and the array's AV context;
it must not change a native golden to make a full retail rules fixture fit.

The zero coordinate is established by original `6F2A50`. An archived point
`(0,0,0)` reaches `70D023` and uses scanner-to-target whole-cell distance;
`(0,0,1)` reaches `70CFBE` and uses archived-point-to-target leptonic distance.
Rescue still independently measures candidate versus archive +48 XYZ for its
strict 1.5-range gate. Default Cell anchors execute `486840/47B3A0` and retain
physical ground height/slope; an entity anchor executes `5F65A0` and retains its
physical XYZ. The retained locomotor head is not the archived point.


The additive reader receipt runs full original `665650` twice with declared
`GuardModeStray+1724` storage values0 and `0x12345678`. It records the same values
after construction and no overlapping native writes. The constructor explicitly
writes IdleActionFrequency at `667574/66757E`; it skips `+1724`. Zero in this
receipt is supplied storage, not a native allocation or constructor-default claim.
The unchanged General key block `670EBD..670EDD` calls `474620` with the current
field as default. Missing/wrong-case keys retain it, `2` reads512, `-1` is a
retain-current sentinel, `-2` reads-512, and a later missing layer retains-512.
These are executed parser controls.

Original `66B3E4..66B40B` reads AudioVisual `IdleActionFrequency` through `5283D0`.
The additive physical layer receipt reads actual `RULESMD.INI` `.15` to
`000000403333c33f`; missing later layers retain those widened-float bits.
GuardModeStray physically reads `2.0` to512. The key-block seam supplies ESI Rules,
EDI CCINI and stack state; it does not execute the whole AudioVisual `6691E0`
reader or native load/allocation sequence. Source bytes, line receipts, defaults,
reader arguments and writes are saved. The VM's observed FPCW is `0x0E7F`
(53-bit precision, chop rounding).

The36 additive idle rows use the physically read frequency. They execute E1
`51CDB0`, native `5216D0` admission, `7099E0` timer admission and actual Walk
`75AB30` through ILocomotion vtable+10. The real moving byte is interface+30
(concrete Walk base+34). Before/after-state records interface, vtable, getter, field pointers and byte
value in `walk_moving`. The separate Foot+3D5 control records that byte without calling it the
Walk moving authority. Ready0, Guard1 and Tread16 admit; Doing-1/4/9/27..30,
actual Walk moving, prone+6DB, firing+68D and an unexpired idle timer refuse in
these controls. Non-null target/NavCom and a supplied animation frame admit at
the receiver; the mission caller has its own gates.

Native `51D6F0` action, `5FB2E0` GameOptions delay and `4C9300` body facing run
unchanged. The rows retain actual action arguments/return, frame+F8, sequence
words+100/+104/+108/+10C, facing raw words+388..+39F, field writes and full three
RNG states. `infantry_idle.rng_at_return` records the state before the AreaGuard
cadence draw. Direct idle rows expose `returned_al`; other EAX bits remain raw.
At seed31, native idle writes258 and selects action9; AreaGuard then returns36.
Seed0 turns through an additional facing draw to raw BAM40960. Seed11 consumes
roll0 and performs no action or turn. Timer auxiliary words are raw stack values.
Stock physical E1 idle9/10 rows take no Main RNG/audio branch; excluded sound
sinks still prevent extending that observation to other sequences or types.

Two rows enter original `520AE0` to follow Idle1/Idle2 completion. They first
execute actual `51D6F0` action9/10 from the native Ready baseline and record
`completion_setup`: actual action return, before/after state, full RNG, real
GI sequence pointer/bytes and frame count. They then supply stage+F8 at that
count and run the full original sequencer through `520D1B` and its forced
DoAction(Ready0,1,0). Walk moving and prone are false. The original native
ordering, resulting Doing/frame/sequence timer/facing and RNG are captured.
These are sequence-end continuations; the absolute-stage clock and the producer
that advances stage to the count remain excluded.

The86 additive threat rows call original `4D9920` directly with literal masks
0/1/2 and separately call the original concrete `743190/51E140` wrappers with
those inputs. Concrete wrappers add their original weapon category bits, giving
Foot masks184/185/186 in these fixtures. With scan latch+688 set, Foot rewrites
only AL by clearing bit1 and setting bit0, so186 becomes185. The actual scanner
runs; no target or score result is supplied. A non-null result retains the latch;
a null result clears it. Literal Foot masks0/1/2 lack the concrete target category
bits here and produce misses even with a declared live MTNK. The requested,
Foot-entry and Techno-entry masks are recorded separately.

Archive/supplied points differ from the scanner's physical position. `(0,0,0)`
and `(0,0,1)` exercise the native zero-coordinate distance branches; latch-set
local scan controls can reject both distant points before any global-array visit.
The separate global scan rows record the score difference rather than claiming
that both scans have the same admission path. Ground/deck supplied archive points
retain their actual Z; no navigation-head or bridge-layer result replaces them.

The registration receipt observes native Techno, Unit and Infantry arrays after
actual constructors and Logic/ground Display arrays after actual Unlimbo.
Constructors append to the Techno registry. Logic appends for the supplied sort0
contract; ground Display has its own sorted order. Global scanner visits
`6F9C86` follow the actual Techno array. The two actual enemy MTNK candidates are
indices3 and5 in this fixture. Their supplied midpoint `(22656,13056,416)` yields
equal scores98570 for the MTNK scanner and98560 for E1. Native comparison at
`6F9D87` and `JLE` at `6F9D89` retain the earlier candidate; the second produces
no `global_best_update` event. This bounds the tie result to these actual scores
and registry inputs, not an invented whole-match construction order.

The extra enemy E1 exposes an old crop fixture boundary: empty cells had supplied
owner fields0, which native Infantry CanEnter treats as owned by House0. After
the preserved82 rows, the additive fixture executes original Cell `47BBF0` on a
donor and supplies only its native empty+54/+58 values to cells whose ground and
upper object heads are both null. This enables original enemy E1 Unlimbo; its
attempts, result, actual pose and registration are recorded. No full map/Cell
construction or arbitrary occupancy relocation is claimed. Extra candidate live
versus limbo bytes are explicit supplied controls after native placement.

The additive `initial_action_receipt` captures real E1 constructor `517A50` and
Walk `75AA90`, then successful `51DFF0` Unlimbo before this companion's explicit
Ready. Both retain `Doing=-1`; the actual Walk destination/head and moving byte
are zero. Full original `520AE0` executes with that initialized stationary actor
and native speed fraction0. It calls actual COM `75AB30` with the retained Walk
interface, obtains AL0, then requests `51D6F0(Ready0,force1,aux0)`. The sole Doing
write is native `51D9D2`; Doing becomes0, with all three full RNG streams unchanged.
Raw animation stage, sequence timer and body facing are recorded. Probe writes and
CPU/x87 are restored before the existing explicit Ready so every earlier row keeps
its exact value. This bounds initialization admission to the stated stationary
receiver; first whole InfantryAI, movement admission, absolute-stage clock and
whole ScenarioLoad remain excluded.

Two additive `navigation_rows` execute original AreaGuard's Foot-anchor stray
branch at supplied axis distance515. They retain the archived Foot reference as
NavCom and call actual Unit `741970` or Infantry `51AA40`, Foot `4D94B0` and Drive
`4AFD40` or Walk `75ACB0`. Both publish the archived Foot's native +4C XYZ to the
locomotor and original ILocomotion +10 reports moving. The original ctor head
remains NullCoord. Only the first path DWORD becomes-1; all23 suffix words and
reference cell remain. Unit clears NavQueue2 to0; Infantry retains2. Aux clears,
blocked becomes0, retry7 remains, accepted movement timer(start,duration) is(1,0)
and blocked timer is(1,60), using physical RepathDelay60. The copied timer middle
words remain raw stack data. New rows supply the nonempty path, externally owned
queue backing, reference/Aux/timers/blocked prestate; they do not validate those
producers. Setter entry/return full RNG states identify its effect independently
from the later AreaGuard(1,5) cadence draw and return39.

`destination_input_readback` retains the target Foot's physical XYZ, OnBridge,
head/destination and actual IsMoving, declared frame and physical RepathDelay.
Original `navigation_coordinate` events capture actual +4C output and requester
arguments. Native `xyz_cell`/`cell_lookup` events retain the returned Cell pointer,
packed XY, level/slope/flags/land, including the shared Dummy when selected.
Before/after navigation snapshots capture the real locomotor interface/vtable,
MoveTo/getter addresses, actual IsMoving, destination/head, NavCom/Aux, all24 path
words/reference, NavQueue header/backing/entries, timers, retry, blocked, animation
stage/sequence and facing. The original getter is executed only for readback; its
writes and CPU are restored. No geometry or moving decision is reconstructed.

Two additive `empty_rescue_rows` execute full original `4DDF90` with native empty
scanner results. They supply existing House storage fields primary cell(87,50) at
+5490, alternate(0,0) at+5494 and radius1024 at+5498, following the physical retained
field contract in [house_base_projection.py](../house_base_projection.py).
The original `4DE139` status1 write precedes unchanged House `500200`; `501AC0`
variant0, `50DEF0`, direction `49F420`, native map/zone `56D230` and FNPC `56DC20`
execute. For seed31, native ranged(0,1024) consumes2026076499 and returns339;
direction consumes2287577493; the selected Cell is(86,51). The concrete setter
publishes that Cell NavCom, then `70C610` clears ArchiveTarget. The final Rescue
rate draw(0,2) consumes2225548056 and returns0, yielding cadence14. Full three RNG
states and ordered query/output records retain that chronology. Final Dummy
coordinates/fields and every native Dummy write are recorded; no write occurs in
these selected in-crop home queries. House construction/base/radius producers and
whole navigation initialization remain excluded. FNPC uses the inherited supplied
crop zone planes; no home, FNPC or other gameplay result is supplied.

The four downstream rows run only after all204 historical rows have been captured.
The actually registered additional enemy E1/MTNK tie candidates and original enemy
candidate are explicitly supplied inactive with limbo+81=1 and alive+90=0 for
empty scans, after each row restores their original placement baselines. An
earlier diagnostic set limbo before calling the row runner; prototype restoration
reset the candidate to limbo0/alive1 before native evaluation, explaining its
selection. Original `6F7D90/6F7D96/6F7D98` rejects a nonzero candidate limbo byte;
the preserved Greatest_Threat miss controls agree. Both bytes remain declared and
recorded premises for these four downstream rows. This preserves original
registration history while supplying the no-target state. These rows do not
execute candidate UnInit, future locomotor Process, occupancy relocation or
bridge producers.

All rows start from declared prior mission state after original construction and
placement. Pose, bridge flags, OnBridge, retained heads, timer expiry and NullPoint
controls do not prove their producers, relocation/occupancy or native scenario
loading. Declaring the placed candidate limbo supplies an empty-scan state; it
does not execute UnInit. The existing cropped map, source-order INI cache,
successful heap and imported OS boundaries remain. Sound/radar/visual sinks and
the inherited hierarchy callback are declared in metadata and observed per row.
The mode does not run a complete UnitAI/FootAI tick, other match objects, bridge
placement/destruction/repair, later locomotor movement, rendering, audio Main RNG,
whole-world scheduling or persistence. Native replay alone is not a Rust or
production parity claim.
