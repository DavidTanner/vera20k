# Ordinary tactical shroud ABuffer evidence

`shroud.py/json/meta.json` execute the original reset, SHP access, cell-edge
selection and tactical ABuffer producer against physical stock `SHROUD.SHP`.
The sidecar records executable, emulator, source and payload identities. This
is the ordinary `FogOfWar=no`, empty-AlphaShape-list route; it does not establish
all vision lifecycle state or the wider fog/AlphaShape mechanisms.

```sh
PYTHONDONTWRITEBYTECODE=1 python -m tools.procedural_drawing_oracle.shroud --check
```

Use the executable/environment described in [`native_oracle.md`](../native_oracle.md).
The existing `rally.Rally` owns Surface/ABuffer setup and `stock.mix` owns retail
archive extraction. There is no second rasterizer producing the golden pixels.

## Concrete prerequisite correction

Stock `ra2.mix/conquer.mix/SHROUD.SHP` has SHA256
`9818f7e2685fb1e8ed75ceff568aaf5d36350c747cbd4f30856458bba12e4964`, canvas60x30
and96 frames. The selector uses frames0..46. Each of these47 frames has exactly
the same900-pixel nontransparent diamond; their only other pixels are254(skip).
Frame15's drawn pixels are **2**, not0. Frame0's drawn pixels are127.

Original `47EFE0` writes each source byte directly as a u16 A value except254,
which preserves the destination. Original `4801F0` chooses frame15 when
`6D8700` returns-2. Executed flags `Cell+0x12C=0` produce cache-2/frame15 and
900 A=2 pixels. A generated black diamond therefore changes native rally pass
admission: `4C0750` distinguishes A=0 from A!=0. For ordinary palette blitters,
small nonzero A can still choose a black palette row; it is not interchangeable
with A=0 for procedural consumers.

The prior Rust diamond's scan intervals match frame15's900-pixel mask exactly.
Row0 is empty (the prior comment claiming an added tip was incorrect); rows1..15
expand4,8,...,60 pixels and rows16..29 contract56,...,4. Reuse the loaded stock
frame15 in the existing CPU frame blitter and remove the generated substitute.
That retains one asset and drawing owner and also respects replacement assets.

## Executed coverage and schema

- `leaf_cases`:73 original47EFE0 executions: all47 selected stock frames,
  left/top/right/bottom/offscreen origins, four interior clips and two circular
  storage offsets. `input.point` is the native top-left frame origin;
  `input.clip` defaults to `[0,0,160,120]`. `frame_rect` is returned by original
  69E7E0. `alpha_hex` contains19200 logical row-major A bytes, without padding.
- `selector_cases`: all256 neighbor masks with center flags0x18 through whole
  4801F0/6D8700. `caches` are signed Cell+120/+121 bytes; `frame` is the actual
  47EFE0 argument. Sixteen `flag_controls` cover0x00,0x08,0x10,0x18 with masks
  0x00,0x01,0xAA,0xFF. The two bitmask domains remain distinct from Rust's compact
  presentation bitmap.
- `scenes`: nine whole6D3660 executions. Full redraw reaches6D71E0; dirty-cell
  cases reach480A30/6D1F10. Cases cover clear, unrevealed, a revealed factory
  frontier, an island, integer camera changes, circular storage wrap, flat and
  raised dirty cells, and a separately bounded partial0x10 flag state.
- Every reset runs original4112D0 and asserts all A values become127. SHP rect,
  SHP data, clipping, circular-row access and pixel stores all execute original
  instructions. Only platform Surface storage comes from the existing native
  BSurface fixture; there are no function-result substitutions.
- The factory-frontier scene feeds the resulting original ABuffer to original
  rally6DA9D0/4C0750 and records `rally_passes`. Factory/selection/color fields
  are the existing rally fixture inputs, not a complete Scenario constructor.

Production tests should load the SHA-identified asset through `AssetManager`,
`ShpFile` and `extract_shp_brightness`, then compare the shared CPU frame/rebuild
owner's bytes to this corpus. Synthetic GPU tests alone do not establish these
A inputs. Container hashes are in `stock`; this oracle reads the named physical
archive, not a complete native archive/loose-file override resolver.

## Projection, ordering and clipping

Dirty-cell6D3660 calls GetCenter480A30, forces Z=0 at6D36C0, projects with6D1F10,
subtracts integer camera+0xB0/+0xB4 and centers the frame by(-30,-15). Full scan
6D71E0 executes the original inverse-matrix constructor values and projection.
The same Cell10,20 at native camera[-320,440] is drawn at[-10,10]; the raised
level8 dirty control has identical A bytes to level0. Rust's existing world
coordinate convention adds15 to camera Y, so its matching camera is[-320,455].
Fractional camera/zoom behavior is a separate presentation extension.

The native full scan walks diagonal screen rows; dirty cells use queue order.
Rust walks `ry` then `rx`. For all47 selected stock frames, the common mask has
zero intersections with its lattice neighbors at offsets(30,15),(-30,15),
(0,30),(60,0). Thus the order cannot change this ordinary shroud-only result.
Every full scene also replays the exact original per-cell calls through
4801F0/47EFE0 in Rust row order and with full-plane clips; byte equality is
asserted and recorded as `row_major_full_plane_replay_equal`. This establishes
that native per-cell clipped rectangles and full-plane CPU blits agree for
these cases. It does not establish fog/AlphaShape blend ordering.

Frame0 writes127 instead of being empty. Skipping it is equivalent only after
a full127 reset with the established disjoint stock masks. Incremental native
redraws must still execute the write; this is not a general frame0 skip rule.

## Remaining boundaries

The partial `0x10` state chooses frame0 even though bit0x08 is clear. It is saved
as a raw consumer boundary, not claimed to be an ordinary reachable state in
this chain. Existing `shroud_current_sight` evidence establishes selected
counter/open/pending transitions but explicitly excludes complete edge-cache
outputs. Its shared FogState/vision owners remain the lifecycle authority.

Dynamic AlphaShape `420F40/421350` can further alter A; enabled fog uses47F250
and FOG.SHP under the scenario0x1000 gate. Those writers, incremental dirty
invalidation, runtime scroll production, map-edge dummy state, mod assets and
full reveal traversal are outside this ordinary-input corpus. No full-game or
whole procedural-drawing closure follows from these vectors alone.
