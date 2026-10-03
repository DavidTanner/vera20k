"""Original House56F9 color from physical RULESMD lexical inputs.

ReadColor, ramp generation, lighting tables and House color extraction execute
original code. The existing palette oracle owns conversion; this fixture only
prepares caller data. Default/--check compares without writing.
"""
import hashlib
import json
from pathlib import Path
import struct

from unicorn.x86_const import *

from tools import native_oracle as native
from tools.palette_oracle import oracle as palette
from tools.rules_oracle import bridge_child_sound as lexical
from tools.spatial_oracle import building_body_rules as ini


def physical_colors():
    path = Path('ini/rulesmd.ini')
    raw = path.read_bytes()
    # Existing physical lexical extraction supplies strings only. The original
    # ReadColor474C70 below owns numeric parsing, defaults and byte narrowing.
    colors = lexical.sections(raw)['Colors']
    return raw, colors


def parse_native_color(name, raw):
    f = ini.Fixture()
    u = f.u
    key, default, out = ini.SCRATCH + 0x7000, ini.SCRATCH + 0x7100, ini.SCRATCH + 0x7110
    u.mem_write(key, name.encode('ascii') + b'\0')
    u.mem_write(ini.TYPE + 0x1F8, b'Colors\0')
    f.ini(key, raw)
    u.mem_write(default, b'\0\0\0')
    u.mem_write(ini.SP, ini.dwords(native.RET_MAGIC, out, ini.TYPE + 0x1F8, key, default))
    u.reg_write(UC_X86_REG_ESP, ini.SP)
    u.reg_write(UC_X86_REG_ECX, ini.INI)
    native.run_checked(u, 0x474C70, native.RET_MAGIC, count=100000,
                       required_addresses=[0x474C70, 0x528A10])
    return bytes(u.mem_read(out, 3))


def native_ramp(hsv):
    u = palette.machine()
    src, out, color = palette.HEAP, palette.HEAP + 0x1000, palette.HEAP + 0x2000
    u.mem_write(src, bytes([0xA5]) * 768)
    u.mem_write(color, hsv)
    sp = palette.STACK + 0x80000
    u.mem_write(sp, ini.dwords(palette.STOP, src, out, 0, 53, 1000, 1000, 1000, 0x83E1AC))
    u.reg_write(UC_X86_REG_ESP, sp)
    u.reg_write(UC_X86_REG_ECX, color)
    u.reg_write(UC_X86_REG_EDX, src)
    native.run_checked(u, 0x68C3B0, 0x68C4AD, count=20000,
                       required_addresses=[0x68C3B0, 0x6260D0, 0x4CACB0, 0x4CAD00, 0x517440])
    result = bytes(u.mem_read(out, 768))
    assert result[:48] == b'\xA5' * 48 and result[96:] == b'\xA5' * (768 - 96)
    return result


def constructor_index():
    u = palette.machine()
    scheme = palette.HEAP
    u.reg_write(UC_X86_REG_ESI, scheme)
    u.reg_write(UC_X86_REG_EBX, 0)
    u.reg_write(UC_X86_REG_EAX, 53)
    u.reg_write(UC_X86_REG_ESP, palette.STACK + 0x80000)
    native.run_checked(u, 0x68C769, 0x68C7DC, count=100)
    return struct.unpack('<I', u.mem_read(scheme + 0x330, 4))[0]


def house_color(table, index=16, campaign=False, scheme_index=1):
    u = palette.machine()
    registry, scheme, convert = [palette.HEAP + x for x in (0x1000, 0x2000, 0x3000)]
    house, lookup = palette.HEAP + 0x10000, palette.HEAP + 0x30000
    for address, value in ((0x8A0DD0,11),(0x8A0DD4,3),(0x8A0DD8,0),
                           (0x8A0DDC,3),(0x8A0DE0,5),(0x8A0DE4,2)):
        palette.put32(u, address, value)
    palette.put32(u, 0xB054D4, registry)
    palette.put32(u, registry + 4, scheme)
    palette.put32(u, registry + 20, scheme)
    palette.put32(u, scheme + 0x330, index)
    palette.put32(u, scheme + 0x30C, convert)
    palette.put32(u, convert + 4, 2)
    palette.put32(u, convert + 0x174, lookup)
    palette.put32(u, house + 0x16054, scheme_index)
    u.mem_write(lookup, table)
    u.mem_write(house + 0x56F8, bytes(range(0xA0, 0xA8)))
    before = bytes(u.mem_read(house + 0x56F8, 8))
    if campaign:
        u.reg_write(UC_X86_REG_ESP, palette.STACK + 0x80000)
        u.reg_write(UC_X86_REG_EBX, house)
        u.reg_write(UC_X86_REG_EAX, scheme_index & 0xFFFFFFFF)
        native.run_checked(u, 0x500DF7, 0x500ECC, count=200)
    else:
        palette.call(u, 0x50B840, (), house)
    after = bytes(u.mem_read(house + 0x56F8, 8))
    assert before[0] == after[0] and before[4:] == after[4:]
    return dict(rgb=list(after[1:4]),
                scheme_index=struct.unpack('<i',u.mem_read(house+0x16054,4))[0])


