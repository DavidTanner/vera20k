# Concrete ground-bridge damage and rebuilding

The stock Anytown bridge uses concrete overlays205..232 on ground-level cells;
it has no raised deck or structural bridge record. Its damage, hut and cursor
consumers must read live Cell overlay identity. Requiring a `BridgeRuntimeCell`
missed this authored bridge, while the former deferred damage walker could not
publish Cell/Recalc/occupant effects at their original call positions.

The shared ordinary-bridge controller now owns `57CCF0`, roots`57CF60/57D530`
and propagation`57E7A0/57ED00`. Damage and the existing Engineer repair controller
share centering, family classification and the live publication host. Each Cell
write reaches resolved terrain, OverlayGrid and saved dynamic terrain before
Recalc and occupant callbacks. The runtime bridge mirror is updated when present;
it is not a prerequisite for ground-concrete publication. Superseded concrete
damage walkers and test-only repair implementations were removed.

`487A10` is one shared damage/repair occupant controller. Residents retain Next
before callbacks; incoming Foot objects read Next after callbacks. Damage mode
adds the resident TechnoType JumpJet gate. Concrete EW roots pass mode1 even on
first damage, whereas NS roots pass their final-collapse byte. The host uses
the existing numeric admission, active locomotor coordinates and direct damage
receiver owners. These concrete primitives never call structural`47DD70`.

The ordinary MTNK firing witness also exposed an existing prerequisite error:
Unit`736473` and Infantry`51BC51` Ready/Commence belong before Foot/Techno AI.
Commence clears the mission counter; the shared Techno increment`6FA64E` follows
it. Promoting after that increment left the first mission visit at counter0.
The existing mission authority and post-movement checkpoints remain the owners.

The Slice6 retasking regression still compares every recorded frame and all
three RNG streams. Its final actors now have mission counters5/16/7. Inverting
only the additional first visit for actors1 and3 restores the unchanged prior
hash`498EF189907FB2F2` and all historical schema projections; actor2 already
entered Guard during Unlimbo. The test then restores both live missions. This
is a causal Rust regression check, separate from the executed native first-visit
counter1 in the mission packet.

## Evidence and executable comparisons

The executable is SHA256
`1cdd1180e49024fbda8ad568caac2e86e856063ff67ab38f62b7d2c7bb84298c`.
Native packages record their input identity, substitutions and replay commands.
They do not claim a single uninterrupted native whole-game comparison.

| Boundary | Native evidence and comparison |
| --- | --- |
| Concrete scalar controllers | [90 cases](../../tools/spatial_oracle/bridge_ordinary_damage.json), including both axes, every overlay204..233 and all three width positions; ordered stores/callbacks match Rust. Recalc, occupants and graph callbacks are seams in this corpus. |
| Shared occupant traversal | [29 cases](../../tools/spatial_oracle/bridge_occupants.json), preserving cached/live Next order, incoming coordinates and damage-mode gates; linked Rust controller tests. |
| Physical damage and navigation | [Anytown package](../../tools/spatial_oracle/anytown_damage/README.md) and [full navigation witness](../../tools/spatial_oracle/anytown_damage/navigation.md): original Recalc and navigation over13,515 physical cells; healthy, damaged, collapsed and repaired class/height/base-ID planes, all13 movement rows and all three ordered graphs compare to production exports. |
| Hut damage | Both authored huts in three starting states compare21 final cells, full Main/Scenario/MapGen states and ordered animation requests. [Native projection](../../tools/spatial_oracle/anytown_damage/hut_test_vectors.json) is generated from the executed packet, not calculated in Rust. This boundary starts at the hut dispatcher, not a whole Building death sequence. |
| Repair cursor | [Eight original calls](../../tools/spatial_oracle/anytown_damage/hut_cursor.md): each hut returns0/0/1/0 for healthy/first-damaged/collapsed/repaired. The selected overlay query does not change Cells, RNG or frame. |
| Ordinary firing | [Mission evidence](../../tools/spatial_oracle/anytown_damage/mission.md) executes command, Unit/Foot/Techno/Mission, firing and projectile paths. The interleaving witness supplies observed Terrain RNG prefixes through original Next, then compares naturally executed selected-owner calls. It does not establish native Terrain scheduling. |
| Load reconstruction | [Original 581F50 execution](../../tools/spatial_oracle/anytown_navigation_restore.md) independently rebuilds each damaged, collapsed and repaired native state; all27,225 allocated base planes,13 movement rows and complete ordered graphs match production restoration exports. |

