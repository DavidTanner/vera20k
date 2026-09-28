# Empty-FV paid bridge continuation

`paid_conditional.py` executes the original FV constructor, placement, Attack event,
mission dispatch, Approach, paid Drive movement, MoveSound/FireAt, Bullet flight,
bridge impact, Anim cleanup and live object-list mutations. It reuses the existing
`paid_world.Paid` and `anytown_damage.mission.Mission` owners. Native expected
values come from this execution; the vector generator only selects JSON values
and decodes little-endian actor, Bullet and Anim bytes.

The compact input declares the Scenario identity cursor **before** the actual FV
constructor, the original Scenario seed, GameOptions speed 3 and outside-world raw
RNG calls. The optional Stop command uses original TargetClass `6E6AB0` and simple
Event `4C65E0` constructors followed by full Event Execute `4C6CB0` before Logic 3. It
never writes a mission, target, movement, Bullet or impact outcome. Outside calls
execute original `65C780` and assert the observed word/indices; no RNG return is
substituted. Event-boundary RNG hashes are assertions, never state imports.

This is a selected-owner continuation under declared inputs. It does **not** prove
complete native Scenario startup/population, outside-object/global AI cadence,
keyboard or network event admission, renderer/audio output, every Unit/weapon, or
whole-bridge parity. The physical map/TMP/Cell/Terrain/navigation owner is rebuilt
natively on every replay; the separate native House/population boundary inherited
from Paid remains. The original isolated `paid_world` corpus is preserved.
The separate [two-hut composition](huts/README.md) executes CABHUT construction,
collapse callbacks and admitted repair. It bounds the unaffected huts omitted by
the selected paid loop; full Scenario population and ordering remain outside it.

## Coverage and evidence

The six cases cover healthy, damaged, already-collapsed and repaired concrete
bridges; an explicit seed 3 damaged case that induces collapse; and Stop during
paid pursuit with an already-launched missile. The six native continuations match
the retained production capture in 336 states and all 3,293 raw RNG words
(healthy 535, damaged 559, collapsed 559, repaired 385, damaged-collapse 629,
stopped 626). Independent native-only replay and literal
projection checks pass. Every selected Bullet, Anim and deferred object drains.
The four physical cases use seed 31 and stop after their first-burst effects drain,
with Attack still active and the target retained. The seed 3 actual-collapse and
Stop cases continue through target clearing and the transition to idle/Guard.
The production FV/rules/restore set has five tests, including four snapshot
checkpoints with paired 64-frame continuations. Run it against the physical
retail map, not only the default library suite's non-retail fixtures.

The literal frame projection covers actor position/health, native identities,
mission/queue/status/counter, dispatch/rearm and passive-scan state, destination
versus accepted head, target presence, physical target-cell state, all live Bullet
velocity/arm/guidance fields, all selected Anim runtime fields and three RNG
buffer hashes. The full compressed packet also retains raw actor/Drive/Bullet/
Anim state, complete RNG buffers, original ranged call results, every raw word,
ordered live callbacks, bridge-span snapshots, Event before/after and retirement.
The diagnostic comparison additionally checked full RNG buffers and every raw
word, stream, index and order.

Two discovered defects are protected by executed behavior. After collapse,
Mission_Attack queues Guard while Techno AI still reads committed Attack at
`6FA697`; the later Unit Commence owns promotion. A cleared target is not an early
Guard transition. Stop Event 6 clears target/destination but retains the mission
and accepted Drive head, so its original paid movement and existing missile
continue. Stop does not assign mission 13. `stop_event_sources.py` pins the exact
producer/receiver bytes and vtable/switch links; the conditional Stop case proves
its exercised runtime result. The independent [Temporal Stop consumer](../temporal_stop.py)
also executes the deferred beam-release deadline through the original Attack handler.

Production firing now runs at each Unit's original post-movement slot, before
its second Ready/Commence. That required placing Infantry firing in its own
slot too: otherwise a later Unit could kill an earlier Infantry before its
shot. The [original Infantry caller controls](../infantry_ai_order.md) establish
second Ready/Commence, fear, fire and sequencer ordering; mixed-object production
regressions exercise firing, fatal consequences and Temporal consumers. A new
Inviso Bullet detonates at its later live-list visit, after pre-existing actors.
Infantry's retained firing-latch/Ready representation and the full retained
AttackMove mechanism remain separate required work. This host migration does
not claim to close either mechanism or the remaining global AI compatibility
hosts.

The projection and full packet have immutable promotion digests. The compressed
publisher removes copied lexical INI text in favor of SHA256 and normalizes
checkout references; it refuses changed native output even with `--write`.

## Reproduction

Use the pinned original `gamemd.exe` (SHA256
`1cdd1180e49024fbda8ad568caac2e86e856063ff67ab38f62b7d2c7bb84298c`),
Unicorn 2.1.4 and local retail assets as described in
[`tools/native_oracle.md`](../../native_oracle.md). Set `VERA20K_GAMEMD_EXE`
(or `RA2_DIR`), `VERA20K_ANYTOWN_INPUTS`, `VERA20K_SHRAPNEL_INPUTS` and
`VERA20K_PROJECTILE_RENDER_ASSETS` to the corresponding existing local fixture
roots. No proprietary assets are bundled and no machine-specific path is a
runnable default in this package.

From the repository root:

```sh
python -m tools.spatial_oracle.fv_cell_attack.paid_conditional --check
python -m tools.spatial_oracle.fv_cell_attack.paid_conditional_vectors --check
python -m tools.spatial_oracle.fv_cell_attack.stop_event_sources --check
```

The production consumer is
[`bridge_fv_pursuit_tests.rs`](../../../src/sim/world/bridge_fv_pursuit_tests.rs),
`retail_fv_paid_pursuit_fire_impacts_and_cleanup_match_native`, using the ordinary
retail loader and simulation. Its optional `VERA20K_FV_PAID_EXPORT` diagnostics
support auditing, but are not inputs to the native generator and are not bundled
as goldens.

## Input provenance boundary

`paid_conditional.input.json` contains only declarations, assertion hashes and
outside-call receipts. Its producer catalog records source locations, relevant
source lines and file hashes from retained build `bridge-fv-mission-v11a`, plus
that build’s HEAD/source/binary/manifest identities. These are historical input
receipts, not a claim that subsequent source files remain byte-identical. Actual
test-only Logic-object context classifies the selected FV, its Bullets and new
Anims; their calls remain native, including FV Guard calls after collapse. Only
established outside global producers may have a null object context. Interleaving
an outside call inside the selected native pass is rejected rather than fitted.

The original native runner does not import production diagnostics, the extraction
helper, or Rust gameplay outcomes. Changing this boundary is a new comparison,
not permission to regenerate native expected behavior from Rust.

`capture_conditional_inputs.py` can audit the historical capture without reading
later checkout sources as the old build. It verifies the production export and
build-manifest identities, uses the sealed source receipts, reclassifies calls by
observed Logic ownership, and compares the complete reconstructed input:

```sh
python -m tools.spatial_oracle.fv_cell_attack.capture_conditional_inputs \
  --production-dir /path/to/retained/exports \
  --manifest /path/to/retained/build/manifest.json \
  --check-input tools/spatial_oracle/fv_cell_attack/paid_conditional.input.json
```

A new capture uses `--declarations declarations.json --out new-input.json` in
place of `--check-input`. It requires the current source fingerprint to match its
retained build. Both modes remain separate from native execution and projection.
