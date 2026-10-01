# Native tactical fast scrolling

`fast_scroll.py` executes the pinned retail `gamemd.exe` through the shared
[`native_oracle`](../native_oracle.md) loader and checked runner. It never reads
VERA's camera implementation or computes a second scrolling algorithm. Its JSON
contains native observations, and the sidecar identifies the executable,
Unicorn version, producer/shared-runner hashes, substitutions and payload hash.

```sh
python -m tools.input_oracle.fast_scroll --check
# Only to explicitly regenerate and review original-executable outputs:
python -m tools.input_oracle.fast_scroll --write
```

Configure `VERA20K_GAMEMD_EXE` or `RA2_DIR` as described by the shared runner.
Accepted executable SHA-256:
`1cdd1180e49024fbda8ad568caac2e86e856063ff67ab38f62b7d2c7bb84298c`.
No retail executable is included. Help does not require retail configuration.

## Executed coverage

| JSON collection | Coverage |
| --- | --- |
| `cases` | 760 full `Tactical_RightDrag_Pan` calls at `0x00693440`: 742 method-0 cases and 18 method-1/2 characterization cases |
| `sequences` | Six successive original calls retaining only the same fixture's state: threshold crossing, a held stationary displaced pointer, return to anchor and a tiny reversed displacement |
| `update_cases` | 11 full `ScrollClass__UpdateMouseScrolling` calls at `0x00692F30`, including the original call into right-drag, logical-button priority, capture, input lock, planning-handler consumption and edge-scroll routing |
| `message_cases` | 18 full tactical receiver calls at `0x006930A0`: right press/release, admission gates, capture change and inert middle-button messages; original cleanup `0x004AEAD0` and band-rectangle clearing `0x006DA160` execute |
| `throttle_cases` | 37 calls from original throttle entry `0x0055E160` to `0x0055E33B`, before unrelated FPS bookkeeping: modes 0..6, remaining waits 0/6/10/11/20 and inactive-app/game-state gates |
| `camera_sequences` | Ten retained-state request/commit/absolute-view sequences, including original full drag into `Scroll_Map` and the original directional request, clamp, Tactical AI and instant SetView bodies |

The principal body executes original threshold tests, displacement arithmetic,
edge boosts, Options getter `0x005FBF70`, direction helper `0x0075F230`, table
trigonometry and `Math__ftol` `0x007C5F00`. The four probe calls and subsequent
horizontal-then-vertical scroll requests are recorded in their original order.
`expected.motion` only decodes the signed requested distances from those calls;
it is not map movement after clipping. The native cursor table at `0x0083E790`
is retained for all 16 supplied direction-block masks.

All seven retail scroll rates are exercised with both signs, both axes,
diagonals, threshold boundaries and each viewport edge. Additional cases cover
anchor positions 9/10 and width-or-height minus 10/minus 9, stationary anchors
at the exact boundaries, custom OS drag metrics, active-game gates and supplied
band-box states. Of the 760 cases, 294 explicitly exercise reciprocal arithmetic:
divisors 1..7, quotients 1/2/3/7/49/100/1000, exact multiples and their two
neighbors, in both signs. These include synthetic pointer displacements outside
the supplied viewport to characterize arithmetic, not desktop cursor reach.

## Established native behavior

The active caller chain was checked against original instructions:
`Main_Tick` calls `GScreenClass::Input` at `0x0055D8AB` with the display singleton;
its call through slot `+0x28` at `0x004F43D9` reaches the MouseClass body
`0x005BDDC0`, whose call at `0x005BDF1F` reaches `0x006922E0`. That dispatcher
calls `0x00692F30` at `0x006922E3` before command-bar input. Inside the captured
right-button arm, `0x00692FD2` calls `0x00693440`. The earlier gadget processing
does not conditionally skip this slot call when a gadget consumes input.

The tactical mouse receiver handles `WM_RBUTTONDOWN` (`0x204`) and
`WM_RBUTTONUP` (`0x205`). An admitted right press stores the tactical-relative
anchor at `Display+0x5550/+0x5554`, clears the threshold byte `+0x5558` and takes
the shared capture `+0x555A`. Its switch has no middle-button case. Presses do
not layer a second capture over an existing one.

The per-frame update subtracts tactical viewport offsets from the current
pointer, then checks `FUN_0063AB60`. That function is a **planning-mode handler**,
not a sidebar hit-test: its body checks planning mode/node globals. With capture
set, logical button 1 is tested before button 2. Left therefore routes to the
band-box update; right routes to `0x00693440`. Ordinary edge/coast processing is
on the uncaptured branch. The supplied planning-consumption and logical-button
results are declared boundaries, not execution of those input subsystems.