def generate():
    raw, entries = physical_colors()
    default_index = constructor_index()
    assert default_index == 16
    rows = []
    for name, lexical in entries.items():
        hsv = parse_native_color(name, lexical)
        ramp = native_ramp(hsv)
        outputs = {}
        for mode in ('scalar', 'cmov', 'mmx'):
            table = palette.palette_table(53, (1000,1000,1000), ramp, mode)
            middle = table[26*512:27*512]
            regular = house_color(middle, default_index)
            campaign = house_color(middle, default_index, campaign=True)
            assert regular == campaign
            outputs[mode] = dict(packed=struct.unpack_from('<H',middle,32)[0],
                                 rgb=regular['rgb'], scheme_index=regular['scheme_index'],
                                 campaign_rgb=campaign['rgb'],
                                 campaign_scheme_index=campaign['scheme_index'])
        direct_table = palette.plain_palette_table(1, ramp)
        rows.append(dict(name=name, lexical=lexical, hsv=list(hsv),
                         palette_rgb=list(ramp[48:51]),
                         ramp_rgb=[list(ramp[i:i+3]) for i in range(48,96,3)],
                         plain_packed=struct.unpack_from('<H',direct_table,32)[0],
                         modes=outputs))
    # Index controls use different table bytes at15/16/17, proving the +330
    # lookup rather than assuming the brightest address from a matching word.
    control_table = struct.pack('<256H', *[(i*251)&65535 for i in range(256)])
    controls = [dict(index=i, packed=struct.unpack_from('<H',control_table,i*2)[0],
                     normal=house_color(control_table,i),
                     campaign=house_color(control_table,i,campaign=True))
                for i in (0,15,16,17,31,255)]
    fallback = dict(normal=house_color(control_table,16,scheme_index=-1),
                    campaign=house_color(control_table,16,campaign=True,scheme_index=-1))
    assert all(c['normal']==c['campaign'] for c in controls)
    assert fallback['normal']==fallback['campaign']
    return dict(native_sha256=native.NATIVE_SHA256, rulesmd_sha256=hashlib.sha256(raw).hexdigest(),
                constructor_palette_index=default_index, colors=rows, index_controls=controls,
                negative_scheme=fallback,
                scope='Native ReadColor, ramp, N53 middle-row colors and House/campaign RGB extraction')



if __name__ == '__main__':
    native.finish_vectors(generate, Path(__file__).with_suffix('.json'),
        provenance=lambda: native.provenance(
            scope='21 physical RULESMD Colors entries parsed by original474C70; original ramp68C3B0, N53 LightConvert556090 in scalar/CMOV/MMX modes, whole House50B840 and campaign color block500DF7..500ECC',
            assumptions=[
                'ini/rulesmd.ini physical bytes and lexical strings are recorded with SHA256. Existing lexical.sections only extracts strings; supplied native CRC/section caches replace INI/archive IO. Original ReadString64, sprintf and sscanf own parsing. Mode/map/LANGRULE overrides are not loaded in this base-reader corpus.',
                'Original ColorScheme68C769..68C7DC constructor slice writes lookup index16. Original ramp68C3B0..68C4AD includes palette copy, native trig table sampling, ftol and HSV conversion, stopping before allocation of LightConvert. Base palette outside indices16..31 is supplied A5 and asserted unchanged.',
                'Existing tools.palette_oracle.oracle owns full original556090 table construction, scalar/CMOV/MMX dispatch and plain4BBB00 comparison. N53 and RGB1000/1000/1000 are supplied constructor inputs from68C710; active RGB565 shifts/losses and FPCW0E7F are supplied. ColorScheme mask16 is1, so its result is identical to the fixture all-one mask.',
                'Convert48E740 stores +174 at ((N-1)>>1)*512 bytes after +170, so House receives the actual native row26 bytes. Converter allocation, row-pointer construction and active effects/rebuild state are instruction evidence, not executed in this fixture.',
                'Whole House50B840 receives supplied registry/ColorScheme/converter fields. Campaign starts500DF7 after ReadColorString with supplied index in EAX and House in EBX; stops500ECC immediately after RGB stores, before separate laser-color normalization. Whole campaign INI reader and name lookup are excluded.',
                'All21 colors compare normal/campaign and all three CPU paths. Six distinct +330 lookup controls plus negative-scheme fallback are saved; adjacent bytes and separate House+56FC laser RGB remain unchanged. No full renderer/GPU or exhaustive arbitrary-HSV equivalence claim.',
            ],
            substitutions=[],
            entry_points={'read_color':0x474C70,'ramp':0x68C3B0,
                          'constructor_index_begin':0x68C769,'constructor_index_end':0x68C7DC,
                          'light_table':0x556090,'plain_table':0x4BBB00,
                          'house_init_color':0x50B840,'campaign_color_begin':0x500DF7,
                          'campaign_color_end':0x500ECC}),
        source_paths={'caller':Path(__file__), 'palette_owner':Path(palette.__file__),
                      'ini_fixture':Path(ini.__file__), 'lexical_fixture':Path(lexical.__file__)})
