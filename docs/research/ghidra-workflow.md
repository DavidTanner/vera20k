# Ghidra working notes

[AGENTS.md](../../AGENTS.md) defines evidence and delivery. These notes cover tool
behavior and recurring interpretation errors; choose the investigation method yourself.

For repeatable static byte reads, disassembly, direct caller candidates and field
scans beside Ghidra, use [the shared native inspection tool](../../tools/native_inspect.md).
It checks the original executable identity and maps file-backed PE ranges through
the oracle owner. Its linear sweeps do not establish instruction boundaries,
reachability or exhaustive aliases; preserve those limits in findings.

## Connect to the intended program

Discover the instance and confirm the program path, binary identity and image base
before relying on addresses. Use explicit program selectors when the tool exposes
them; another session can change the shared current program.

The installed bridge registers analysis tools after connection. `check_tools` checks
that registry, not endpoint health: `not_found` before connection does not establish
that an operation is unsupported. After connecting, inspect the current schema,
load needed groups if using lazy loading, and try a relevant read. Tool names,
arguments and availability come from that schema, not an old command list.

Empty discovery does not prove Ghidra is stopped. Inspect the process and configured
connection before relaunching; use machine-local `ghidra-up` when available. Reuse
the analyzed program. Re-importing or enabling analysis is not routine reconnection.

## Interpret evidence

- Names, signatures and pseudocode are interpretations. Resolve consequential
  ambiguity from bytes/instructions, receiver/argument flow and actual callers.
  A nearby label or attractive decompile is not proof of identity.
- Check pointer types: `int *p; p[0xac]` addresses byte offset `0x2b0` on this
  32-bit target. Addition to an integer address uses byte offsets.
- For virtual calls, establish the table/subobject owner, read the actual slot,
  follow receiver-adjusting thunks and check callers. Inspect surrounding instructions
  for questionable boundaries; compiler lifecycle plumbing can resemble gameplay.
- Find state writers and initialization. Zero-filled image data may be populated
  at runtime. Confirm active-YR gates and retail inputs; inherited TS code alone
  does not establish a feature's applicability.

Follow production consumers far enough to establish the claimed result. Visual/audio
work includes composition, active flags, selected assets/frames, timing and output;
a loaded asset or working helper does not prove the final result. Keep address,
verified role and reproducible evidence together, naming uncertainty honestly.

## Names and their sources

Bulk passes append a dated, tagged paragraph to each plate they touch (data labels
get a plate on the data address). The tag says where the name came from and what
was checked:

- `[2026-09-30 vtable functions]`: a function created at an RTTI vtable target
  that had none; its extent is Ghidra's disassembly from that entry.
- `[2026-09-30 RTTI names]`: a virtual method named from its slot. The class prefix
  is the slot's owner, the first class in the MSVC RTTI hierarchy whose vtable
  holds this body at that slot. The plate names the slot (including the interface
  vtable of a secondary subobject) and the source of the method name: the COM
  interface declaration, the other named overrides of that slot, YRpp's virtual
  declaration order where its length matches the RTTI vtable (a lead), or the
  deleting-destructor body (flag test, `operator delete` 0x7C8B3D on `this`,
  `RET 4`). `vt_entry_<hex>` means the method is unidentified and gives the slot's
  byte offset; `_adjustor<N>` marks a thunk that shifts `this` by N and jumps to the
  implementation. Interface-slot names, wrong class prefixes and constructor names on
  destructor slots were corrected, and the plate records the earlier name. A corrected
  prefix keeps the earlier method name when the slot's method is unidentified; when
  that earlier name's class is unrelated to the slot owner, the name was kept. Where
  an older method name only disagrees with the slot, the name was kept and the plate
  records the slot's method; most of these are synonyms. Seven older names that YRpp
  gives to another slot of the same class were checked against their bodies and
  corrected (for example `UnitClass__DrawExtras` 0x73CEC0, now `UnitClass__DrawIt`).
