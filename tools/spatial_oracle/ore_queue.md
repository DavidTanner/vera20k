# Enqueue-side ore queue rebuild comparison

`ore_queue.py` executes the pinned retail `gamemd.exe` through the existing
[`harvest_field` fixture](harvest_field.py). It supplies prior map/queue state;
the enqueue, rebuild, Cell predicates, iterator, heap operations and Scenario RNG
run original instructions. `ore_queue.json` contains 60 cases: 52 direct enqueue
calls, three full `CellClass::Reduce_Tiberium` callers, and five full
`TiberiumClass::GrowthProcessor` callers. The sidecar records executable identity,
Unicorn version, payload hash and fixture boundaries.

```sh
RA2_DIR='/path/to/retail/Yuri' python -m tools.spatial_oracle.ore_queue --check
```

The default is read-only. `--write` deliberately publishes fresh original-execution
results; it is not a way to rebaseline Rust output. The Rust comparison lives in
[`queue_oracle_tests.rs`](../../src/sim/ore_growth/queue_oracle_tests.rs) under the existing queue owner.

## Executed behavior

The declared successful native `[Map] Size` is 4 by 4. Original
`SurfaceDataCount @ 0x0042B1F0` returns 64; original
`ToSurfaceIndex @ 0x0042B1C0` establishes the saved coordinate/bitmap mapping.
The retained backing table and Cell vtables come from the shared fixture. The
original iterator starts at `(1,4)` and stops at its first null after the allocated
diamond. This comparison does not infer iteration order from Rust.

| Entry | Executed threshold and order |
| --- | --- |
| `AddToGrowthQueue @ 0x007235A0` | Density below 11 admits the request. Array counter 53 and 54 append normally; 55 invokes `RebuildGrowthQueue @ 0x007233A0` before appending. This is signed `counter > capacity - 10`, independent of the heap reference count. |
| `AddToSpreadQueue @ 0x00722AF0` | `CanSpreadTiberium` and the receiver's bitmap admit the request first. Array counter 43 appends normally; 44 and 45 invoke `RebuildSpreadQueue @ 0x007228B0`. This is signed `counter >= capacity - 20`. Rejected requests leave old queues untouched. |
| Both rebuilds | Reset the receiver's array counter, heap and whole bitmap, then iterate cells and seed eligible cells of the receiver's own type at priority bits zero. Draw no RNG and retain every class timer. |
| Both enqueue callers after a rebuild | Append the already-admitted request and draw once. They do not repeat admission or bitmap deduplication, even when the rebuild already seeded that coordinate. |

The supplied nonzero percentage is 0.125, exactly representable at the native
INI reader float-to-double boundary. The Rust fixture uses that production reader;
this comparison isolates queue behavior from decimal parsing.

Controls cover empty and late-reference heaps, stale bitmap clearing, same-class
duplicates, cross-class receivers, a receiver with no matching overlays, densities
0/1/10/11, occupied and sloped cells, growth/spread Scenario gates, zero percentages,
and seeds 1/31/42. Growth admission is a density-only gate even when rebuilding
finds no eligible cells because growth is disabled; spread admission uses the
request cell's own type and percentage, while the rebuild uses the receiver class.
The occupied `(3,4)` cell is retained by growth reseeding and rejected by spread
reseeding. Four classes' queue and timer state is captured before and after every
case, including classes unaffected by the call.

The full-removal caller controls establish a material same-turn difference:
removing an empty Gem cell enqueues all eight Ore neighbors into the Gem receiver
and draws eight times. Removing an empty Ore cell first rebuilds and seeds all
eight Ore neighbors, appends the first admitted north neighbor again, then skips
the other seven neighbors through `Reduce_Tiberium`'s outer bitmap gate. That call
draws once. A density-11 partial-removal control calls growth admission before
reducing density; its initial density guard prevents a rebuild and any draw.

