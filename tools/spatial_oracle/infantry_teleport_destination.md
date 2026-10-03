# Infantry Teleport Cell destination ownership

The ordinary CLEG Cell setter is `Infantry51AA40 -> Foot4D94B0 ->
Teleport718100 -> resolver718B70 -> Cell481180 -> Infantry51BF90`. The
comparison executes the original bodies through their actual returns, including
StopDriver51DAF0, DoAction51D6F0 and the destructive raw receivers5217C0/521850.

```sh
PYTHONPATH=. python -m tools.spatial_oracle.infantry_scatter_destination --teleport-cell --check
```

`infantry_teleport_destination.json` records 22 cases and 46 command boundaries
before Process. Its sidecar records the original binary, body spans, startup,
fixture boundaries and source bindings. The binary SHA256 is
`1cdd1180e49024fbda8ad568caac2e86e856063ff67ab38f62b7d2c7bb84298c`.
The existing Walk18, default-motion54, CMIN50 and Jumpjet531 corpora retain their
original values.

The fixture uses the existing Mission reader controller to execute the original
CLEG type constructor5236A0 and whole reader5240A0 on physical RULESMD, absent
LANGRULE, Battle and AnyTown layers, with fixed ART. JumpJet is the byte at+D94;
MovementZone is the dword at+5B4. The reader establishes false/Infantry7,
SpeedTypeFoot0, Strength125, Crawls1, Fraidycat0 and the Teleport GUID. All42
ClegSequence records come from the original523D00 reader. House50B730 executes
on explicitly recorded mode/house flags. These controls retain the existing
asset and OS boundaries; they do not execute the whole scenario loader or
Infantry construction/unlimbo path.

The already-live command fixture separately supplies Rules+1768=22 and a
nonzero land0 speed row (nine original float entries of1.0 at89EA40). These
are declared fixture inputs, not values read from retail GeneralRules. The
Rust boundary comparison supplies the same inputs; production keeps its
retail blocked delay60. The unchanged expected boundary values caught both
missing fixture inputs before validation passed.

The selected setter preserves these responsibilities and ordering:

1. Human Doing27..30 returns before mutation. Otherwise51AB65 asks the active
   locomotor's IsMoving. Its physical Cell4834A0 query precedes the current
   Attack/same-reference exception and StopDriver. Queued Attack is not current.
   Human same-reference prone Up7 runs through the existing DoAction owner.
2. Foot owns NavCom publication, admission and accepted timers. MoveTo calls
   the resolver, which releases retained+28 XYZ, or physical XYZ when+28 is NULL.
   Stop718230 clears armed+1C and request+34 but retains+28 and its reservation.
3. The resolver selects deck only for structural100 and owner Z strictly above
   floor+3*104, independently of OnBridge. Original startup establishes104;
   equality312 selects ground and313 selects deck on these flat cells.
4. Ready521B60 and optional Commence5B3570 precede placement. The existing
   Mission authority promotes a queued mission at this call position. The
   original priority corridor needs an object NavCom; pure Cell requests use0.
5. `bump_crush::place_infantry_in_native_cell` is the shared481180 selection
   owner used by Teleport, Walk and Jumpjet. It preserves preference, priority,
   vehicle/ground-Gate guards, the centre-row Scenario draw and scan order.
   `walk_head::selected_head` owns packed XY and successful output Z. Ground
   is sampled at the original incoming point, after successful selection.
6. InfantryCanEnter reads the resolved point. Success reserves that point;
   refusal restores physical raw occupation before the recursive class NULL
   setter. Full-slot refusal draws once; vehicle refusal draws nothing. Foot's
   timer tail follows either answer. No FNPC call executes in these controls.

`TeleportRuntime` privately owns resolved XYZ, request and the retained warp
adapter in the complete locomotor payload. Active queries and Process read the
active instance. The Foot effect view can also read a suspended instance;
BEGIN/END, snapshots and hashing retain that same payload, with no entity copy
or cell-centre destination mirror. Snapshot286 changes this layout. The child
`teleport_cell_destination_tests` compares native states and all three full RNG
objects after each boundary; its save/load, hash, suspension and command-route
checks are separately identified Rust regressions.

