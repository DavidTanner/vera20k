# Stock smudges in the ordinary GAPOWR fatal continuation

The `--joined-stock-smudges` mode extends the existing
`building_death_anims.py` owner. It executes three ready-MTNK/105mm/AP fatal
routes with Scenario/Main seeds 2, 1 and 31 and MapGen seed 31. The inherited
primitive, four-route joined and limbo-cancel controls remain separate unchanged
payloads. This comparison establishes the selected stock smudge prerequisite,
its transient lifecycle and the complete subsequent footprint, crew and drain
continuation; it does not establish whole-object or whole-game parity.

The accepted original executable SHA256 is
`1cdd1180e49024fbda8ad568caac2e86e856063ff67ab38f62b7d2c7bb84298c`.
The sidecar pins the executable, original-code section identities, Python/Unicorn
environment, existing fixture owners and the additive source. No native result
is generated from Rust.

With the selected extracted retail files available, replay from the checkout:

```sh
VERA20K_GAMEMD_EXE=/path/to/gamemd.exe \
VERA20K_BUILDING_DEATH_ASSETS=/path/to/selected-retail-extract \
python -m tools.spatial_oracle.building_death_anims --joined-stock-smudges --check
```

The materialized input directory retains the ordinary joined files and extraction
receipts, physical `TEMPERATMD.INI`, `CLEAR01.TEM`, and the 24 physical flagged
`BURNT01..12.TEM` / `CRATER01..12.TEM` images. File identity and requests are in
the payload and sidecar. An explicit `--output` can select another reference.

The original stock registry CRT `0x004E7260`, active Rules::Process list block
`0x00668DD2..0x00668E23`, and Rules::ReadTypeData loop
`0x00679BE2..0x00679C06` execute against selected physical reader caches from
RULESMD, optional LANGRULE, Battle mode and the inner Hills map. The unused named
ReadSmudgeTypes wrapper `0x006727D0` is not the active list producer. Whole
SmudgeType constructors `0x006B5260` and ReadINI `0x006B56D0` execute. Width,
height and Burn/Crater come from rules; ObjectType's Theater read comes from
physical ARTMD. ART and rules use distinct native caches. The list has 46 types,
including 12 flagged Burn and 12 flagged Crater types. Legacy CR/BURN names are
unflagged and legitimately have no requested SHP images. Physical SmudgeTypes
key/value declarations are retained as ordered arrays in
`native_stock_smudge_inputs.layers[*].physical_smudge_registry_entries`;
dictionary lexical order must not reconstruct their declaration order.

The legal flat ClearTile0 prior executes the original IsoTileType registry CRT
`0x004E76E0`, constructor `0x005447C0`, and physical TEMPERATMD TileSet0000
Morphable Boolean reader `0x00546124..0x0054614C` with its original result writer
`0x0054644C..0x00546452`. The reader default is false; the physical value produces
true. Admitted interior cells have TileIndex0, slope0, no overlay and initial
SmudgeIndex-1. Full theater/TMP/map loading is excluded. All shared class/height,
connectivity, navigation and gameplay owners retain the joined fixture.

Original Smudge runtime registry CRT `0x004E69E0` and sentinel `0x006B5210`
execute. Selected placers, CanPlace, constructor `0x006B4A50`, Mark
`0x006B4BE0`, Place `0x006B6080`, UnInit `0x005F65F0`, expiry `0x007258D0`,
deferred scalar `0x006B4FA0` and Object destructor `0x005F3B80` execute unchanged.
The RTTI getter `0x006B4F40` returns 29. No type, placement, lifecycle result or
RNG pick is supplied.

| Scenario/Main seed | Selected type | Smudge native ID | Final unique-ID cursor | RNG requests | Crew |
| --- | --- | --- | --- | --- | --- |
| 2 | CRATER11, registry index44 | 14 | 21 | 78 | E1 ID19, HP80 |
| 1 | CRATER11, registry index44 | 14 | 21 | 75 | none |
| 31 | BURNT11, registry index32 | 14 | 20 | 75 | none |

