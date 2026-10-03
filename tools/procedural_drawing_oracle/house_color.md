# House presentation color evidence

`house_color.py/json/meta.json` execute the pinned retail executable for all 21
physical `RULESMD.INI [Colors]` entries. `--write` and an independent `--check`
passed. The sidecar records binary, emulator, source and payload identities.

```sh
PYTHONDONTWRITEBYTECODE=1 python -m tools.procedural_drawing_oracle.house_color --check
```

Use the configured executable/environment in [`native_oracle.md`](../native_oracle.md).
The existing palette oracle owns conversion; this fixture adds caller data and
uses its original scalar, CMOV and MMX paths. No conversion is copied into Python.

## Established producer

Original `50B840` chooses House+16054's ColorScheme (negative becomes5), reads
scheme+330 through converter+30C's lookup table+174, and stores unpacked RGB at
House+56F9..56FB. ColorScheme constructor slice68C769..68C7DC sets lookup index16.
In RGB565 the unpack clears the channel low bits; it does not replicate them.
Campaign reader500B40 repeats the conversion at500DF7..500ECC after `Color=`.
The fixture executes whole50B840 and that campaign block, and preserves adjacent
bytes including the distinct House+56FC laser RGB.

The table is not a direct RGB565 pack of the generated ramp. InitColorSchemes
66D3A0 constructs N1 then N53 variants per physical entry (calls66D444/66D45B).
ReadColorString474A90 skips N1. The N53 constructor's neutral input is1000 per
channel; Convert48E740 sets +174 to +170 plus `((N-1)>>1)*512`, giving row26.
These variant/read/pointer claims are instruction evidence. The fixture executes
original556090 to produce all table bytes, then passes the actual row26 bytes to
original House/campaign consumers. Their scalar, CMOV and MMX results agree for
all21 entries. Original constructor/ramp and original474C70 numeric INI parsing
also execute; lexical extraction merely supplies the physical key/value strings.

The existing Rust `PaletteLight::color_scheme([1000;3],1000).rgb565(color,16,127)`
is the shared conversion owner for this result. Preserve raw HouseColorRamps for
palette, atlas and loading consumers. A shared presentation lookup should derive
House color through that owner, with no second scalar formula.

Four stock inputs differ from direct ramp packing:

| Entry | Raw ramp[0] | House RGB bytes | Native RGB565 | Direct pack |
| --- | --- | --- | --- | --- |
| Gold | 248,250,70 | 240,248,64 | F7C8 | FFC8 |
| Grey | 128,128,128 | 120,124,120 | 7BEF | 8410 |
| DarkBlue | 74,128,207 | 72,124,200 | 4BF9 | 4C19 |
| NeonBlue | 162,124,233 | 160,120,232 | A3DD | A3FD |

The sidecar bounds the fixture: physical base INI only; supplied native INI
caches, palette outside16..31, registry, converter pointer, surface format and
constructor inputs; no archive/layer loader, whole campaign reader, active table
rebuild/effect state, renderer or GPU equivalence. Six distinct lookup-index
controls and negative-index fallback distinguish the lookup from an accidental
matching packed word. This is not an exhaustive arbitrary-HSV proof.

## Affected consumers and separate paths

Rally6DA9D0 reads House+56F9. Radar655C50's ordinary tracked-object path loads
owner+21C at655F50, may resolve a virtual owner override at655F5F/655F70, then
reads House+56FB/56FA/56F9 at655F86/655F96/655FB8. It packs those RGB bytes and
passes the packed result to the radar surface at656053..65608C. Active callers
are RefreshRadar657CE0 at657D77 and Update656EC0 at6574C3/657520. This is original
body/caller evidence; the whole radar path was not executed by this fixture.
`render/minimap_helpers.rs::owner_dot_color` now calls the same House presentation
owner as rally. Focused Rust comparisons passed all21 stock colors and the
radar-dot lookup; whole native radar execution remains outside this fixture.

The other production raw-ramp[0] reader is `app/input/messages.rs`. It is a
separate mismatch: TypeSelect732950 supplies its local House's scheme index to
Add_Message5D3BA0 and TextLabel72A440. TextLabel draw72A4A0 resolves the scheme
from label+34;72A565 loads scheme+308,72A56B calls HSV_To_RGB517440, and72A638
passes the unquantized RGB to text623880. Scheme+308 holds the original input
HSV (constructor68C710). Thus messages need unmodulated scheme HSV conversion,
not House+56F9 or ramp[0]. Keep that later chain distinct. Whole-ramp consumers
remain unchanged. The Rust reader search and direct xrefs are bounded findings,
not an exhaustive field-alias or whole procedural-drawing audit.