The processor controls execute `GrowthProcessor @ 0x00722F00`,
`GrowTiberium @ 0x00483710` and existing-cell `PlaceTiberium @ 0x00487190`.
The budget draw precedes growth; Place synchronously enqueues spread and can
rebuild its receiver before the processor draws its growth-reinsert priority.
The later duplicate spread admission draws nothing. A growth append counter of
55 becomes 57 through direct processor reinsertion while the spread counter
rebuilds from 44. Processor reinsertion does not call AddToGrowth's array-counter
guard. An `INT_MIN` budget draw still removes the first heap root before returning
without growth or spread.

Signed raw RNG controls 0, `INT_MAX`, `INT_MIN` and -1 execute the original
absolute/remainder instructions. They replace one word in the seeded RNG data
state, never a function return. Every case records draw count, actual draw words,
full retained RNG-state SHA-256, indices and four subsequent original draws.
Frame controls include negative frames, `2^24+1`, `2^24+4`, `INT_MAX` and `INT_MIN`.
The original `FILD i32; FSTP f32` uses the fixture's native round-toward-zero state
(`0x0E7F`). For example, frame 16777220 and seed 1 produce integer priority
16777251 but store bits `0x4B800011` (16777250), whereas a nearest f32 conversion
would store `0x4B800012`. This is an executed conversion result.

## Inputs and limits

The successful Resize diamond, adequate queue storage, prior array counters and
heap references are supplied state. The fixture represents history with popped
entries still present in the array; it does not simulate a multi-hour game to
reach these thresholds. Its four synthetic Tiberium instances use the shared
Riparius/Cruentus overlay registry, MaxDensity 12 and declared per-case
percentages. Constructors, INI/MIX loading, allocation failure and whole Scenario
startup/scheduling are outside this comparison.

The shared fixture answers OS Interlocked imports and presentation sinks.
`Cell` rectangle queries at `0x0047FDE0`, `0x0047FB90` and `0x0047FF80` return empty
rectangles; radar/tactical dirtiness are service sinks. The full-removal
RecalcAttributes sink publishes declared bare land. No admission, growth, density,
queue, rebuild, iterator, timer or RNG return is substituted. Existing-cell Place
does not enter the new-overlay constructor/allocator branch in these processor
controls. Timer padding is declared zero and has no independent semantic claim.

Native execution establishes these bounded queue/caller and numeric cases. The
Rust corpus comparison and separate production integration checks establish their
Rust implementation. This fixture does not certify complete ore economy, whole
map load, visual output, arbitrary map dimensions or arbitrary prior histories.

## Rust owner and validation

`OreGrowthState` remains the sole queue/counter/bitmap owner. Both enqueue sites
reuse its existing per-type rebuilds; processor reinsertion retains its separate
native semantics. Spread enqueue takes one explicit receiver, removing the old
implicit-type wrapper. Rebuilding joins spawning terrain, the live ground
occupancy grid and all terrain objects through the existing object view. The
Terrain caller borrows the actual growth/spread flags, so forced placement still
creates new ore while growth-off rebuilds seed nothing. Queue resets retain
backing allocations. Serialized queue/timer ownership and snapshot layout remain
unchanged.

The [validation receipt](ore_queue.validation.json) records the 60-case Rust/native
comparison, the two pre-fix failing regressions, connected production callers,
9,416 passing mandatory-retail library tests (226 ignored), successful clippy,
436 Python checks (4 skipped), and the unchanged simulation field ratchet. These
are bounded executable parity and Rust integration results. Whole Scenario
startup and actual multi-hour histories remain outside the comparison. Other
OQ-38 residuals and the deferred VoxelAnim IsTiberium expiry/ring are separate.

Extending the shared harvest fixture's presentation rectangle sink required
refreshing TIBTRE source provenance. Original TIBTRE execution was repeated and
its payload stayed byte-identical (SHA-256 recorded in the receipt).