The drag threshold is strictly greater than twice each supplied
`SM_CXDRAG/SM_CYDRAG`, not greater-than-or-equal. Method 0 measures displacement
from the press anchor every call, so holding a displaced pointer continues to
scroll. An outward anchor within the native 10-pixel band substitutes at least
five pixels of displacement and multiplies by four before rate scaling. The
stationary edge tests are asymmetric: X compares against `width - 1`; Y compares
against `height`. Method 0 uses original double `1.0` at `0x007E1718`; methods
1/2 use double `12.0` at `0x007ED8D8`, warp to the anchor and, for method 2,
reverse signs. Those alternate methods are characterized only.

A release whose threshold was crossed skips the cancel/deselect callback;
an ordinary captured right-click release reaches it. Both run the original
cleanup and release capture. Native engagement `+0x554C` remains set across
completed drags and subsequent presses. Its value is not a required equality
for VERA's internal lifecycle: in the ordinary right-only path, with no active
band box, it cannot change the movement result. Synthetic active-band-box
inputs in this fixture do not establish a reachable mixed-button lifecycle.
Likewise, `WM_CAPTURECHANGED` runs native cleanup but does not itself clear
`+0x555A`; this fixture does not certify the rest of Windows capture delivery.

The throttle's mode branches at `0x0055E1AD` and `0x0055E1B6` send modes 0 and
5 to `0x0055E2B4`, which waits without calling `GScreenClass::Input`. The other
mode path can call Input at `0x0055E253` only when the supplied remaining wait
exceeds 10 and its active-state gates pass. The offline waiting interval cannot
justify scrolling once per arbitrary render refresh. Clock units, timer setup
and the caller's frame cadence require their separate caller evidence; the
fixture supplies clock sequences and observes this original admission logic.

## Arithmetic and x87 state

Before every drag fixture, original WinMain instructions
`0x006BBFB7..0x006BBFCE` execute `_controlfp` and cache its result through
`0x007C5EE4`. The supplied process-start word is `0x027F` (PC53, nearest).
Unicorn then reports live/cached `0x0E3F`: PC53, toward zero, all exceptions
masked, with reserved bit 6 clear. The shared runner's legacy `0x0E7F` differs
only in that reserved bit. The fixture retains the executed startup result
instead of substituting a claimed universal control word. Original `ftol`
observes this cached word and runs unchanged.

Native method 0 first rounds `1 / (ScrollRate + 1)` toward zero at 53-bit
precision, then multiplies the integer displacement with the same rounding and
converts toward zero. Therefore `3 / 3` requests zero pixels, `6 / 3` requests
one and `49 / 7` requests six. Ordinary nearest-rounded f32/f64 division or
reciprocal multiplication does not reproduce these exact-multiple cases.

For divisors 1..7 and a non-overflowing signed 32-bit boosted displacement, the
equivalent integer result is truncating division, with the magnitude reduced by
one for a nonzero exact multiple of 3, 5, 6 or 7. Powers of two have exact
reciprocals. For the other divisors, their positive reciprocals round strictly
below exact; toward-zero multiplication cannot raise an exact multiple back to
its integer boundary. The reciprocal and multiply error for a magnitude at most
`2^31` is below `2^-20`, far below the smallest nonzero remainder fraction
`1/7`, so no non-multiple crosses its lower integer boundary. This is a bounded
arithmetic proof, corroborated by the 294 executed boundary samples, not a claim
for arbitrary scroll settings, overflowed native shifts or all x87 functions.

## Accumulation, commit and absolute view

The camera sequences retain the native `Scroll_Map` body `0x004A9840` and its
directional request `0x006D8530`. The latter adds movement to requested center
`Tactical+0xD74/+0xD78` without changing committed center `+0xD64/+0xD68`.
Original `Tactical__AI` `0x006D2540` compares those pairs and, in its block
`0x006D26F9..0x006D2770`, clamps once through `0x006D8640`, then writes the
result back to both pairs. Opposing requests therefore combine before the
boundary is applied.

The fixture sets `Tactical+0xA8` equal to the current frame (both zero), which
selects this full original AI commit path while skipping unrelated per-frame
animation/timer processing. It supplies MapWidth 100, LocalSize `(0,0,100,100)`
and viewport `(640,480)`; these are explicit geometry inputs, not a claim of
loading a particular retail map. Initial centers are obtained by executing the
original clamp on supplied seed points, not by calculating golden bounds.

For example, the native west-boundary seed clamps to center `(-2680,2000)`.
An east request of 21 followed by the original drag's west request of 25 leaves
the committed center unchanged and requested center at `(-2684,2000)`. One
original AI commit produces `(-2680,2000)` in both pairs. The separate-commit
control inserts an AI commit after the west request, then another after the
east request; original execution now ends at `(-2659,2000)`. That control is
deliberately a different schedule and demonstrates why immediate per-producer
clamping changes the camera position.

The keyboard request of 21 is a supplied producer output (its original constant
is at `0x0082A030`); its input producer is not executed by these rows. The mouse
request of 25 is produced by the complete original drag body from a 100-pixel
anchor displacement and scroll rate 3. In the main tick, the keyboard scroll
calls at `0x0055DD42..0x0055DD96` follow AI/follow processing, so their request
can remain pending until the following mouse poll and AI commit. The fixture
preserves that request across calls; it is not a full tick replay.