The actual Crater/Burn pick is `RandomRanged(0,1)=0`, from return PC
`0x006B5ECE` / `0x006B5BDE`. That reached pick precedes footprint and crew RNG.
Constructor admission consumes unique ID14, advancing the cursor13 to14 and
the ordered pending queue `[7,8]` to `[7,8,14]`. Before the frame drain, the
queue is `[7,8,14,1,4]`. The Smudge joins the native Smudge and generic Object
registries and never joins Logic.

Each expiry boundary retains signed Health255 and raw Location `[2688,2688,0]`,
AbstractFlags2, marked0 and Logic98=0. The first two expiry return PCs
`0x005F661B` and `0x005F5316` see alive1/limbo0; deferred scalar return PC
`0x006B4FC6` sees alive0/limbo1. All pass control1. The scalar owner has already
popped ID14 when its third expiry sees remaining pending IDs `[1,4]`. Constructor
return is alive0/limbo1 because native Mark immediately self-UnInits the object.
The original Unit/Foot/Techno callbacks execute; all three full RNG streams and
the MTNK's target/passive-scan timer remain unchanged across each Smudge expiry.
Ordered queue members, IDs, generic/Smudge/Logic membership, flags, Location,
Health, cell writes and complete RNG states are in `stock_runtime` snapshots.

Native Place writes four persistent cells `(10,10),(11,10),(10,11),(11,11)` with
data `0,1,2,3`. After drain, the pending and transient Smudge registries are empty
and these cells still retain their type/data. Object retirement must preserve
the persistent marks.

The fatal Building call is Mark0, return PC `0x005F4D73`, while the Building is
still alive1/limbo0/Health0. Mark0 dispatches `0x0043F1CA` to MapPlaceUp; it leaves
the four marks unchanged. Ordinary Building PUT (Mark1/3) has a separate
whole-footprint clearing writer `0x0043F8A6`; the ToTile arm writes at
`0x0043F3A1`. Neither writer executes in these post-effect fatal routes. The
comparison does not identify a defect in a legitimate placement-time clearing
owner.

The inherited empty-registry joined route suppresses this real pick and
constructor lifecycle. Its seed2 crew is ID18/HP69 and its final cursor is20;
the stock route instead produces ID19/HP80. The primitive control records the
Smudge constructor and executes Place separately, so it cannot establish native
ID allocation, registration, callbacks or deferred retirement. A production
grid write without the shared native-ID and retirement owners is incomplete for
this reached stock path.

The physical legacy header sections are empty. The sealed cache retains their
empty maps. An additional original section-omitted cache control preserves all
46 declarations even when 22 ReadINI calls return AL0. Constructor defaults are
Width1 (`0x006B5281`), Height1 (`0x006B5287`), Crater0 (`0x006B528D`), Burn0
(`0x006B5294`), Image equal to the type ID (ObjectType copy
`0x005F726F..0x005F7276`), image pointer0 (`0x005F70C7`) and Theater0
(`0x005F7176`). Registry append/index ownership is `0x006B5327..0x006B5361`.
ReadTypeData's virtual reader call `0x00679BF9` continues at `0x00679BFC`
without deleting a false-returning declaration. The additional control does not
establish native physical disk-loader handling of empty headers.

The actual constructor Mark1 re-runs CanPlace with force/allowBuilding1
(`0x006B4C31`, return PC `0x006B4C36`), then calls Place (`0x006B4C45`). These
three routes reach the check rather than its priority/specific-flag bypasses.
No RNG or persistent smudge-cell mutation occurs between the pick and Place:
the pick's Scenario state equals constructor entry, all three streams remain
unchanged through Place entry, and the first smudge write is inside Place.

Coverage retains all inherited fixture assumptions in the sidecar, including
already-admitted Houses/building, the ready shooter, flat navigation plane and
bounded House prefix. Earlier volleys, complete physical map/theater loading,
other stock/nonflat placement routes, final pixels and audible output are not
certified. Native replay and Rust production parity are separate validation
claims. The joined continuation ends at impact frame4, original frame increment
to5 and that frame's deferred drain. Later visits of surviving death Anims are
not exhaustively executed by this mode. No Rust validation was performed by
this prerequisite audit.