- `[2026-09-30 YRpp names]`: a non-virtual function or global named from a YRpp
  address binding. The plate states whether the body's `RET` matches YRpp's declared
  arguments. The name stays a lead. A global that already had its own name kept it;
  its plate records YRpp's binding. A later pass also replaced `vt_entry` placeholders
  at YRpp-bound addresses when YRpp declares the method in that slot, and skipped
  bindings whose YRpp name is itself a placeholder (`func_3C`, `sub_53E3C0`). Some YRpp
  addresses are wrong: a few land in the middle of an instruction, and some are a few
  bytes off. The plates of the functions involved say which.

  These passes read an Ares-era YRpp copy (29d74e92). A refresh against the pinned
  Phobos fork (`reference/YRpp` at 8468aab5), whose paragraphs cite it, named 597
  more functions at fork-bound addresses, 43 `vt_entry` slots and 4 owner-draw window
  procedures, and labelled 185 globals. Every name passed the `RET` check;
  constructors and destructors of classes with RTTI vtables also had to store their
  class's vtable. Of a random sample of 40 names and 10 labels judged from their
  bodies, 48 matched, one was contradicted (not applied) and one could not be told.
  Where the fork binds another name at an address named from the older copy, the
  name stayed and the plate records the fork's binding. The fork renames 20 slot
  names taken from the older order (slot +0x114 is `DrawIt`, +0x42C
  `GetAttackCoordinates`, +0x2A8 `TurretFacing`), and they were renamed. Slot +0x104
  is `DrawIfVisible`: it tests visibility and then calls +0x114, so
  `ObjectClass__DrawIt` 0x5F4B10 and two overrides named after the wrong slot were
  corrected.
- Trigger actions: `TriggerAction__Execute` 0x6DD8B0 (the fork's
  `TActionClass::Execute`) switches on the action kind. Of the 131 handlers the fork
  binds, it calls 47 from the case its enum names. The other 84 have no call, jump or
  pointer anywhere in the image, so the game never runs them; the fork notes that
  Execute inlines most handlers. Port an action from its Execute case; the handler's
  plate says whether the game runs it.
- `[2026-09-30 destructor audit]`: a destructor an older pass had named
  `__Constructor`, with the byte evidence.
- `[2026-09-30 duplicate names]`: a name several functions shared, or a
  `__Constructor` name on a function that is not that constructor, corrected from
  the body.
  - The vtable a function stores last names its class. A constructor calls its base
    first and returns `this`; a destructor stores its own vtable first and returns
    nothing. An older pass had named every function that stores X's vtable
    `X__Constructor`.
  - `_NoInit` is the save-game constructor: `X__Load` or a derived NoInit
    constructor calls it, it pops one argument and it sets the vtables.
    `AbstractClass__Constructor_VtablesOnly` is AbstractClass's.
  - `_Default` is the constructor without arguments. Usually only
    `TClassFactory<X>__CreateInstance` calls it.
  - `_Copy`, `_StringObj` and `_FromSurface` mark other overloads; the plate says
    what each takes.
  - A function that is not the constructor its name claimed, and whose identity is
    unproven, went back to its default `FUN_` name. Its plate keeps the evidence and
    the earlier name. Examples are 0x4CD600, which `FlyLocomotionClass__Process`
    0x4CCB40 calls every frame, and 0x718B70, which only
    `TeleportLocomotionClass__Move_To` calls.
  - Twelve names still belong to more than one function. They are thunks that show
    their target's name, three identical `CRect` copies, the two `What_Am_I` slots of
    `CellClass` and `SuperClass`, and the two `VXL_Sort_Rasterize` variants.
- `vtable__<Class>` and `vtable__<Class>__secondary_<offset>` (a decimal offset) label
  every vtable that has an RTTI complete object locator. Where an older label existed,
  the older one stays primary, and listings and decompiles show it: `vtable_BuildingClass`,
  `vtable_MapClass` and the other map and sidebar layers, and the locomotors'
  `<Class>__ILocomotion_vtable`, `__IUnknown_vtable` and `__IPiggyback_vtable`.

