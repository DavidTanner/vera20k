# Bridge-collapse metallic debris

This chain connects `CellClass::BlowUpBridge` (`0x0047DD70`) to the existing
AnimStore constructor, Bouncer flight, contact/landing receivers and cleanup.
It also fixes the shared RulesClass animation vectors and their asset binding;
ordinary death debris consumes the same corrected metallic vector. This document
bounds the evidence for this chain, not whole-bridge parity.

## Rules and assets

The original vector constructors (`0x00665827..0x0066585F`) initialize both lists
empty. `ReadGeneral` reads `MetallicDebris` (`0x0066DA90`) and `BridgeExplosions`
(`0x0066DB93`) through `ReadString` with capacity128. It truncates to127 bytes,
trims the whole buffer, splits only on commas, and calls the original AnimType
factory. A zero-length result retains the previous list. A nonempty result
replaces it, even when comma-only input or exact `none`/`<none>` tokens produce
an empty list. Individual tokens retain spaces; successful duplicate references
retain their order and first registered spelling.

The production retail `RULESMD.INI` value authors20 names but the native read
produces **15**, ending in literal **`D`**. The first14 have ART bodies and
15-frame,30x30 SHPs. `D` has neither an ART body nor an image; its real AnimType
retains constructor defaults and is not a Bouncer. The four bridge explosions
are `TWLT026`, `TWLT036`, `TWLT050`, `TWLT070`, with17,17,17,26 raw SHP frames.
The supplied `LANGRULE.INI` is absent; `MPBattleMD.ini` and `Hills.mmx` omit both
keys and retain the prior lists.

[`RulesPassProcessor`](../../src/rules/native_processing.rs) owns the retained
vectors for one ordered RulesClass processing stack and publishes them through
`ProcessedRulesLayers` to [`RuleSet`](../../src/rules/ruleset.rs). Process-resident
Type registries remain a different owner. GeneralRules and BridgeRules no longer
reparse the merged INI or invent a20-entry default. Configuration hash version7
includes both resolved ordered vectors. Simulation keeps an interned projection
for selecting registered references; map binding and validated restoration
rebuild it from the bound rules, and snapshots omit that derived cache.

Animation roots and both binders preserve literal type identities. The dedicated
animation Image value is read from the exact type's ART section with the native
25-byte buffer/current-ID default (`ObjectType::ReadINI 0x005F933B`). The loader
uses literal type ID only when that value is empty (`0x00427B9F..0x00427BBC`).
Thus `[ FX] Image=REAL` differs from `[FX] Image=WRONG`; an absent Image in the
former section can legitimately trim its default to `FX`. Generic object and
particle image resolution is unchanged. Binding, atlas candidates and smudge
frame dimensions share this animation authority.

The ART-read receipt gives registered unread types constructor metadata before
binding. They enter the scheduler without loading an orphan image. The headless
loader now applies that receipt before binding its ART copy, matching ordinary
and generated app loading. Both atlas candidate generation and the existing
presentation gate reject image drawing for unread `D`.

The oracle inputs now have independent original-reader evidence. The
[ART/SHP corpus](../../tools/rules_oracle/bridge_anim_inputs.md) executes full
AnimType construction and ART/image readers against physical lexical strings
and complete SHP bytes, then checks the supplied production exports. Its24
retail rows include unread D; two asymmetric controls distinguish Scorch from
Crater. The separate landing-input corpus executes selected Rules, Warhead and
Terrain readers through the actual RULESMD/mode/map layers and original ART
Foundation. Archive selection and physical INI parsing remain explicit fixture
boundaries; no Rust-interpreted scalar establishes its own native expectation.

## Producer and consumers

[`spawn_bridge_debris`](../../src/sim/world/bridge_orchestrator.rs) reads signed
Cell.Level and emits at `level * 104 + 416`, including after structural bridge
flags disappear. Each admitted cell takes the original outer gate, X/Y jitter,
metallic gate and metallic selection, constructs that AnimClass immediately,
then draws the sibling explosion delay and selection. The metallic constructor's
RNG and immediate Start therefore precede the sibling draws. Coordinates retain
the native final-absolute-coordinate truncation, including negative cells.

