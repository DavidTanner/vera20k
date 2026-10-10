# Keyboard selection navigation

Run from the checkout with the supported retail executable selected:

```sh
VERA20K_GAMEMD_EXE=/path/to/gamemd.exe \
VERA20K_COMBATANT_INPUTS=/path/to/physical/selection/layers \
python -m tools.input_oracle.selection_navigation --check
```

Explicit `--write` regenerates the JSON and identity/coverage sidecar. The shared
native runner verifies executable identity, checked return boundaries and unchanged
original code. No executable is distributed. Inputs are initialized memory, not a
native scenario load or whole constructor/lifecycle proof.

The physical selection directory contains `RULESMD.INI`, `MPBATTLEMD.INI` and
`XMP03T4.MAP`, plus `LANGRULE.INI` when present. Filename case may follow the
existing extraction tool. Extract these bytes through the production asset owner;
the appended corpus records their SHA-256 identities. The older three groups
retain their original inputs, fields and outputs.

The fixture reuses `KeyboardFixture` to execute ordinary command registration,
the input oracle's scalar/sink helpers, `NativeCallTrace` for nested return
observation, and `gen_rng_vectors` for original seeding and raw-draw continuation.
There is no Python comparator, candidate scan, selection mutation or RNG algorithm.

## Native route

Registered NextObject/PreviousObject execute536610/536A80. They disable power,
planning, repair and sell in that order; capture the first selected pointer; call
4AA2B0/4AA380; and, when a candidate exists, call UnselectAll48DC90, candidate
Select+14C, CenterOnSelection4AE290 and Redraw4F42F0(1). The Select result is ignored.

Search reads the actual820030 table `[2,3,4,4,3,2]`: Ground/Air/Top forwards for
Next and Top/Air/Ground backwards for Previous. It preserves each layer's stored
order. A candidate passes actual6F32D0 and6FC030 before it can count as the anchor.
An absent or ineligible anchor wraps to the first eligible member. Nulls and the
other two display layers do not participate.

Real Unit7F5C70 and Infantry7EB058 vtables dispatch those gates. The primary gate
checks positive health, non-limbo, in-playfield, non-Building, discovered-current,
Selectable, and House50B6F0. Dynamic selection rejects slave, robot-offline,
bunker and dock-on-building state, then reaches Foot4DFA50/Object5F6C30. Foot+6AD
is the **locomotor-swap** flag; it must not be confused with virtual+1D4/+270
(warp refusal) checked later by ObjectSelect5F4520. The supplied disguise+1D8 is
zero; these controls do not establish disguised-object visibility.

ObjectDeselect5F44A0 clears native follow when deselecting the followed object.
Singleton wrap therefore clears follow despite selecting the same identity again.
No candidate leaves selection/follow/camera unchanged after mode cancellation.
A found candidate refused by pending placement clears selection/follow, preserves
placement and armed modes, does not forward a camera coordinate and still redraws.
Mission-only+3D4 and virtual+1D4 refusal controls also distinguish search admission
from final Select admission. Ordinary Unit+6D8 is explicitly supplied as-1;
zero-filled Unit memory would incorrectly report a control blocker in746C90 and
hide the mission-only refusal.

## Corpus and execution boundaries

- `search_cases`: explicit actors, five layer vectors, direction, anchor and
  observed candidate/ordered gate visits. Includes building exclusion, dock cell
  queries, campaign human/control ownership and the separate Alive bit control.
- `command_histories`: original consecutive Execute calls retain the native
  selection array. Each step records selection/selected bits, follow, modes,
  placement, type-selection mode, calls, forwarded coordinate, redraw, queued
  voices and raw draws. This is the native reference for Rust's pending selection
  integration; no Rust scheduling assumption is supplied to the fixture.
- `voice_histories`: original708EB0/708D90 over empty/single/multiple numeric
  lists, seeds0/1/1234, both ordinary classes, special list branches and queue
  rejection controls. Each history retains full RNG bytes and the next four
  original Main draws. Sound IDs are fixture-relative, not stock registry IDs.
  The appended `unit_multiple_seed1_after248_main_raw` first executes248 raw
  draws on the same Main886B88. Its `main_prefix` records those draws and full
  before/after state; `rng_before_hex.main` is that advanced state. Four voices
  then cross the250-word cursor wrap before `main_next_four` records the next
  original raw continuation. The prior28 histories retain their fields/results.

