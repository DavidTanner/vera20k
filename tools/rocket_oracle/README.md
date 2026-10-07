# Rocket locomotor oracle

`flight.py` runs the original `RocketLocomotionClass` (ILocomotion vtable
`007F0B1C`) through whole missile flights: the constructor `00661EC0`, Move_To
`006632E0`, then one Process `006622C0` per frame until Detonate `00663030`
UnInits the owner or the row's frame budget ends. The impact predictor
`006620F0`, Is_Moving / Is_Moving_Now, the sine, cosine and arctangent tables,
`Sqrt_Approx`, `atan2`, `ftol`, the owner's FacingClass Set/Current, the
`In_Bounds` diamond and the owner's GetCoords / cell / GetHeight bodies execute.
The owner's Mark, SetLocation, display, AircraftTracker, sound, anim and damage
calls are recorded seams, listed in the module docstring and `flight.meta.json`.

Use the repository's `tools/requirements-test.txt` environment (Unicorn 2.1.4),
set `RA2_DIR` or `VERA20K_GAMEMD_EXE`, then from the repository root:

```sh
python -m tools.rocket_oracle.flight --check
```

`--write` replaces `flight.json` and its `.meta.json` after review. The Rust
parity test replays every row through the production kernel
(`sim::movement::rocket_movement`) with a host that records the same seams:

```sh
cargo test -p vera20k --lib sim::movement::rocket_movement::tests::
```

## Coverage

Sixteen rows on retail-shaped `[General]` blocks:

| Rows | Exercises |
| --- | --- |
| `v3_*` | V3: no pause, tilt, climb along PitchFinal, LazyCurve cruise, steering, predictor impact at the destination height; elite payload; a second Move_To ignored; killed on the rail (explodes on the first moving frame) and in cruise; not-alive owner skips the tracker update; In_Bounds refusals; CruiseStartDistance 0 falls back to the level cruise and dive; structural-bridge deck impact |
| `dmisl_*` | Dreadnought: pause, level cruise, dive (state 5) and its three turn arms, a short shot that loops, a dive onto higher ground (height clause) |
| `cmisl_boomer` | Boomer: V3TAKOFF puffs and the display resubmit in the pause, the raise (state 6), the climb's resubmit |
| `dmisl_with_null_types_takes_the_boomer_arms` | the Boomer test compares type pointers: two unset types send a DMisl block down the Boomer arms |

Not covered: the seam bodies themselves (FootClass Mark/SetLocation, the
AircraftTracker buckets, AnimClass, SelectAnim, Apply_area_damage), a bridge
the owner stands on (`+0x8C`), save/load (`Load @ 00663410`) and the missile's
own mission (`AircraftClass::Mission_Move`).
