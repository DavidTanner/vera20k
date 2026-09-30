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
  corrected (for example `UnitClass__DrawExtras` 0x73CEC0 is `UnitClass__Draw`).
- `[2026-09-30 YRpp names]`: a non-virtual function or global named from a YRpp
  address binding. The plate states whether the body's `RET` matches YRpp's declared
  arguments. The name stays a lead. A global that already had its own name kept it;
  its plate records YRpp's binding.
- `[2026-09-30 destructor audit]`: a destructor an older pass had named
  `__Constructor`, with the byte evidence.
- `vtable__<Class>` and `vtable__<Class>__secondary_<offset>` label each vtable from
  its RTTI complete object locator.

Destructor and COM-interface method names rest on the bytes. For the 2,356 method
names taken from YRpp's declaration order, each body's `ret N` was compared with
YRpp's declared parameters: none showed a shifted slot, and a one-slot shift would
have changed the popped bytes for about 60% of them. Two `GetSomeCellStruct` bodies
take a pointer argument YRpp does not declare. A random sample of 40 names from YRpp
order, YRpp address bindings and older overrides all matched their bodies; 10 were
trivial bodies judged through other overrides of the slot.

A name without a dated paragraph predates these passes; judge it by its own plate or
re-derive it. The scripts, plans and results of the 2026-09-30 passes are in the
machine-local research folder listed in `LOCAL.md`.

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
- The `find_code_gaps` records carry the neighbouring function names; compare gap
  positions and sizes, not the text, across renames.