The corpus contains94 searches,20 command histories/31 calls and29 voice
histories/112 calls, plus the248-call Main prefix. The prefix supplies prior
consumer activity; it does not execute terrain loading, MoveSound, Gattling or
death. It distinguishes a retained shared Main cursor from a freshly seeded
audio-only cursor without introducing a fixture RNG algorithm.

Mouse cursor entries5BDA80/5BDAA0, final Tactical camera application6D6070 and
Redraw4F42F0 are observed presentation boundaries. All search, candidate, selection,
follow cleanup, mode bodies and camera selection reduction execute unchanged.
The planning cursor backup is empty. Tags are null, planning-group propagation is
off, and selected-control-group metadata is empty. No graphical window, bandbox
geometry, Fly movement, display membership producer, authored Selected trigger,
save stream, audio playback or whole-game parity is claimed.

## Voice draw and admission distinction

For any nonempty selected voice list, VoiceSelect708EB0 spends one raw Main
Random65C780 draw, then chooses `draw % count` and calls QueueVoice708D90. This
includes a one-item list. An empty list preserves the queued ID and RNG state.
QueueVoice may subsequently refuse a disabled voice, nonlocal owner, or-1 sound;
the draw has already occurred. Direct receiver controls pin that ordering.

The outer TechnoSelect6FBFA0 checks successful ObjectSelect, owner/HouseType
admission and global822CF2 **before** calling VoiceSelect. Thus the complete
`voice_disabled` command control spends no draw. These are different call paths.
Ordinary PickCallback4AC2B0 clears822CF2 at4AC2FB only after a successful Select;
Tactical rectangle selection similarly clears it at6DA6F5 and restores it at
6DA71D/6DA730. That caller ordering is instruction-backed, not rectangle execution
in this fixture. A first successful object with an empty voice list still closes
the ordinary first-success voice allowance.

The measured selection path does not change Scenario or MapGen RNG. Main886B88
is one shared native stream, including callers outside audio; retaining its
exact fixture state does not establish whole-engine determinism. No action-line
timer start or Detach is traversed in this
bounded untagged path. Arbitrary Tag actions and later audio/renderer consumers
remain separate behavior.

## CombatantSelect P and retained selection modes

`combatant_histories` executes the originally registered CombatantSelect
`5367F0`, TypeSelect `5368B0` (press and release), and HealthNav `536950`.
P and Health pass `!(key >> 8) & 1`; ordinary P clears selection, Shift+P adds.
TypeSelect uses its actual key-edge/tap receiver, with a supplied 100 ms
`timeGetTime` interval. No command, filter, selection, comparator or RNG algorithm
is implemented in Python.

P `732280` first rejects the nonzero command guard `A8B538`, then invokes
`6DA770` to drop a lone nonowned selection. Outside campaign, owner admission is
exact current-house identity through `50B6F0`; campaign uses owner `+1ED` directly.
Human `+1EC` alone does not pass that campaign branch. `Deselect5F44A0` clears
follow where appropriate and retains the current selection mode; dropping a
lone nonlocal selection therefore retains an existing P map scope.
The `shift_P_lone_nonlocal_campaign0_human0` and
`shift_P_lone_nonlocal_campaign1_human1` histories start with mode1/maptrue,
selected/follow20 and Shift held. Both remove the nonlocal20, clear follow,
retain map scope and end with only local40 selected. Shift's retained selection
policy therefore still requires this explicit deselection to reach committed
membership; the campaign control has human1EC true and player-control1ED false.

The source gate `732580` tests pointer, Alive `+90` and that owner admission.
It does not use the stricter N/M health/limbo/in-playfield/discovered gate
`6F32D0`. The supplied undisguised controls distinguish those inputs. P screen
collection executes `6DC420/6DC430` on initialized Tactical selectable records,
then `7342C0` checks only nonnull and abstract flag `+14 & 1`. Map collection
walks the supplied native Techno vector in stored order and applies `732580`.