The physical Rust witnesses load the original map and retail rules through the
production loader. They cover admitted damage, incoming Drive heads, both huts,
ordinary ForceAttackCell and Engineer CaptureBuilding, and validated save/load
continuations. The rendering witness uses the same live overlay builder and
atlas/submission owners through an offscreen GPU surface; its claim is bounded
to published concrete overlay art, not native full-screen pixel equivalence.

The ordinary retail firing run reaches first damage at Logic5370, collapse at
Logic7013 and completed Engineer repair at committed frame7050. It checks
immediate Cell-target detachment and no further source shots before Guard.
All four navigation states from this full production run also match the frozen
native topology packet. These frame numbers describe the Rust loaded-world
stream; the native selected-owner packet has a different global caller history
and does not establish whole-world timing parity.

The [validation receipt](../../tools/spatial_oracle/bridge_concrete_validation.json)
pins the final test/release binaries and actual outputs:9,554 retail lib tests,
17 focused regression checks, six physical/offscreen tests, clippy success and
a release-build retail load producing ten digests. The final physical and
ordinary firing exports each match all four native navigation states; all
three restored states match original hierarchy reconstruction. The Python tool
suite reports168 tests with two expected skips. The receipt preserves the
comparison limits and Ghidra readback hashes.

## RNG, ordering and persistence

Area damage retains its native inclusive strength roll and rejection draws.
The concrete primitive itself has no RNG draw. Occupant receivers run immediately,
so any receiver-owned RNG and lifecycle effects precede subsequent cells. A true
primitive return lets the area caller detach the original Cell target through
`70D4A0`; first damage does not detach it. Hut pre-effects precede each primitive
and retain the existing Scenario animation-position/delay draws. The hut packet
pins their complete stream states and constructor requests. Engineer variants
use MapGen; the physical seed0 repair yields the three original variant draws
1,1,2. No independent bridge timer was added. The firing witness retains mission
dispatch, counter, rearm and projectile frame boundaries.

Snapshot restoration retains Cell overlay/terrain authority and rebuilds its
derived navigation. Native successful load`67E8CD` calls`581F50`, which clears
the three graph record vectors and rebuilds them through`581F90` (cell IDs reset
at`58200F`). Pre-save incremental graph IDs and retired records therefore are
not a load invariant. Native MouseLoad`5BE355..5BE3A7` does retain the base
class/height/ID plane and13 movement rows. The Rust witness checks those retained
facts, independently rebuilt graphs and deterministic restored futures.

## Remaining whole-bridge work

This chain is the common stock Anytown ground-concrete path. Required mechanisms
outside that path keep the whole-bridge goal open:

- A tagged span invokes event31 through`575EE0`; the current publication visits
  cells but does not dispatch that trigger. Tagged maps can miss downstream
  mission effects. The untagged stock witness cannot cover this dependency.
- Cursor tileset precedence and the structural BridgeRecord branch still need
  their complete owner. Huts beside structural or mixed tile/overlay families
  can choose the wrong cursor or repair admission.
- The structural hut fallback remains a separate required mechanism. Ordinary
  wooden damage now shares the synchronous owner; its evidence and remaining
  boundaries are in [the wooden chain](https://github.com/YuriPlanet/vera20k/blob/e83df3de8bf2d9fa1ba6d7c4a9e70b5236b7294a/docs/research/bridge-wood-ground-damage.md).
- The physical witness covers one concrete orientation. Both scalar axes are
  executed, but physical loading, geometry and connected consumers of the other
  authored bridge arrangements remain part of the whole-bridge audit.
- FV/HoverMissile force-fire range and pursuit still interpret the legacy
  `has_bridge_deck` overlay-identity flag as raised target height. On a ground
  bridge this adds416 leptons and changes non-arcing maximum/minimum-range
  decisions. The existing retained-cell range owner has the native raw-flag
  and WaterSet predicates; the ordinary FV route needs migration and execution.

Ghidra occupant, hut, cursor and Unit/Infantry mission-order annotations were
corrected, saved and read back exactly. The checkpoint and validation receipt
record their local readback files and the final candidate checks.