The existing AnimStore remains the only animation lifecycle owner. Its Bouncer
body updates the exact world coordinate; contact walks the landing cell's ground
object list and delegates entity and Terrain receivers to their existing owners.
Landing damage uses the existing area-damage dispatch, including bridge-state
notifications. Destruction removes animation ownership through the existing
lifecycle. No parallel debris effect store is introduced. The joined comparison
below extends producer evidence through the primary animation's flight and
landing. Its empty contact lists do not establish receiver behavior by themselves.

The joined native corpus schedules only the primary metallic animation, starting
at binary frame 1000. The Rust test applies that same boundary: the sibling
explosion, smoke trailers and landing children construct immediately but do not
receive AI visits. It compares every primary visit's exact position, nonzero
velocity and Bouncer scalar bits, world coordinate, frame/delay/loop/timer state, child
construction order, full Scenario RNG state and four continuation draws. Its
32 cases cover four seeds across ground at levels 0 and 4, water, structural deck,
water beneath a deck, mesa, pit and cliff fixtures. Structural deck landing
executes original area-damage admission, including the rejection-sampled draw
that consumes three raw Scenario values in seed 1.

On a Stopped result, native queues the primary twice; original drain `0x00725C70`
removes both entries and calls the scalar destructor once. Rust's idempotent
Destroy queues once and its existing drain finalizes once. The comparison checks
the resulting logical destruction and physical removal, while populated expiry
observers and duplicate-call side effects remain outside these empty-observer
fixtures. Rust's `inactive` flag also represents deferred deletion, so it maps to
native Alive for this comparison, not directly to native field `+0x19B`. Native
quaternion bytes are not represented or compared; these zero-spin SHP animations
do not consume orientation when drawn.

The existing flat-reflection/cliff-zero shortcuts can store `+0` velocity where
native matrix arithmetic stores `-0` ([prior bounded comparison](PHASE3_BOUNCE_GROUND_QUERY_DELIVERY_NATIVE_REPORT.md)).
The joined test excludes only that sign difference on a terminal zero-elasticity
visit. The stop calculation converts velocity to integers, landing consumes the
position, and the animation is destroyed before another physics visit. No
coordinate, outcome, timer or RNG tolerance is introduced; full native body-bit
parity is not claimed. Rust retains its deterministic stored bits for hashing
and snapshots.

Terrain receivers require the native Strength fallback. The original TerrainType
constructor initializes Strength to `-1` (`0x0071DBAC`); ObjectType's exact-key
read retains the current field as its default (`0x005F94D3..0x005F94F3`), and
TerrainType replaces a successful read's remaining `-1` with current TreeStrength
(`0x0071DEC8..0x0071DEDC`). The fresh production reader now implements that
fallback. The [retail terrain inputs](../../tools/spatial_oracle/bridge-retail-terrain-inputs.json)
establish TreeStrength 200, absent TREE01/TIBTRE01 Strength and no relevant
LANGRULE/mode/Hills override for this selected chain. See the
[native reader comparison](../../tools/spatial_oracle/terrain_strength.md).

One required layered-reader discrepancy remains for later work: an already-read
TerrainType retains its numeric Strength when a later layer changes only General
TreeStrength, whereas Rust currently merges type sections and constructs Terrain
types against the final General value. The saved two-pass native rows give 200
after an omitted Strength and 375 after explicit `Strength=-1`. This does not
affect the selected Hills inputs, but it prevents a broader layered Terrain
reader equivalence claim and remains open in the whole-bridge goal.

Terrain object coordinates now have one retained authority in
[`TerrainObjectState`](../../src/sim/terrain_object.rs). Construction centers
the signed map cell and clamps input Z0 against the native ground surface;
structural deck flags do not raise a tree onto the deck. Original TerrainType
placement `0x0071E0D0`, Map ground lookup `0x00578080`, raw coordinate write
`0x005F6940` and GetCoords `0x005F65A0` provide the
[41-row native corpus](../../tools/spatial_oracle/terrain_coordinate.json).
The Rust comparison covers its 40 authored-construction rows; the extra row has
nonzero caller-supplied Z. The retained Z participates in the object's hash and
snapshot version 215. Area collection, direct contact, lethal nested C4 and
terrain presentation consume that retained XYZ after ground changes, instead
of resampling current ground or deck height.

