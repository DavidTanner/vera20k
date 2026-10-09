# Next/Previous object selection

Run from the checkout with the supported retail executable selected:

```sh
VERA20K_GAMEMD_EXE=/path/to/gamemd.exe python -m tools.input_oracle.selection_navigation --check
```

Explicit `--write` regenerates the JSON and identity/coverage sidecar. The shared
native runner verifies executable identity, checked return boundaries and unchanged
original code. No executable is distributed. Inputs are initialized memory, not a
native scenario load or whole constructor/lifecycle proof.

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

The measured selection path does not change Scenario or MapGen RNG. Main is a
presentation stream; retaining its exact fixture state does not establish whole
engine determinism. No action-line timer start or Detach is traversed in this
bounded untagged path. Arbitrary Tag actions and later audio/renderer consumers
remain separate behavior.