Destructor and COM-interface method names rest on the bytes. For the 2,356 method
names taken from YRpp's declaration order, each body's `ret N` was compared with
YRpp's declared parameters: none showed a shifted slot, and a one-slot shift would
have changed the popped bytes for about 60% of them. The two bodies of slot +0x2F4
(`GetLastFlightMapCoords`, formerly `GetSomeCellStruct`) pop a pointer: the hidden
return buffer of the CellStruct the fork declares they return. A random sample of 40
names from YRpp order, YRpp address bindings and older overrides all matched their
bodies; 10 were trivial bodies judged through other overrides of the slot.

A name without a dated paragraph predates these passes; judge it by its own plate or
re-derive it. The scripts, plans and results of the 2026-09-30 passes are in the
machine-local research folder listed in `LOCAL.md`.

## Function boundaries

Offline passes on 2026-09-30 changed function extents. Their plates name what was
checked:

- `[2026-09-30 DB repair]`: misdecoded bytes that overlapped real instructions were
  cleared. The function was created or its body was extended, and the plate gives the
  old and new instruction counts.
- `[2026-09-30 recovered functions]`: a function created where real code had none.
  The plate names the evidence:
  - an entry of the startup or exit initializer tables (static initializers, which set
    globals' startup values);
  - a call or tail jump;
  - an address stored as a callback, such as the main window procedure 0x7775C0 and
    the dialog procedures;
  - or no reference at all. Such functions are probably dead code: show that one runs
    before porting it.
  
  Three functions were split out of bodies that had absorbed them, including one that
  `AircraftClass__Mission_Move` tail-jumps to.
- `[2026-09-30 boundary repair]`: one of these changes.
  - A body that stopped early was completed.
  - A fragment was merged back into its function.
  - A switch got its table and cases.
  - A jump to another function's entry was marked as a tail call.
  - A reference that pointed into the middle of an instruction was removed.
  - A body that left out bytes of its own instructions got them (11 functions).
  - `FUN_00435b70`, which started inside another instruction, became
    `BuildingLightClass__Destructor` at its real start 0x435B50.
  - Four functions that store a vtable were created where the bytes had never been
    decoded or were in no function, such as `LightConvertClass__Destructor` 0x556510.
  - Three scalar deleting destructors named `__Destructor` were renamed
    `__ScalarDeletingDestructor`, like every other function in the AbstractClass
    destructor slot (+0x20). `X__Destructor` names the plain destructor.

The decompiler follows control flow past a function's body, so a completed body
changes listings, cross-references and call graphs, but rarely the decompile. A switch
the decompiler cannot recover is the exception. Its decompile warns "Could not recover
jumptable" and "Treating indirect jump as call", and leaves out every case.
`BuildingClass__Mission_Attack` had this problem until a jump-table override was added.

The decompiler also shows a plain jump to another function's entry as that function's
code, inline. Every such jump now carries the flow override `CALL_RETURN`, so it shows
as a call. `_adjustor<N>` thunks therefore decompile as a bare call to their target,
because the target has no prototype. The `this` shift is N.

About 800 instructions still belong to no function. They are plausible code that
nothing references, so a reference from one of them (a reader or writer "in no
function") is not evidence until that code is shown to run. Some numbers in data
tables were once typed as pointers into code. A data reference into the middle of a
function is not proof of a code pointer until its source has been checked.

Undecoded bytes that decode as code are not always a missed function. Some functions
begin with a patched `ret` or `mov al,1; ret`, and their original body follows as
undecoded bytes that never run: 0x49F5C0, 0x49F740, 0x49F7A0 and 0x49F8B0, for example.
The code after each stub reaches its `ret` having popped 4 to 16 bytes more than it
pushed, which only the overwritten prologue could have supplied. At 0x4E60EB a
conditional jump was patched to `jmp`, which skips the code after it.

## Virtual-call references

Since 2026-09-30, a `call [reg+disp]` site whose receiver's class is established has
user-defined `COMPUTED_CALL` references to every function the RTTI vtables can put in
that slot. Caller lists, cross-references and call graphs include these virtual calls;
the decompile does not change. 6,296 of the 18,872 such sites have them. The analysis
and the list of added references are in the research folder listed in `LOCAL.md`.

- The receiver's class comes from the bytes: `this` of a virtual method (its class and
  every subclass), `this` of a function whose every caller passes a known object, a
  global object, an object a constructor just built, or a vtable the function stored.
- They are may-call edges: a call through a base class lists every override, including
  overrides that site never reaches.
- Calls through an object loaded from a field, an argument or a container have none. A
  method without callers may still be called virtually.
- `get_bulk_function_hashes` hashes cover references, so the hashes of the functions
  that got one changed that day.

## Class layouts

Since 2026-10-01 these structs have every field checked in code against the
constructors, ReadINI, the other methods and the reads through their type pointers:

- `BuildingTypeClass` (0x1798 bytes): 195 fields after its base. `BuildingClass` +0x520
  is `BuildingTypeClass *pType`.
- `TechnoTypeClass` (0xDF8 bytes): 318 fields after its base, so BuildingClass
  decompiles show `pType->base_TechnoTypeClass.nTechLevel`. `g_TechnoTypeClass_Array`
  is `TechnoTypeClass **`.
- `UnitTypeClass` (0xE78 bytes): 34 fields after its base. `UnitClass` +0x6C4 is
  `UnitTypeClass *pType` and `g_UnitTypeClass_Array` is `UnitTypeClass **`. The
  TechnoType and BuildingType fields that hold a unit type (`pUndeploysInto`,
  `pPowersUnit`, `pUnloadingClass`, `pFreeUnit`, `pSecretUnit`) are `UnitTypeClass *`.
- `InfantryTypeClass` (0xED0 bytes): 38 fields after its base. `pSequence` points to a
  `SequenceStruct`: 42 `SubSequenceStruct` entries of 0x24 bytes, one per DoType.
  `InfantryClass` +0x6C0 is `InfantryTypeClass *pType` and `g_InfantryTypeClass_Array` is
  `InfantryTypeClass **`. `pEnslaves` (TechnoType) and `pSecretInfantry` (BuildingType)
  are `InfantryTypeClass *`.
- `AircraftTypeClass` (0xE10 bytes): 11 fields after its base. `AircraftClass` +0x6C4 is
  `AircraftTypeClass *pType`, the same offset UnitClass uses, and
  `g_AircraftTypeClass_Array` is `AircraftTypeClass **`. `pAirstrikeTeamType`,
  `pEliteAirstrikeTeamType` and `pSpawns` (TechnoType) are `AircraftTypeClass *`.
  AircraftClass +0x6C0 holds the IFlyControl interface, so IFlyControl methods such as
  `AircraftClass__Is_Fighter` read the type as `[this+4]`.
- The `TechnoClass` chain, +0x0..+0x520: 224 fields in `AbstractClass` (0x24, new),
  `ObjectClass` (0xAC), `MissionClass` (0xD4), `RadioClass` (0xF0, new) and `TechnoClass`
  (0x520), with new member structs such as `FacingClass` (`PrimaryFacing` +0x388),
  `StageClass`, `TransitionTimer` and `RecoilData`. The flat structs keep their older
  fields in their old shape, so the same member can appear twice: `Location_X/_Y/_Z` ints
  in ObjectClass and TechnoClass are a `CoordStruct` in RadioClass, and MissionClass
  `nDispatchStartFrame`/`nDispatchDelayFrames` are the `CDTimerClass DispatchTimer` of
  TechnoClass. Object fields have no INI key: a name is the existing one, YRpp's where
  the code bears it out, one taken from the code, or `Unknown_0xNNN`. TechnoClass
  +0x2AC/+0x2B0 are `pLocomotorTarget`/`pLocomotorSource` (once `DeployedFrom` and
  `pDeployedInto`), and CDTimerClass +4 is `dwClockPad` (once `nAccumTime`; no timer
  reads it).
- `FootClass` (0x6C0 bytes) is flat from +0 like TechnoClass: the TechnoClass rows
  +0..+0x520 are copied in, then 66 own fields. The server's naming policy prefixed 28
  older names in the copy (`Health` is `nHealth` there), so fix a chain field in both
  structs. `UnitClass`, `InfantryClass` and `AircraftClass` embed `FootClass
  base_FootClass` at +0. Corrected names include `NavQueue` +0x5AC (once `EnterQueue`),
  the attack-move order +0x5C4..+0x5D1 (`MegaMission`, `pMegaDestination`,
  `pMegaTarget`, `fHaveAttackMoveTarget`; once `TarCom_*`), the path directions +0x5E0
  (one int[24] kept as `nPathDirections` and `aPathDirections_1`), `cTubeIndex` +0x684,
  `fIsInitiated` +0x689 (once `bConvoyArrived`), `fIsFiring` +0x68D (once
  `bHasReachedDock`) and `fShouldEnterOccupiable` +0x690 (once `bIsDockingToBuilding`).
- `BuildingClass` (0x720 bytes) is flat from +0 the same way: the TechnoClass rows
  +0..+0x520, then 94 own fields, so a chain field is now fixed in TechnoClass, FootClass
  and BuildingClass. The garrison is `Occupants` +0x684 and the overpowering infantry
  `Overpowerers` +0x66C, each a `DynamicVectorClass<InfantryClass *>` spelled out as seven
  members. Where the code contradicts YRpp: +0x6CA/+0x6CB are the `[Structures]` map fields
  8 and 15 (`AIRebuildable`, `AIRepairable`; YRpp BeingProduced and ShouldRebuild), +0x6EB
  holds +1 or -1 for the cloak generator (`CloakGeneratorState`; YRpp HasCloakingData),
  and +0x53C, +0x6C9 and +0x6DE stay `Unknown_0xNNN` (YRpp OwnerCountryIndex,
  ShowRealName and NeedsRepairs; the code shows none of those roles). `PrismTargetCoords`
  +0x708 holds the weapon index in delayed-fire stage 1 and the coordinates only in
  stage 2.

Notes for readers:

- **Field names.** A field read from an INI key carries the exact key. The server's
  strict naming policy puts a Hungarian type prefix in front: `fPowered`, `nX`,
  `aBuildupFile`, `pToOverlay`. A key that names a file is stored in `<Key>File`, and
  the object built from it takes the key: `aCameoFile` and `pCameo`.
- **Still placeholders.** `ObjectTypeClass` (0x294 bytes) has only `pVtable` typed.
  `AircraftClass` (0x6D8 bytes) has its FootClass base and `pType`, none of its own
  fields.
- **Receivers.** Since 2026-10-01 the methods of `AbstractClass`, `ObjectClass`,
  `MissionClass`, `RadioClass`, `TechnoClass`, `FootClass`, `UnitClass`,
  `InfantryClass`, `AircraftClass` and `BuildingClass` have typed receivers. 928 of the
  1,079 functions with those name prefixes are `__thiscall` in their class namespace, so a decompile
  reads `TechnoClass::TechnoClass__IronCurtain(TechnoClass *this, ...)` and
  `this->IronCurtainTimer`. Each prototype declares the stack bytes its RETs pop;
  parameters nobody has checked are `undefined4`. Plates tagged `[receivers 2026-10-01]`
  record twelve corrected prototypes, among them the two `GetCursorForCell` of FootClass
  and InfantryClass, which take no stack parameter. Every direct call to the TechnoClass
  and ObjectClass `ReceiveDamage` now shows its seven arguments, and the 70 calls to
  `BuildingClass__CreateAnimForSlot` show their five. The plates also say why 151
  functions stay untyped:
  - COM methods (primary-vtable slots 0–7) take `this` on the stack (`__stdcall`).
  - Methods of a secondary interface receive the interface pointer, not the object.
    The IUnknown adjustor thunks (`_adjustor<N>`) shift it on the stack and jump.
    `What_Am_I`, `Fetch_ID` and `Create_ID` (+4) and AircraftClass's IFlyControl
    methods (+0x6C0) take it as their first stack argument. The INoticeSink overrides
    (+8) read it from ECX, which is the object + 8. The interface names are YRpp leads.
  - Trampolines tail-jump through a vtable, and some direct functions have no usable
    caller evidence. Some are not methods: `BuildingClass__ReadFromINI` 0x44F820 runs
    with the scenario INI in ECX.

  Six vtable methods without a class prefix were outside both passes and are untyped:
  0x4D9C60, 0x4E0150, 0x6FDD50, 0x709A90, 0x70A990 and 0x70AA60. The type-class methods
  are not typed yet, so their decompiles still show raw offsets.

Plates tagged `[2026-10-01 BuildingTypeClass layout]`,
`[2026-10-01 TechnoTypeClass layout]`, `[2026-10-01 UnitTypeClass layout]`,
`[2026-10-01 InfantryTypeClass layout]`, `[2026-10-01 AircraftTypeClass layout]`,
`[2026-10-01 TechnoClass layout]`, `[2026-10-01 FootClass layout]` and
`[2026-10-01 BuildingClass layout]` record where YRpp is wrong and the native quirks a
port must keep. Examples:

- AddOccupy and RemoveOccupy are swapped in YRpp.
- `TurretControl` is 0x14 bytes, and nothing initialises WeaponCount.
- PitchAngle is read in degrees but stored in radians.
- An unset BurstDelay draws a 3..5 frame delay from the scenario RNG.
- ReadPip does not keep an absent Pip: the default 1 comes back as 2.
- A missing EliteAirstrikeTeamType takes the AirstrikeTeamType read on the same pass.
- A SpawnDelay of 0 faults (it divides the frame counter) on an aircraft with a Trailer.
- The TechnoClass constructor draws once from the scenario RNG (+0x3C8), and
  TechnoClass::Fire draws the first spray offset of each burst (+0x2A0).
- The magnetron release sets BeingManipulatedBy only on a foot target whose vt+0x1C8()
  is above 0.
- Secret labs draw their reward with the drawn number itself as the index, so two labs can
  get the same type (`Assign_Secret_Lab_Production` 0x68C050).

The UnitTypeClass pass also corrected two wrong names: the LandType name converters
0x48DFD0 and 0x48DF80, once named `MovementZone_*`, are `LandType__ToName` and
`LandType__FromName`. The InfantryTypeClass pass found that 0x522910, named
`BuildingClass__AddGarrisonOccupant`, runs with the entering infantry as `this`; its
plate has the evidence. The AircraftTypeClass pass found that
`HouseClass__CheckBuildLimit` reads AirportBound through its type argument, where no
scan of the reads through AircraftClass +0x6C4 can see it; its plate has the rule. The
TechnoClass pass renamed nine functions whose `this` or purpose the code contradicts:
0x4D0EF0 `FoggedObjectClass__Constructor_Building`, 0x6B7D80
`SpawnManagerClass__CountLaunchingSpawns`, 0x4C2BD0 `EBolt__SetOwner`, 0x56DC20
`MapClass__Find_Nearby_Passable_Cell`, 0x720440 `ThemeControl__Constructor`, and the
iron-curtain and airstrike tint functions 0x70E380, 0x70E4B0, 0x70E5A0 and 0x70E920 (once
named after temporal, warp-in and gap effects). The FootClass pass renamed 0x4DFCB0
`FootClass__EnterBattleBunker` (once `Find_Nearest_Dock`) and 0x457CE0
`BuildingClass__CanBeOccupiedBy` (once `CanDock`; it tests CanBeOccupied, Occupier and
Assaulter, not docking). The BuildingClass pass named 0x459840
`BuildingClass__GetSecretProduction` and 0x68C050 `Assign_Secret_Lab_Production`, and
corrected the HasTurret 0x4527D0 plate (its loop walks the upgrades, not the occupants).
Each plate gives the evidence. The
per-field ledgers, the checks and the rehearsals are in the research folder listed in
`LOCAL.md`. Reading established these facts. Nothing was executed, so a port pins the
conversions with the native oracle.

## Preserve findings without polluting shared analysis

During authorized reverse engineering, preserve proven identities and useful evidence
with focused labels, comments and missing references. Read-only requests or
`--no-sync-ghidra-labels` disable these writes; `--sync-ghidra-labels` explicitly requests
them. This policy is the same for serial and delegated work. No separate candidate
ledger is required when a concise finding suffices.

Keep uncertain identities unnamed. References need decoded endpoints, operand and
reference kind; check for duplicates. Type, prototype or boundary repairs belong
within an authorized analysis-repair task, with prior definitions recoverable.
Once that scope is granted, do not ask permission for every edit. Read back structural
repairs immediately, including layout/offsets and affected decompilation. Byte patches,
bulk reanalysis and unrelated database changes need their own task scope.

Use one writer per shared program and coordinate changes affecting other workers'
evidence. Small coherent annotation batches are allowed. Inspect per-item results,
explicitly save the intended program, and read back the changes before unrelated
work or handoff. A committed analysis transaction is not a disk save. After a timeout,
inspect actual state before retrying; report partial or unsaved work accurately.

Label/type changes affect analysis, not executable bytes. Inspect current analyzer
settings when relevant; do not assume historical settings are still in force or
blame drift on an analyzer without evidence.

Tool behavior checked 2026-09-04 against the installed GhidraMCP 5.14.2 bridge
(`connect_instance`, `check_tools`) and plugin (`CommentService`,
`ProgramScriptService.saveCurrentProgram`). No connected instance was available
for a live persistence test; repeat capability checks when working against one.

Checked 2026-09-30 against the headless GhidraMCP 5.14.2 server:

- `get_plate_comment` and `set_plate_comment` work on functions only. For a data
  address, read the plate with `audit_global` (`plate_comment`) and write it with
  `batch_set_comments`, which rejects a first line shorter than four words.
- `rename_data` rejects a global name without a `g_` prefix; `rename_or_label`
  accepts a per-call `strict_mode` (`enforce`, `warn`, `off`).
- `can_rename_at_address` omits the current name at undefined addresses;
  `audit_global` reports it.
- A thunk shows its target's name until it gets its own, so renaming the target
  renames the thunk's display name. Rename thunks before their targets.
- `rename_function_by_address` with a function's own default name (`FUN_` and its
  address) turns the name back into a default symbol. It rejects an empty name.