The separate [24-row Terrain contact corpus](../../tools/spatial_oracle/terrain_debris_receiver.json)
executes original Anim contact admission and Terrain ReceiveDamage through
ordinary damage gates, immediate Limbo, ground-list unlink, Logic removal and
deferred deletion. Its Rust regression compares 19 conditional single plain-tree
contact rows, including radius and receiver gates. Direct-only receiver rows,
synthetic two-tree lists and custom SpawnsTiberium with failed allocation remain
native characterizations outside that Rust comparison. The empty-list joined
flight corpus and populated conditional contact corpus are separate evidence;
they do not prove a single native loaded-map flight striking a centered tree.

Native47DD70 does not guard an empty MetallicDebris vector before indexing it.
Rust deliberately skips that invalid-list metallic spawn and continues the
explosion; it does not restore fabricated retail defaults. This is safe handling
of an invalid native input, not a native successful-construction equivalence.

## Live bridge input and landing dependencies

The app previously kept a map-load copy of bridge flags for the tactical
screen-to-cell inverse. Destruction, rebuilding and snapshot replacement updated
the simulation but left that copy stale. All cursor, picker, context/order and
camera-bookmark consumers now query the live `ResolvedTerrainGrid` through the
shared `TacticalBridgeLookup` interface. The app cache and its constructor are
removed; the small per-candidate query adds no whole-map scan or serialized
state. Original `TacticalClass::ScreenPixelToCell` (`0x006D6590`) and its active
picker caller read the current cell flags. The
[36-row state-transition corpus](../../tools/bridge_click_state_oracle/README.md)
compares intact, collapsed and repaired flags in both orientations on the
same level-2 cells. Existing candidate-search and inverse corpora provide wider static
coverage. The production regressions first failed after collapse and real
`PreparedLoad` replacement, then passed using the live owner.

Independent landing-input proof also found that the common HE percentage reader
used nearest-rounded host multiplication where original `0x0075DE2D..0x0075DE48`
uses a stored binary64 `0.01` with PC53/chop rounding. Six retail HE armor factors
differed by one ULP; original damage execution changed 27 of 198 sampled integer
results under those perturbations. The existing `WarheadType.verses_f64` reader
now reuses deterministic `X87Chop53` for that multiplication, respects the native
128-byte string buffer and skips empty comma tokens. Bare numbers use the shared
numeric-prefix scanner, removing a duplicate parser. The
[landing-input evidence](../../tools/rules_oracle/bridge_landing_inputs.md)
records 13 successful original reader cases, four native null-token faults and
all 198 damage outputs. Rust's safe handling of malformed short lists is an
explicit exception. This is a demonstrated gameplay requirement for the existing
arithmetic helper, not a new general x87 emulation layer.

The full-suite radiation expectation also depended on the old percentage
rounding. A separate [six-row original comparison](../../tools/spatial_oracle/radiation_damage_boundary.md)
establishes health 295 for that synthetic heavy-unit fixture; its former 294
expectation was arithmetic-only. It exposes an independent radiation spread and
ReadDouble precision gap (native base59 versus Rust60) that happens to produce
the same selected armored result. That larger radiation mechanism and its
bridge consumers remain required follow-up work; this PR does not claim matching
radiation intermediates.

`GeneralRules` now owns the native constructor defaults for TreeStrength (25)
and ConditionRed (0.5); actual retail reads still produce 200 and 0.25. Terrain
convenience construction derives that same TreeStrength authority. The selected
retail layered reader, original Terrain fields/Foundation, HE/Super and shared
Wake/Splash/C4 inputs have independent native reader evidence. Legacy byte
Verses targeting consumers and general per-pass Terrain retention remain broader
required follow-ups, bounded in the companion document.

The actual TWLT026/TWLT036 landing children bind one-shot Reports
`ExplosionShard`/`Explosion06`. Original Anim destruction releases its Report
handle through `0x00406060` before optional StopSound playback; it lets a playing
one-shot finish. The production correction uses the existing release event and
audio owner instead of truncating the sound with a hard stop. The
[child-sound corpus](../../tools/rules_oracle/bridge_child_sound.md) executes the
original sound registry/readers, ART reference binding and release-versus-stop
bodies. Its production regression follows natural child expiry through ordered
sound publication and arbiter lifetime. Device mixing and native audible waveform
equivalence are outside that comparison. No Scenario RNG, timer, event schema or
saved state is added.

