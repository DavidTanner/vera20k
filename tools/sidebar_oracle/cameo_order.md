# Sidebar cameo ordering

`cameo_order.py` executes original `gamemd.exe` instructions through the
[shared checked runner](../native_oracle.md). It never computes expected
comparisons or ordered lists in Python. The JSON is a Rust-consumable corpus;
its `.meta.json` identifies the selected executable, source files, Unicorn
versions, scope and supplied boundaries. No executable or disassembly packet
is distributed.

From the checkout root, select a supported original executable and run:

```sh
VERA20K_GAMEMD_EXE=/absolute/path/gamemd.exe \
  python -B -m tools.sidebar_oracle.cameo_order --check
```

`RA2_DIR` is the alternative executable selector. `--write` explicitly
generates/replaces the JSON and sidecar; `--output` selects another path.
`--help` and imports do not read the executable. Original bytes and static
addresses can be inspected with `python -m tools.native_inspect`.

## Original comparator and retained owner

`StripClass::CompareItems` (`0x6A8420..0x6A870B`) receives
`(kind1, typeIndex1, kind2, typeIndex2)`. Its only caller is
`StripClass::InsertEntry` (`0x6A8710`, call at `0x6A8742`). The original
RTTI resolver `0x48DCD0` selects initialized arrays; their order is input to
the fixture. An empty right-hand kind returns true after those lookups.

For the ordinary type entries, native ordering compares:

1. Whether each `TechnoType +0x6D0` (`AIBasePlanningSide`) equals the current
   player's `HouseType +0xBC` side. The match comes first; two nonmatches
   continue without ordering their side numbers.
2. Movement groups. Only UnitType kind 40 and AircraftType kind 3 read
   `ConsideredAircraft +0xD96` and `Naval +0xCCE`. InfantryType kind 16 and
   BuildingType kind 7 do not use those flags for this comparison. With
   mutually exclusive flags, ground comes before considered-aircraft, which
   comes before naval. This is conditional native code, not a general sort
   rank: when both flags are set, an aircraft left operand precedes a naval
   right operand before comparing the remaining keys. The corpus includes
   all such pairs, including self-comparisons.
3. Signed `TechLevel +0x634`, ascending.
4. Each type's actual virtual `+0x84` `Cost_Of` for the current player,
   ascending. Both original virtual bodies execute: `0x711F00` and the
   BuildingType override `0x45EDD0`, using the shared
   [cost owner](../spatial_oracle/cost_of.py). Raw `Cost=` alone is insufficient;
   country, FactoryPlant, and BuildingType FreeUnit adjustments can change
   the order.
5. The localized UTF-16 string at AbstractType `+0x60`, compared by original
   `wcscmp` (`0x7CA5D3`). It compares unsigned 16-bit code units, case
   sensitively; this is neither locale collation nor Unicode scalar ordering.
   `SETLE AL` at `0x6A8703` makes equality return true.

Kinds 31, 32 and 57 take the superweapon branch. Supers precede ordinary
types without the side test. Between supers, native compares the signed
SuperWeaponType `+0xB0` recharge time, then the same `+0x60` name. It does
not read the individual SuperClass's custom recharge time or readiness.

`InsertEntry` increments count, scans the existing order and its empty tail,
then inserts before the first comparator-true entry. Later entries are copied
as complete 52-byte records. Consequently a new exact tie precedes an old
tie. It initializes the new entry's factory, request state, progress value,
rate and duration to zero; starts the progress timer at the current frame;
and clears the flash end frame. Kind 7 also records the BuildingType BuildCat.
Timer clock padding is not meaningful output. No RNG or detach runs in this
insertion path.

`SidebarClass::AddCameo` (`0x6A6300`) maps kinds to four strips: ordinary
buildings to 0, Combat BuildCat 5 buildings and supers to 1, infantry to 2,
and Unit/Aircraft to 3. It rejects an exact `(kind, typeIndex)` duplicate
before insertion. An accepted non-kind-31 entry calls the construction-options
EVA when the native nesting counter at `0xA8E7AC` is zero.

`StripClass::Recalculate` (`0x6AA600`) removes failed admissions and preserves
the order and full records of surviving entries. It never calls the
comparator, `Cost_Of` or `wcscmp`. It asks type virtual `+0x94` for a factory,
then that building's owner `CanBuild(type, false, true)`; a zero answer removes
the entry. A super survives only when its index is within the player's Supers
vector and its `SuperClass +0x6D` granted byte is set. Removing a linked
production factory or a type with a primary factory can enqueue abandonment
events; those production paths are excluded from this corpus. The removal
loop resets the vacated final record and its progress timer. Tab activation,
scroll adjustment and redraw follow the loop and are also excluded here.

## Active caller chronology

Instruction reading establishes the current-player recheck path in
`HouseClass::Update` (`0x4F9265..0x4F9302`):

- Read and clear the House `+0x1FC` recheck flag, then walk its Buildings
  vector in increasing stored index (`0x4F9283..0x4F92DB`). Skip null,
  non-live, zero-health, Selling current/queued-mission and foreign-owner
  entries. Dispatch surviving Building virtual `+0x4E0` at `0x4F92D2`.
- The original Building vtable slot `0x7E439C` contains `0x4456D0`,
  `UpdateConstructionOptions`. It additionally requires local owner,
  non-limbo, discovered-by-current-player `+0x41B` and online `+0x660`.
  According to its `BuildingType Factory +0xEB8`, it walks one of the four
  original type arrays in increasing index and calls `AddCameo` for every
  nonzero `CanBuild(false, true)` answer (`0x445736..0x44583F`).