The absolute-view rows execute full `SetViewToCoordInstant` `0x006D6070`,
including its original projection, `AdjustForZ`, clamp and writes to both center
pairs. A pending movement is overwritten, including when the new absolute view
equals the already committed center. A later AI commit does not resurrect that
movement; a fresh subsequent scroll request still applies. These rows use Z=0
and the established native projection multiplier from
[`bridge_click_oracle.py`](../bridge_click_oracle.py). They establish the setter's
overwrite behavior, not selection/follow target choice or new height coverage.

The JSON stores native **center** coordinates. The derived viewport updater
`0x006D8B30` subtracts half its viewport dimensions to obtain native top-left
coordinates; VERA also has its existing world-row Y bias to account for when
comparing its own coordinate frame. Neither conversion is baked into the golden
values. The only additional camera-sequence sinks are CRT exit registration
`0x007C978A` and the derived viewport updater `0x006D8B30`; request tables,
direction probes, clamp arithmetic and authoritative center writes execute.

## Fixture boundaries and remaining validation

Windows metrics, logical-button observations, pointer position, planning-handler
consumption, click admission, display flush and capture calls are supplied or
observed sinks. In ordinary drag rows, Scroll_Map returns a supplied
direction-block mask and preserves its distance pointer; those rows stop before
map projection, clipping and camera/render side effects. The camera sequences
above retain the original downstream request, probe, clamp and commit instead.
Cursor changes and OS warps are recorded
sinks. The band-box callback receives the original `(-1,-1)` argument and does
not receive invented engagement writes.

The throttle's clock leaves, Sleep, Input, offline/network service, command,
tactical and render calls are explicit supplied/observed boundaries. Its
original branches and wait arithmetic execute; this is not a full game-loop
replay. No RNG draw, simulation timer write or detach call occurs inside the
executed drag body. Throttle input clocks and `0x00A8E314` accumulated-wait
writes are retained; effects behind its declared service sinks are not claimed.

These vectors establish the stated original instruction outputs. Rust parity
requires production-function tests against the JSON, and visible integration
still requires the release game's actual input/render route. They do not
certify GPU pixels, whole-map boundary movement, arbitrary modded settings,
OS-specific pointer capture, or network service cadence.

## Rust integration and validation

The camera controller in `src/app/input/camera.rs` owns drag polling, the
method-zero distance calculation and pending scroll requests. The runtime's
existing `RuntimePassDecision` admits native scrolling once per offline game
frame. Zoom and placement retain their separate presentation cadence; VERA's
developer pause still admits camera input. Networking is not live in this build
and its existing extra-poll policy is not a native cadence claim.

Mouse input is sampled before simulation. At the existing pre-follow seam the
camera consumes the previous keyboard request and current mouse request in one
clamp against this frame's playfield authority. Main_Tick's later keyboard
request remains pending until the next admitted frame. Absolute view changes
(follow, bookmarks, minimap, loading and VERA zoom) discard pending requests
through `set_camera_position`. This prevents an outward request at a boundary
from being discarded before an opposing inward request arrives. The private
`PendingCameraScroll` stores only uncommitted displacement; the existing camera
point remains the sole committed view.

`src/app/input/fast_scroll_tests.rs` compares 738 active stock drag cases, the
six-call retained gesture, right-release cancellation, capture/button priority,
offline throttle admission and all 10 camera sequences against the original
outputs. The camera sequence test enters the production request commit and
clamp with bounds from the production playfield-geometry owner. It converts the
native center to VERA's top-left frame using the verified viewport half-extents
and the existing shared world-row bias. `camera_cadence_tests.rs` also drives the
real local frame pacer to ensure extra redraws cannot add scroll steps or latch
a drag between admitted polls.

Validation on macOS, 2026-10-01:

- Original cadence regression failed before the fix: 193 redraw polls versus
  13 admitted frames in its 193 ms input schedule.
- Canonical native oracle `--check` passed, including an independent critic's
  reproduction of the pending-request/clamp interaction.
- Retail-required full library suite passed (9,401 passed, 225 ignored).
  After the review correction, the affected app suite passed again (959 passed,
  26 ignored), including the native commit and SetView lifecycle sequences.
- The release game loaded Pacific Heights through the ordinary skirmish UI.
  Selection, inert middle click and right-click deselection were observed;
  the user reported that the requested held right-button drag worked.

This is bounded original-executable comparison plus Rust regression and live
integration evidence, not whole-game or whole-map pixel equivalence. Hidden
ScrollMethod 1/2, arbitrary out-of-range INI settings, mixed-button band-box
lifecycle, OS-specific capture delivery and network cadence remain outside the
stock right-drag comparison. The existing pan cursor probes the committed view
rather than retaining the native pre-commit probe, a presentation-only timing
residual at map limits. No simulation state, RNG draw or simulation timer is
introduced by the camera's pending request.