## Reproducible comparisons and validation

All native corpora use Unicorn2.1.4 and original `gamemd.exe` SHA-256
`1cdd1180e49024fbda8ad568caac2e86e856063ff67ab38f62b7d2c7bb84298c`.
The companion `.meta.json` files record inputs, substitutions and payload hashes.
From the repository root, with `VERA20K_GAMEMD_EXE` configured:

```sh
python -m tools.rules_oracle.bridge_anim_lists --check
python -m tools.rules_oracle.anim_image --check
python -m tools.rules_oracle.bridge_anim_inputs --check
python -m tools.rules_oracle.bridge_landing_inputs --check
python -m tools.rules_oracle.bridge_child_sound --check
python -m tools.spatial_oracle.radiation_damage_boundary --check
python -m tools.bridge_click_state_oracle --check
python -m tools.spatial_oracle.bridge_debris_producer --check
python -m tools.spatial_oracle.bridge_debris_flight --check
python -m tools.spatial_oracle.terrain_render --check
python -m tools.spatial_oracle.terrain_coordinate --check
python -m tools.spatial_oracle.terrain_debris_receiver --check
python -m tools.spatial_oracle.terrain_strength --check
```

| Boundary | Native evidence | Rust/production validation |
| --- | --- | --- |
| Constructors and retained list reads | [24 sequential rows](../../tools/rules_oracle/bridge_anim_lists.json), original ReadString/strtok/factory/constructors/vector copy; supplied cached INI indexes. Native `--check` passed. | `bridge_animation_lists_match_original_retained_vectors`, `retail_bridge_animation_vectors_match_original_reader_and_unread_d`; passed with required retail INIs. |
| Image25 and empty-image fallback | [11 rows](../../tools/rules_oracle/anim_image.json), original reader and loader selection; exact section/current default supplied from inspected caller. Native `--check` passed. | `animation_image25_matches_original_reader_and_loader_selection` and exact-token binder tests passed; atlas regression passed after including the existing generic-letter filename retry in its expected candidates. |
| Actual bridge producer and constructors | [Producer corpus](../../tools/spatial_oracle/bridge_debris_producer.json), original47DD70/AnimType ctor/AnimClass ctor/immediate Start; all15 metallic slots exercised. ART scalar/image-header inputs supplied from production export and independently established by the original full ART/image-reader corpus. | Production-reader producer/RNG comparisons and shared death-loop12 rows passed, including seed5 selecting `D`. |
| Primary flight, empty contact lists, landing and cleanup | [32 joined cases](../../tools/spatial_oracle/bridge_debris_flight.json), original primary AI/Bounce/result handling, dry/water/deck landing, area bridge admission and native drain; [explicit boundaries](../../tools/spatial_oracle/bridge_debris_flight.md). Native `--check` passed. | `native_bridge_producer_primary_flight_landing_and_rng_continuation` passed all32 histories through final physical removal. The separate `debris_contact_matches_original_tree_damage_gates_radius_and_retirement` passed its19 covered contact cases. |
| Terrain retained-coordinate rendering | [82 native rows](../../tools/spatial_oracle/terrain_render.json), original Render projection and ordinary static DrawIt through the shape-call boundary, before pixels. | `terrain_retained_xyz_projection_and_piece_z_match_original_render` uses the production projection and static-pair helper, compares draw points with explicit world/dirty-rectangle translation and exact body/shadow gradients and Z-adjust. All82 rows passed. |
| Mixed scheduler and persistence | Native Load reseeds Scenario RNG; the primary-only native corpus does not establish whole-world scheduling. | Final release [force-fire witness](../../examples/bridge_forcefire.rs) on `822161c1` collapsed the exact retail Hills bridge at frame7327 after121 Cannon shells. All3 observed DBRIS Bouncers moved and were removed by frame7385; the next200 frames constructed26 SMOKEY2,1 TWLT026 and2 TWLT036, final state hash `818d1342a839bdbe`. Log `/tmp/bridge-debris-release-witness-822161c1.log`. The explicitly enabled [production restore test](../../src/sim/combat/bridge_live_chain_tests.rs) passed in188.96s from the retained final test executable: all3 restored Bouncers flew and expired, and two validated restores matched all200 state hashes, final `219282f5d7a0f8dd`. Log `/tmp/bridge-debris-restore-822161c1.log`. This is production composition, not native whole-frame proof. |
| Retail loading and rendering | Independently established ART/SHP inputs plus actual AssetManager/map/renderer integration; no native pixel comparator. | Normal release game and example built successfully from the owned `822161c1` source (1m04s). The ordinary app loaded the exact retail map, accepted the fresh collapse save through its in-game browser, and captured [explosions](bridge-debris-captures/collapse.png) and [flying debris/smoke](bridge-debris-captures/flight.png) through normal Shift+S. [Metadata](bridge-debris-captures/metadata.json) pins binary, save and pixel-identical PCX/PNG hashes. |
| PR candidate checks | No claim from a parser pass alone. | Retail-required full `cargo test -p vera20k --lib` on `822161c1`: **9535 passed, 0 failed, 148 ignored** (65.03s), log `/tmp/bridge-debris-full-lib-822161c1.log`. `cargo clippy -p vera20k --lib` completed successfully with961 warnings (18.80s), log `/tmp/bridge-debris-clippy-822161c1.log`. Both explicitly used the owned worktree; the retained test executable SHA-256 is `91f222ba8b50db66afc2c6e5b59eb62e410eaa4fb4d2e7f1cfdb97eaac24f000`. The one fresh read-only critic follows these implementation and validation results. |