The three bridge/global/retask Rust replay hash ratchets change solely because
the entity-level absent Teleport tag is removed. An isolated control restored
only that old zero tag and recovered all three prior hashes exactly; their
pose/gameplay checks and absolute RNG pins were unchanged. The control was
removed before the final candidate. These constants are Rust hash regressions,
not native gameplay goldens. Legacy supplied warp-effect fixtures retain their
active class and keep the effect in a suspended instance, rather than replacing
the class whose original body the fixture compares.

Coverage remains bounded. Unit/CMIN keeps its prior centre-resolver adapter;
object/FNPC destinations and Chronosphere need their separate class admission,
reservation and cleanup chains. The existing Relocate/ChronoDelay Process and
cell-distance timing adapter remains, including its prior timer policy; this
corpus executes no7192F0 Process and cannot certify that complete lifetime or
the unrepresented secondary Process byte. OpenTopped rider relocation and
incoming-bullet detach remain documented at that owner. Those triggers affect
superweapon warps or in-flight attacks, rather than the selected pre-Process
Cell requests. Legacy placement callers still supplying no ground-Gate context
can refuse an open Gate before RNG; issue683 retains that separate caller
migration. The shared selection cutover preserves their existing supplied
context and existing native occupation comparisons.


## Production integration and validation

The reproducible [CLEG Cell profile](../map_observation.cleg-cell-destination.example.json)
uses stock AnyTown/Battle and the ordinary human production chain. America builds
AMRADR (retail GAAIRC forbids Americans), then GATECH at `(24,89)`, clear of the
construction yard's fixed ART 4×4 foundation at `(27,87)`. The loaded registry and
production captures establish CLEG's type handle 142 and object 1635 for this seed.
Different rules or startup inputs require rediscovering those handles.

The final ordinary release observation runs 10000 exact steps and records 56403
samples under the unchanged 100000 sample limit. CLEG is created at 8001 and becomes
live at 9135. Two ordinary Cell orders execute at 9500, first `(34,94)`, then `(35,94)`.
At 9501 the latter NavCom is installed while physical XYZ is still
`[8384,23616,416]`; at 9502 the unit is at selected XYZ `[9152,24256,416]`, with
NavCom cleared. It returns to Guard at 9518 and remains at that point through 10000.
The inspected 800×600 GPU readback shows the unit at the destination. This is Rust
production integration, separate from the original pre-Process comparison above.

Release executable SHA256 is
`86a61e191e13ce41f757239d224254c96a435868f294d2c3d3d3c0de2b676cc7`.
The profile SHA256 is
`b2add10d8c9145038873dadb5c5fc948f35892f873e36d169927d6e6e13c3486`;
retained capture/run SHA256 values are
`2e032dce6b96f09841bbd8c02274b50a6c5928c20cfed516756237d119d938c5` and
`324378d2c7baf983de2fbf36835b8eea11bb0a0d1818fbb060e09e9eb744ccaf`.
Final BGRA SHA256 is
`c692b9ac59cd7e8766c730ff229643299cc8d38a052769d31c429d1a8fc713d1`.
The capture passed the ordinary wrapper and offline validation against the actual
release binary. Its external `cleg-owner-release-runtime-summary-20261003.json`
retains source/input identities, selected boundaries, artifact hashes and limits.

Final required-retail library validation passes 9545 tests with 0 failures and 227
intentional ignored tests; Clippy passes. A separate exact 63-test retail movement
and integration sweep passes without fixture skips on the same simulation source.
The Python observation validator passes 82 tests. The optional exact type filter is
implemented in the existing observer so this long ordinary tech-tree path fits its
fixed budget; no simulation or RNG behavior changes with filtering. Earlier failed
or incomplete profiles remain retained and are not relabelled as runtime passes.
