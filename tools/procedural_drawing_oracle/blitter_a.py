"""Original per-blitter A selection over existing raw surface pixels.

Reuses the native palette oracle's actual Convert/LightConvert/LUT producers.
Only synthetic source/A/Z/destination state is supplied; no game process or
original instruction is patched. This is leaf composition, not a whole scene.
"""
from functools import lru_cache
from itertools import product
from pathlib import Path
import struct

from tools import native_oracle
from tools.palette_oracle import oracle as palette

PROFILES = {
    'plain1': (1, False, True),
    'light27': (27, False, False),
    'scheme53': (53, True, False),
    'plain53': (53, False, True),
}
RAW = 0xF940
RGB = [200, 100, 50]


@lru_cache(None)
def tables(profile):
    rows, scheme, plain = PROFILES[profile]
    colors = bytes(RGB) * 256
    mask = native_oracle.file_span(native_oracle.image_bytes(), 0x83E1AC, 256)[1] if scheme else None
    converted = (palette.plain_palette_table(rows, colors) if plain else
                 palette.palette_table(rows, (1000, 1000, 1000), colors, 'mmx', mask))
    return palette.intensity_table(rows), converted


def execute(profile, a, index, brightness):
    u = palette.machine()
    obj, dest, src, z, shape, aobj, zobj, alpha, colors, lut = [palette.HEAP + offset for offset in
        (0, 0x1000, 0x2000, 0x3000, 0x4000, 0x5000, 0x6000, 0x7000, 0x8000, 0x10000)]
    lookup, converted = tables(profile)
    u.mem_write(colors, converted)
    u.mem_write(lut, lookup)
    palette.put32(u, 0x887644, zobj)
    palette.put32(u, zobj + 0x1C, z + 0x10000)
    palette.put32(u, zobj + 0x20, 0x10000)
    palette.put32(u, 0x87E8A4, aobj)
    palette.put32(u, aobj + 0x1C, alpha + 0x10000)
    palette.put32(u, aobj + 0x20, 0x10000)
    u.mem_write(src, bytes([index, 0, 1, index]))  # decoded source, hole, depth reject
    u.mem_write(shape, bytes(3))
    u.mem_write(alpha, struct.pack('<3H', *([a] * 3)))
    results = {}
    for shadow, address in ((False, 0x4990E0), (True, 0x497390)):
        u.mem_write(dest, struct.pack('<3H', *([RAW] * 3)))
        u.mem_write(z, struct.pack('<3H', 65535, 65535, 0))
        if shadow:
            palette.put32(u, obj, 0x7E54B0)
            # Original RGB565 mask established by terrain_draw_oracle/leaf.py.
            u.mem_write(obj + 4, struct.pack('<H', 0x7BEF))
        else:
            u.mem_write(obj, struct.pack('<3I', 0x7E53A0, colors, lut))
        palette.call(u, address, (dest, src, 3, 0, 4096, z, alpha, brightness, 0, shape), obj)
        results['shadow' if shadow else 'body'] = {
            'colors': list(struct.unpack('<3H', u.mem_read(dest, 6))),
            'depths': list(struct.unpack('<3H', u.mem_read(z, 6))),
        }
    return dict(profile=profile, a=a, index=index, brightness=brightness, **results)


def generate():
    return dict(raw=RAW, rgb=RGB, decoded_stencil=['source', 'hole', 'depth_reject'],
                candidate=4096, old_depth=[65535, 65535, 0],
                cases=[execute(*case) for case in product(
                    PROFILES, (0, 1, 2, 63, 126, 127, 128, 254, 255), (1, 240), (1000, 1500))])


def metadata():
    return native_oracle.provenance(
        scope='Original extended body4990E0 and shadow497390 over raw destination pixels, using native-generated palette and A lookup tables.',
        assumptions=[
            'Pinned active-retail image, RGB565/MMX and x87 CW0E7F as established by the palette owner runtime capture.',
            'Synthetic palette RGB200,100,50; native mask83E1AC; N1/N27/N53, plain/LightConvert/ColorScheme producers, brightness1000/1500, indices1/240.',
            'Prepared raw destinationF940, A words0/1/2/63/126/127/128/254/255, row candidate4096, signed shape zero, old Z[65535,65535,0], actual compressed source skip.',
            'The raw destination models an earlier DSurface store. Rally producer execution and its pass admission/order remain in rally.json; this corpus does not execute a whole native frame or derive shroud cells.',
            'Mask7BEF is supplied from the existing original RGB565 mask-constructor corpus. No timer, RNG or detach is involved.',
        ], substitutions=[],
        entry_points=dict(extended_body=0x4990E0, shadow=0x497390,
                          light_convert=0x556090, plain_convert=0x4BBB00, intensity=0x420196))


if __name__ == '__main__':
    native_oracle.finish_vectors(generate, Path(__file__).with_suffix('.json'), provenance=metadata,
        source_paths={'oracle': Path(__file__), 'palette_owner': Path(palette.__file__),
                      'native_owner': Path(native_oracle.__file__)})