The earlier research266-row fixture supplied20 metallic types,16-frame images and
three synthetic explosions. It established only that supplied producer/constructor
composition. It is **not retail evidence** and is superseded for retail selection
and constructor continuation by the15/4 corpus above. The older
`anim_bouncer_launch` fixture remains bounded evidence for its explicit supplied
inputs, not the shared production MetallicDebris pool.

Still-unproven boundaries include native physical INI and asset loading as one
joined run, native GPU output, complete custom animation lifecycle branches, and
all bridge mechanisms outside this producer chain. The producer fixture
supplies map lookups, marking/display callbacks, allocations, ART values and
reference pointers; it omits DBRIS1LG's trailer pointer and delayed explosion
Start behavior from its producer claim. The joined flight fixture binds the
actual smoke trailer, wake and splash references, but still leaves all child AI,
audio Report effects, populated destruction observers and actual combat-light
creation outside its execution boundary. These limits are not resolved by
matching RNG snapshots or passing the library suite.

The production animation renderer reads each Bouncer's updated exact world
coordinate, selects its current frame and native display layer, and applies the
world-Z projection. An ordinary visible Hills collapse and a GPU screenshot are
the concrete rendering-validation route; the existing fixed tactical capture
does not drive this chain. Pixel equivalence is not established here. A separate
bridge rendering follow-up remains in the projectile screen-position consumer:
it treats a raw Z lepton value as a clamped map level. That consumer is outside
this animation producer chain.

### Ordinary-menu map identity and restore prerequisites

The chooser's stock “Head for the Hills (2-4)” entry resolves through
`MISSIONSMD.PKT` to `XHills.MAP` in `multimd.mix`. Its parsed content hash is
`db1b6e84c56bf558`; it is a different map input from the loose `Hills.mmx`
used by the production witness (`c045c269668aa87e`). In particular, the
loose map has a `CABUNK01` override absent from the stock menu map. The
snapshot validator correctly rejected that first cross-map load.

For the visible check, the exact 144458-byte map payload inside `Hills.mmx`
(SHA-256 `780d5d6e6d3c81ac0d510a5df326848dd426b3d840ac290114f44faf8eab9e9e`)
was exposed as the task-owned loose file `bridge-debris-hills-20260926.yrm`.
The example's optional third argument selects this same file from the beginning
of the run, preserving its filename as well as its content and rules hashes.
No snapshot metadata or validation gate is rewritten. The Terrain input exporter
also now uses the production map loader for MIX-wrapped `.mmx` files; its
33 selected input rows remained byte-identical after that correction.