- Only after these additions, call `RecalculateStrips` at `0x4F92E2`.
  `0x6A7D20` visits the four strips in order.
- Then call `Update_Owned_Supers` (`0x50AF10`) and
  `Grant_Provided_Supers` (`0x50B1D0`) at `0x4F92F6/0x4F92FD`.
  The grant pass walks the House Supers vector in increasing index and
  calls `AddCameo(kind31, index)` after an admitted grant.

Thus ordinary additions precede pruning within this recheck; newly granted
supers follow pruning. Their order is not established by a set of visible
IDs alone. The whole House recheck, power/discovery producers, complete
CanBuild mechanism, superweapon grants and alternate crate/trigger admission
paths are not executed by this ordering fixture.

## Recharge input identity

The constructor's original store at `0x6CE5E9` initializes SuperWeaponType
`+0xB0` to 4500 frames. The reader at `0x6CED5C..0x6CED95` calls native
`ReadDouble` for exact-case `RechargeTime` with default 0.0. If the returned
value is nonzero, it multiplies by original 900.0 and calls `Math::ftol`
before replacing `+0xB0`. A missing or zero value keeps the previous field.
The corpus executes those stores/readers and retained histories, including
`0.01` becoming 8 frames under the game's FPCW `0x0E7F`. It supplies the
INI cache lookup; it does not execute physical file loading or complete
Rules layer processing. Malformed floating strings and their stale-stack
scan result are deliberately outside this corpus.

## Retained lifecycle and Rust ownership

Instruction reading also establishes save/load lifetime. `MouseClass::Save`
(`0x5BE6D0`, stream write `0x5BE715..0x5BE71C`) writes the entire `0x556C`
display object, including the four strips at `+0x1544`, stride `0xF94`.
`MouseClass::Load` (`0x5BDF70`, read `0x5BE089..0x5BE092`) restores those
bytes. `Tab NoInit 0x5BE9B0 -> Sidebar NoInit 0x6A4F20` resets only progress
fields through `0x6AC7C0`; identities, counts and order remain. Subsequent
`Tab Init 0x6D03A0 -> Sidebar Init 0x6A5310 -> Strip InitSelectZones 0x6A8220`
rebuilds geometry without clearing records. The local House pointer is read
and swizzled separately at `0x67F9F3..0x67FA0E`.

A new scenario clears the records through `ClearScene 0x6851F0` (call at
`0x685573`) -> `Mouse InitClear 0x5BDF50` -> `Sidebar InitClear 0x6A5030`
-> `Strip InitClear 0x6A81B0`. Save/load control flow here is established
by original instructions; the executable corpus above does not emulate
whole save streams.

Rust keeps ordered identities in `ui::sidebar::cameo_order::CameoStrips`,
retained by the existing app sidebar projection. Canonical rule types,
`Simulation::cost_of`, `SuperWeaponType::recharge_time_frames` and the CSF
owner supply comparison fields. View construction resolves those identities
into presentation entries; rendering, tooltips, scrolling and hit testing
consume that same view. Rules/House changes refresh fields without sorting
survivors. The snapshot envelope stores only owner and ordered identities;
load preparation validates them against the restored interner/rules and
rebuilds derived fields before committing. A new scenario clears the owner.

Admission remains with the existing production and superweapon owners.
The existing `production_tech::sidebar_state` residual around House `+0x1FC`,
factory power and discovery is not fixed by this ordering mechanism. Newly
admitted ordinary types currently follow the production roster's iteration;
full cross-factory admission scheduling, alternate grants and linked-factory
removal/cancellation need their own native production integration evidence.

## Corpus and limits

- `comparisons`: 33 declared types, 1,089 original pair comparisons. Each
  row records the boolean, original cost-call count and whether the name
  comparison was reached. Cases separate same-side preference, category
  flag gating/overlap, signed values, supers, exact ties and UTF-16 order.
- `cost_contexts`: four factor contexts over six types, 24 original effective
  costs and 144 comparisons. All five factor slots and a FreeUnit building
  execute through the existing cost owner.
- `histories`: three retained four-strip histories. They cover registration
  order, reverse/mixed admissions, duplicates, a FactoryPlant change without
  resorting, removal/reinsertion, super grant loss, and original strip reset.
  Effective native costs before and after the factor change are included.
- `capacity_history`: the original gate rejects count greater than 75.
  However, the constructor initializes only 75 entries in the `0xF94`-byte
  strip. Native admits the 76th record into adjacent storage. This fixture
  deliberately observes that admission, the following rejection at 76,
  pruning to 75 and a later accepted retry; it excludes adjacent-state
  integrity and UI execution. A safe Rust vector can preserve the admission
  behavior without reproducing native memory corruption. Pruning first
  would admit a new cameo a recheck earlier than the original sequence.
- `recharge_reader`: original constructor field initialization and ten
  selected retained reader calls.

The whole comparator, RTTI resolver, type cost virtuals, name comparison,
insertion, strip constructor and clear execute unchanged. AddCameo stops
after insertion (`0x6A6423`) or before its rejection epilogue (`0x6A65FF`).
Recalculate starts at its entry and stops after the full filtering loop
(`0x6AAAB3`). Its visible-copy allocator and availability queries are declared
fixture boundaries, and EVA is an observed sink. Original instructions are
never replaced. These are bounded ordering comparisons, not whole-sidebar
or whole-engine parity.