- `create_label` at an address that already has a label adds a second one; the first
  stays primary. `audit_global` reports only the primary label; `list_globals` with
  `name_substring` finds the others.
- The `find_code_gaps` records carry the neighbouring function names; compare gap
  positions and sizes, not the text, across renames.

Checked 2026-10-01, struct tools:

- Send `create_struct` and `recreate_struct` `fields` as a JSON string. In a JSON array,
  each offset reaches the parser as a Gson double (`3589.0`), which `Integer.parseInt`
  rejects. The field is then appended instead, so the layout comes out packed.
- Strict naming is the default when the project has no `.ghidra-mcp/conventions.json`.
  It puts a Hungarian type prefix on struct field names on create, `add_struct_field`
  and `modify_struct_field`, and a per-call `strict_mode` does not change that. Struct
  types, `sbyte` and the plain `pointer` type keep the name as given.
- A retype clears the field name. Pass `new_name` to `modify_struct_field`;
  `modify_struct_field_type` always drops the name.
- No struct tool clears a field in place. `remove_struct_field` deletes the component
  and shifts every later field down (FootClass shrank from 0x6C0 to 0x6BC bytes on a
  staging copy). `add_struct_field` refuses to overlay a defined field ("Not enough
  undefined bytes"), and `modify_struct_field` rejects `undefined` ("New data type not
  found"). Change a live layout only by filling undefined bytes and retyping or renaming
  in place; a retype to a smaller type frees the tail bytes.

Checked 2026-10-01 on a staging copy, receiver tools:

- `set_function_this_type` needs a `__thiscall` or `__fastcall` convention first. It
  moves the function into a class namespace named after the struct, and the auto
  `this` then takes that struct. It leaves an explicit custom-storage `this` with its
  old type. `set_function_prototype` applies dynamic storage, so run it first. No
  endpoint moves a function back to the global namespace.
- A `__thiscall` prototype must declare as many stack bytes as the function's RETs pop.
  `(void)` on a `ret 0xC` function breaks the stack analysis of its callers. A custom
  prototype that declares only `this` makes every direct caller's decompile drop the
  arguments.