The ordinary release app accepted the resulting collapse snapshot, exposing
three restoration prerequisites before rendered validation could be claimed:
the app retained the outgoing player's owner name despite restored
`ScenarioSession.current_house`; derived animation IDs came from the outgoing
interner; and headless construction omitted rule-name interning needed by later
UI consumers. The current-player identity is saved native state
([original caller/save/load evidence](PHASE3_CURRENT_HOUSE_IDENTITY_GHIDRA_REPORT.md)),
not a first-human-House selection. These prerequisites are part of this chain's
save/load delivery.

The app now projects its owner from the live simulation's saved current House;
the separate `MatchState` player-name pin and its launch writers are removed.
Selection and debug overrides remain fallbacks only for development worlds
without that identity. A real `PreparedLoad` regression first reproduced the
stale-owner result with two human Houses and the saved player registered second.
It exercises the app owner query, revealed-cell knowledge and queued command
ownership after committing the replacement world.

Map binding and restore share one simulation-owned animation-list resolver,
which interns the bound rules' ordered names in the receiving world. Restore
also adds missing unspawned rule names before building type handles, preserving
every existing saved string ID. The headless loader now performs the app's
pre-launch rule-name and handle binding. Separate regressions first reproduced
foreign explosion IDs and missing unspawned type identities through the real
load transaction. The clean full library run on `822161c1` includes these fixes; focused
persistence63, current-owner2 and snapshot94 regressions also passed. The final
release, production restore and visible captures above exercise the corrected route.

Saved house color schemes and separate display names remain a required later
persistence mechanism: they are absent from the saved House/session state, so
cross-player loads can retain outgoing remap/score presentation. Assigning the
outgoing color to the saved name would invent missing state. This limit affects
Techno remap and score presentation; the selected debris uses the global
animation palette. The whole-bridge goal remains open.

Main-menu loading is also a required separate persistence/loading mechanism.
`load_saved_game_from_menu` currently reports an error without reading the save;
this differs from the working same-content in-game restore validated here.
Original `Load_Game_Content_From_Stream` (`0x0067E730`) restores saved Scenario,
Rules and object state, initializes saved side/theater resources, and rebuilds
presentation. It does not enter the fresh scenario/overlay-Mark path. Cold
resource admission, saved content identity and process-state handoff therefore
need their own complete implementation; routing the shell button through fresh
scenario construction would not establish resume parity. The existing
[shell evidence](https://github.com/YuriPlanet/vera20k/blob/e83df3de8bf2d9fa1ba6d7c4a9e70b5236b7294a/docs/research/shell/2026-09-25-load-saved-game-evidence.md) records this boundary.

### Ordinary release capture

The final normal release built from `822161c1` restores the saved current House
and displays the bridge world correctly. The in-game load browser accepted
`bridge-debris-candidate-20260926-172204.bin`, generated from the exact map by the
release witness. Normal Shift+S captured the GPU output during
[collapse explosions](bridge-debris-captures/collapse.png) and
[debris flight](bridge-debris-captures/flight.png). The latter shows a moving
chunk and smoke trail across the destroyed section. The executable SHA-256 is
`4d741e8bdd34dd5996513b6617cc711dcb4266884a8c3f52f33ede8a3a737023`.
[Capture metadata](bridge-debris-captures/metadata.json) also pins the save,
map/rules hashes and pixel-identical PCX-to-PNG conversion. This is a production
rendering witness, not a native pixel comparison.

The saved MCV is at (136,79); H centers it. With the game window raised, normal
northwest edge scrolling reaches the tank near (64,72); selecting it and pressing
F centers the bridge view. The camera stays in place when the collapse save is
loaded again. The final captures include the live inverse, sound-release and
native-input corrections and the latest integrated Ground-key implementation.

### Independent review

One fresh read-only critic reviewed `474b8646` against `d8af4f6e` after the final
implementation, validation and production capture. It established no blocking
gameplay defect in this bounded chain. The critic independently passed 11 tests
from the hash-verified retained candidate executable and the native
`terrain_coordinate`, `terrain_debris_receiver`, `bridge_debris_flight` and
`bridge_child_sound` checks. Its extra-EOF-line finding was corrected and the
sound corpus rechecked. A suggested single-cell helper could remove a small
temporary set allocation; no measured performance issue or gameplay defect was
established, so this optional cleanup does not alter the validated candidate.
The broader native-comparison boundaries and required whole-bridge follow-ups
above remain open.