Escalation checks the collected screen candidates for non-Building, type `+DBC`,
dynamic virtual `+13C`, and not already selected. With none, or all already
selected, it expands to the map **in the same call**, before clearing selection.
P mode `1` with map byte `B0FE64` already true starts directly from the map.
Final predicate `7325C0` repeats class/type/dynamic admission, then calls original
Select `+14C`, whose additional refusals remain authoritative. Thus limbo,
mission-only, warp and pending placement can produce NothingSelected while P
still has screen scope; the preflight did find an unselected dynamic candidate.

ObjectSelect `5F4520` prepends positive-Primary type `+C9C` objects and appends
others. Every history records source order, Select/voice call order and resulting
CurrentObjects order independently. P voices every eligible newly selected
object while the global voice allowance stays enabled. Replacement reselects
and voices again; Shift's already-selected refusals spend no draw. Empty and
minus-one voice controls preserve the native distinction between list draw and
queue admission. Every step replays the observed Main draws through the original
RNG owner, checks all Scenario/MapGen bytes, and preserves full history boundary
states plus the next four original Main draws.

P finishes with mode `1`, T with mode `2`, and Health with mode `3` when its
snapshot is nonempty or mode `0` when empty. These modes share **one** `B0FE64`
scope byte. Original Health leaves that byte unchanged; a later P or T restarts
scope according to its mode check. The `T_map_P_restarts_screen` history exposes
T map selection followed by P screen selection. Direct ordinary TechnoSelect
and UnselectAll witnesses execute actual mode reset, retain the map byte, then
show the next P restarting the screen preflight. They do not execute a mouse
gesture or a full native scenario replacement.
`ordinary_deselect_retains_P_map_scope` separately executes Deselect5F44A0 on
local20 and then P. Deselect clears membership/follow but preserves mode1/maptrue,
so the following P selects the map candidates. This direct boundary distinguishes
Deselect from successful Select and UnselectAll mode reset. The older output field
`selection_across_map` remains its historical `B0FE58` submode observation;
the new `across_map` and `submode` fields separately name `B0FE64/B0FE58`.

P preserves armed repair/sell/power/planning state, makes no camera/redraw request,
and does not start the action-line timer or enter observed world/abstract Detach.
Timer bytes and writes are observed for active and inactive timers. Follow
clears through actual Deselect during replacement and remains for retained
Shift selection, except the lone-nonowned cleanup described above. MessageList
is a declared UI sink after original copying or
formatting; the fixture supplies actual strings from physical
`langmd.mix/ra2md.csf` through the existing CSF reader and records the original
seven message arguments. It does not execute rendering, device playback or
message timer scheduling.

The fixture supplies slave/offline/bunker/dock/locomotor-swap bits to execute
refusal controls. Their full lifecycle, robot power/voice transitions, Tag
Selected actions, disguise, display membership production, world replacement,
saved streams and held mouse/movement/Fly paths remain outside this corpus.

## IsSelectableCombatant type inputs

`combatant_type_histories` composes the existing BulletReader/type/INI owners.
Full InfantryType `5236A0`, UnitType `7470D0`, BuildingType `45DD90` and base
TechnoType `710AF0` constructors overwrite an `A5`-poisoned allocation. The
observed field writer `71164B` sets `+DBC` to false for each selected type.

For every physical layer, original AbstractType section admission `410A60` runs.
An admitted section then executes the unchanged field block
`71574E..71576F`: current-field default, type-name accessor `524EC0`, exact
literal `IsSelectableCombatant` at `843414`, ReadBool `5295F0`, and final store.
The physical RULESMD/LANGRULE/mode/map history and explicit missing-section,
missing-key, wrong-case, empty, malformed, numeric and retained-value histories
pin native outputs. There is no clamp or independent reset between retained
passes. Physical E1/E2/MTNK/ROBO become true; ENGINEER/HARV/CMIN/GACNST remain
false for the selected stock layers.

These controls execute whole constructors and the original selected field
reader. They do not execute unrelated intervening type reads, full Rules
scenario processing, physical INI file loading, asset binding or actor
constructors. All original `.text` bytes are checked after the type history.
