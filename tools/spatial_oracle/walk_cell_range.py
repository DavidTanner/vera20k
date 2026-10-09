"""Original Coord->Cell range wrapper and Cell range geometry/query order.

Rows with `target_object` range an Infantry object through CanFireAt 0x6F77B0
instead. Rows with `subject_to_elevation` add InRange's elevation bonus
(0x6F6F60 on both arms, 0x6F70E0 on the arcing arm). Rows that supply
`elevation` or `arcing` also record `bonuses`, each bonus InRange computed.
Arcing rows run the arc test (Ballistic_Launch_Speed 0x48AB90, Can_Reach
0x48ABC0) under the row's `gravity` and `floater`, then the bridge ceiling.

The weapon slot and object fields are supplied. No range verdict, Cell getter,
map lookup, distance calculation or line-of-fire callable is substituted.
"""
from pathlib import Path
import struct
from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESP, UC_X86_REG_EIP, UC_X86_REG_FPCW
from tools.native_oracle import load_image, run_checked, STACK_BASE, STACK_SIZE, SCRATCH, RET_MAGIC, finish_vectors, provenance
from tools.spatial_oracle.map_queries import dwords, packed

ACTOR, TYPE, HOUSE, VT, WEAPON, SLOT, PROJECTILE, CELLS, COORD, RULES, TARGET = [
    SCRATCH + i * 0x2000 for i in range(11)]
MAP, TABLE, DUMMY = 0x87F7E8, 0xC00000, 0xABDC50


def query(row):
    u = Uc(UC_ARCH_X86, UC_MODE_32)
    load_image(u)
    u.mem_map(STACK_BASE, STACK_SIZE)
    u.mem_map(SCRATCH, 0x20000)
    u.mem_map(RET_MAGIC, 0x1000)
    u.reg_write(UC_X86_REG_FPCW, 0x0E7F)
    def read32(p): return struct.unpack('<I', u.mem_read(p, 4))[0]
    def xyz(p): return list(struct.unpack('<iii', u.mem_read(p, 12)))
    def cell_coord(p): return list(struct.unpack('<hh', u.mem_read(p + 0x24, 4)))
    def ret(cleanup, result):
        sp = u.reg_read(UC_X86_REG_ESP)
        u.reg_write(UC_X86_REG_EAX, result & 0xffffffff)
        u.reg_write(UC_X86_REG_EIP, read32(sp))
        u.reg_write(UC_X86_REG_ESP, sp + cleanup + 4)
    u.mem_write(VT, bytes(u.mem_read(0x7EB058, 0x600)))
    u.mem_write(ACTOR, dwords(VT))
    u.mem_write(ACTOR + 0x6C0, dwords(TYPE))
    u.mem_write(ACTOR + 0x21C, dwords(HOUSE))
    u.mem_write(ACTOR + 0x9C, dwords(*row.get('source', [2624, 2624, 0])))
    u.mem_write(ACTOR + 0x74, bytes([int(row.get('marked', False))]))
    u.mem_write(ACTOR + 0x8C, bytes([int(row.get('on_bridge', False))]))
    # InRange 0x6F72C8: +0x82 (in an open-topped transport) adds Rules+0xF5C << 8.
    u.mem_write(ACTOR + 0x82, bytes([int(row.get('open_topped', False))]))
    u.mem_write(0x8871E0, dwords(RULES))
    u.mem_write(RULES + 0xF5C, dwords(row.get('open_topped_bonus', 2)))
    # [ElevationModel] Rules+0x1838/+0x1840/+0x1848, the constructor's 0/1.0/0.0
    # unless supplied, and BulletType+0x297 SubjectToElevation / +0x29B Arcing.
    increment, increment_bonus, cap = row.get('elevation', [0, 1.0, 0.0])
    u.mem_write(RULES + 0x1838, dwords(increment))
    u.mem_write(RULES + 0x1840, struct.pack('<dd', increment_bonus, cap))
    u.mem_write(PROJECTILE + 0x297, bytes([int(row.get('subject_to_elevation', False))]))
    u.mem_write(PROJECTILE + 0x29B, bytes([int(row.get('arcing', False))]))
    # The arcing arm's arc test reads Rules+0x16B8 Gravity (0 unless supplied),
    # halved through 0x48ACF0 for a BulletType+0x295 Floater.
    u.mem_write(RULES + 0x16B8, dwords(row.get('gravity', 0)))
    u.mem_write(PROJECTILE + 0x295, bytes([int(row.get('floater', False))]))
    u.mem_write(WEAPON + 0xA0, dwords(PROJECTILE))
    u.mem_write(WEAPON + 0xB4, dwords(row.get('range', 768)))
    u.mem_write(WEAPON + 0xB8, dwords(row.get('minimum', 0)))
    u.mem_write(WEAPON + 0x134, bytes([int(row.get('cell_rangefinding', False))]))
    u.mem_write(SLOT, dwords(WEAPON))
    u.mem_write(COORD, dwords(*row.get('target', [3036, 2780, 123])))
    u.mem_write(0x89E7C0, dwords(104))
    u.mem_write(0xAC13C8, dwords(104))
    u.mem_write(0xAC13BC, dwords(416))
    u.mem_write(0xB0EB24, dwords(416))
    u.mem_write(0xB0EB34, dwords(104))
    u.mem_write(0xAA0738, dwords(row.get('water_base', 314)))
    table = bytearray(0x100000)
    def put_cell(p, c):
        u.mem_write(p, dwords(0x7E4EEC))
        u.mem_write(p + 0x24, packed(*c['coord']))
        u.mem_write(p + 0x38, dwords(c.get('tile', 0)))
        u.mem_write(p + 0x11B, bytes([c.get('level', 0) & 255, c.get('slope', 0)]))
        u.mem_write(p + 0x140, dwords(c.get('flags', 0)))
    cells = row.get('cells', [{'coord': [10, 10]}, {'coord': [11, 10]}])
    for i, c in enumerate(cells):
        p = CELLS + i * 0x200
        put_cell(p, c)
        x, y = c.get('slot', c['coord'])
        struct.pack_into('<I', table, (y * 512 + x) * 4, p)
    put_cell(DUMMY, {'coord': [99, 98], 'tile': 65535, **row.get('dummy', {})})
    u.mem_write(TABLE, bytes(table))
    target_object = row.get('target_object')
    if target_object is not None:
        u.mem_write(TARGET, dwords(VT))
        u.mem_write(TARGET + 0x6C0, dwords(TYPE))
        u.mem_write(TARGET + 0x21C, dwords(HOUSE))
        u.mem_write(TARGET + 0x9C, dwords(*target_object['location']))
        u.mem_write(TARGET + 0x74, bytes([int(target_object.get('marked', True))]))
        u.mem_write(TARGET + 0x8C, bytes([int(target_object.get('on_bridge', False))]))
    u.mem_write(MAP + 0x13C, dwords(TABLE, 0x40000))
    events = []
    geometry = None
    bonuses = []
    def observe(_u, address, _size, _data):
        nonlocal geometry
        sp = u.reg_read(UC_X86_REG_ESP)
        # Returns of the direct (0x6F72FF) and arcing (0x6F746B) bonus calls.
        if address in (0x6F7304, 0x6F7470):
            bonus = struct.unpack('<i', dwords(u.reg_read(UC_X86_REG_EAX)))[0]
            bonuses.append(['direct' if address == 0x6F7304 else 'arcing', bonus])
        if address == read32(VT + 0x3F8):
            events.append(['weapon', read32(sp + 4)])
            ret(4, SLOT)
        elif address in (0x565730, 0x578080):
            events.append([hex(address), xyz(read32(sp + 4))])
        elif address == 0x5657A0:
            events.append([hex(address), list(struct.unpack('<hh', u.mem_read(read32(sp + 4), 4)))])
        elif address in (0x486840, 0x4867E0):
            p = u.reg_read(UC_X86_REG_ECX)
            events.append([hex(address), p == DUMMY, cell_coord(p)])
        elif address == 0x47B3A0:
            p = u.reg_read(UC_X86_REG_ECX)
            events.append([hex(address), p == DUMMY, cell_coord(p)])
        elif address == 0x6F7220:
            p = read32(sp + 8)
            events.append(['range', xyz(read32(sp + 4)), p == DUMMY, cell_coord(p)])
        elif address == 0x6F7379:
            geometry = xyz(sp + 0x20)
        elif address == 0x4CC310:
            events.append(['line', xyz(u.reg_read(UC_X86_REG_ECX)), xyz(u.reg_read(UC_X86_REG_EDX))])
    u.hook_add(UC_HOOK_CODE, observe)
    sp = STACK_BASE + STACK_SIZE - 0x1000
    entry, target = (0x6F77B0, TARGET) if target_object is not None else (0x6F7970, COORD)
    u.mem_write(sp, dwords(RET_MAGIC, target, 0))
    u.reg_write(UC_X86_REG_ESP, sp)
    u.reg_write(UC_X86_REG_ECX, ACTOR)
    run_checked(u, entry, RET_MAGIC, count=100000, required_addresses=[entry, 0x6F77B0, 0x6F7220])
    assert u.reg_read(UC_X86_REG_ESP) == sp + 12
    out = {'input': row, 'result': bool(u.reg_read(UC_X86_REG_EAX) & 255), 'target_geometry': geometry, 'events': events, 'dummy_coord': cell_coord(DUMMY)}
    if 'elevation' in row or row.get('arcing'):
        out.update(bonuses=bonuses)
    return out


def generate():
    rows = [
        {},
        {'range': 326}, {'range': 325}, {'minimum': 326}, {'minimum': 327},
        {'range': -512, 'target': [3550, 2780, 0], 'cells': []},
        {'target': [3550, 2780, 0], 'range': 1024},
        {'target': [3550, 2780, 0], 'range': 1024, 'dummy': {'level': 2, 'slope': 1}},
        {'target': [3550, 2780, 0], 'range': 1024, 'dummy': {'level': -1}},
        # The first missing target and subsequent missing shooter cell share
        # one pointer. Source recentering overwrites the target's coordinates.
        {'target': [3550, 2780, 0], 'source': [5184, 2624, 0], 'cell_rangefinding': True, 'range': 1, 'dummy': {'level': 2}},
        {'target': [-257, 2780, 0], 'source': [128, 2688, 0], 'range': 1024,
         'cells': [{'coord': [0, 10], 'level': 3, 'flags': 256}], 'dummy': {'level': 1}},
        {'target': [-130816, 512, 0], 'source': [384, 384, 0], 'range': 1,
         'cells': [{'coord': [1, 1]}]},
        {'cell_rangefinding': True},
        {'source': [2624, 2624, 800], 'marked': True},
        {'source': [2624, 2624, 800], 'marked': True, 'cell_rangefinding': True},
        {'cells': [{'coord': [10, 10], 'flags': 256}, {'coord': [11, 10], 'flags': 256}]},
        {'cells': [{'coord': [10, 10], 'flags': 256}, {'coord': [11, 10], 'flags': 256}], 'cell_rangefinding': True, 'on_bridge': True},
        # +0x82: the target cell's centre is 326 leptons away, so a 70-lepton
        # weapon reaches it exactly with one bonus cell and misses it by one
        # lepton at 69; the bonus needs the flag, and the retail two cells
        # lift a 0-lepton weapon past it.
        {'range': 70, 'open_topped': True, 'open_topped_bonus': 1},
        {'range': 69, 'open_topped': True, 'open_topped_bonus': 1},
        {'range': 70, 'open_topped_bonus': 1},
        {'range': 0, 'open_topped': True},
        {'range': 326, 'open_topped': True, 'open_topped_bonus': 0},
        {'range': 582, 'open_topped': True, 'open_topped_bonus': -1},
        {'range': 581, 'open_topped': True, 'open_topped_bonus': -1},
        {'minimum': 327, 'open_topped': True},
    ]
    for tile in [313, 314, 327, 328, -1]:
        rows.append({'cells': [{'coord': [10, 10]}, {'coord': [11, 10], 'tile': tile, 'flags': 256, 'level': 1}], 'range': 500})
    for base, tile in [(-1, 0), (-1, 13), (2147483640, 2147483640)]:
        rows.append({'water_base': base, 'cells': [{'coord': [10, 10]}, {'coord': [11, 10], 'tile': tile, 'flags': 256}], 'range': 400})
    # InRange's elevation bonus: the actor at cell (10,10) on `level` shoots at
    # cell (13,10). Each case runs at the range where the distance equals the
    # range plus the bonus, and one lepton short of it.
    retail = [4, 2.0, 2.0]
    def elevated(level, target_level=0, source_flags=0, target_tile=0):
        return {'source': [2624, 2624, level * 104], 'target': [3550, 2780, 0], 'marked': True,
                'elevation': retail, 'subject_to_elevation': True,
                'cells': [{'coord': [10, 10], 'level': level, 'flags': source_flags},
                          {'coord': [13, 10], 'level': target_level, 'tile': target_tile}]}
    infantry = {'source': [2624, 2624, 416], 'marked': True, 'elevation': retail, 'subject_to_elevation': True,
                'cells': [{'coord': [10, 10], 'level': 4}, {'coord': [13, 10]}]}
    for edge, case in [
        (273, elevated(4)),
        (932, {**elevated(4), 'subject_to_elevation': False}),
        (578, elevated(3)),
        (202, elevated(8)),
        (932, elevated(0, target_level=4)),
        (932, {**elevated(4), 'marked': False}),
        (49, {**elevated(7), 'elevation': [3, 1.5, 4.0]}),
        (234, {**elevated(6), 'elevation': [2, 0.7, 10.0]}),
        (273, {**elevated(4), 'elevation': [-4, 2.0, 2.0]}),
        (175, elevated(0, source_flags=0x80)),
        (932, elevated(4, target_tile=314)),
        (234, {**elevated(4), 'cells': [{'coord': [10, 10], 'level': 4}], 'dummy': {'level': -2}}),
        # Truncation toward zero takes the Dummy's centre (-128) into cell 0.
        (256, {'target': [-257, 2780, 0], 'source': [128, 2688, 312], 'marked': True, 'elevation': retail,
               'subject_to_elevation': True, 'cells': [{'coord': [0, 10], 'level': 3}], 'dummy': {'level': 1}}),
        (322, {**elevated(4), 'arcing': True}),
        (1346, {**elevated(4), 'arcing': True, 'elevation': [-4, 2.0, 2.0]}),
        # Sqrt_Approx gives 254 for a 255-lepton 2-D distance.
        (254, {'source': [2689, 2688, 0], 'marked': True, 'arcing': True}),
        (273, {**infantry, 'target_object': {'location': [3456, 2688, 0]}}),
        (932, {**infantry, 'target_object': {'location': [3456, 2688, 0], 'marked': False}}),
    ]:
        rows += [{**case, 'range': edge}, {**case, 'range': edge - 1}]
    # The arcing arm's arc test: the speed the range gives must reach the
    # target's height. The actor stands 834 leptons from cell (13,10)'s centre;
    # each case runs at the least range the native test admits, and one less.
    def arc(target_level, **extra):
        return {'arcing': True, 'gravity': 6, 'target': [3550, 2780, 0],
                'cells': [{'coord': [10, 10]}, {'coord': [13, 10], 'level': target_level}], **extra}
    for edge, case in [
        (834, arc(0)),
        (912, arc(2)),
        (1004, arc(3)),
        (1126, arc(4)),
        (903, arc(2, floater=True)),
        (834, arc(2, gravity=-6)),
        (912, arc(0, gravity=-6, source=[2624, 2624, 208])),
        (322, {**elevated(4), 'arcing': True, 'gravity': 6}),
        # No 2-D distance: Can_Reach measures 0.001 leptons.
        (701, arc(4, source=[3456, 2688, 0])),
        # An Infantry target two levels up, snapped to its cell's ground.
        (912, arc(2, target_object={'location': [3456, 2688, 208]})),
    ]:
        rows += [{**case, 'range': edge}, {**case, 'range': edge - 1}]
    # The bridge ceiling: a target point whose cell carries the bridge bit
    # (snapped onto the deck, 416 up) is refused from 312 leptons below.
    def ceiling(source_z, gravity=0, flags=256, rng=834, level=0):
        return {'arcing': True, 'gravity': gravity, 'range': rng, 'target': [3550, 2780, 0],
                'source': [2624, 2624, source_z],
                'cells': [{'coord': [10, 10]}, {'coord': [13, 10], 'level': level, 'flags': flags}]}
    rows += [
        ceiling(104), ceiling(105), ceiling(104, flags=0, level=4),
        ceiling(104, gravity=6, rng=1100), ceiling(208, gravity=6, rng=1100),
        # The Dummy's centre (-128) truncates into real cell (0,10), whose bit decides.
        {'arcing': True, 'target': [-257, 2780, 0], 'source': [128, 2688, 0], 'range': 1024,
         'cells': [{'coord': [0, 10], 'level': 3, 'flags': 256}], 'dummy': {'level': 1, 'flags': 256}},
        {'arcing': True, 'target': [-257, 2780, 0], 'source': [128, 2688, 0], 'range': 1024,
         'cells': [{'coord': [0, 10], 'level': 3}], 'dummy': {'level': 1, 'flags': 256}},
    ]
    return [query(row) for row in rows]


if __name__ == '__main__':
    finish_vectors(generate, Path(__file__).with_suffix('.json'), provenance=lambda: provenance(
        scope='Original6F7970->6F77B0->6F7220 Infantry Cell-target range and lookup ordering, including the +0x82 OpenToppedRangeBonus stage; 6F77B0->6F7220 against an Infantry object; the SubjectToElevation bonus (6F6F60 both arms, 6F70E0 arcing arm), the arcing arm\'s 2-D distance, its arc test (48AB90, 48ACF0, 48ABC0) under supplied Gravity/Floater and its bridge ceiling (6F74D7); explicit supplied object/weapon/map fields.',
        entry_points={'coordinate_cell_wrapper': 0x6F7970, 'range_source': 0x6F77B0, 'range': 0x6F7220, 'elevation_direct': 0x6F6F60, 'elevation_arcing': 0x6F70E0, 'cell_height': 0x487D50, 'map_cell_packed': 0x5657A0, 'cell_coords': 0x486840, 'cell_tile_gate': 0x4867E0, 'cell_ground': 0x47B3A0, 'map_ground': 0x578080, 'map_cell': 0x565730, 'line': 0x4CC310, 'launch_speed': 0x48AB90, 'floater_gravity': 0x48ACF0, 'can_reach': 0x48ABC0},
        assumptions=['Supplied original Infantry table7EB058 (actor and object target) and Cell table7E4EEC; actor non-garrison, no bunker/veteran range bonuses; the open-topped rows set +0x82 and Rules+0xF5C (Rules at0x8871E0, bonus2 unless supplied). Projectile flags are false but the rows\' Floater +0x295, SubjectToElevation +0x297 and Arcing +0x29B: no wall/cliff collision; original line callable executes.', 'Supplied independently established104 level/208 high-flight and416 bridge constants and the104 Techno level height0xB0EB34 (StaticInit6F2970), x87 control0E7F. WaterSet base is a supplied theater input; tile and Dummy level/slope/flags are supplied current state. [ElevationModel] Rules+0x1838/+0x1840/+0x1848 are supplied per row, the constructor\'s 0/1.0/0.0 otherwise.', 'Rules+0x16B8 Gravity and BulletType+0x295 Floater are supplied per row, Gravity 0 unless given: a zero gravity gives launch speed 0 and Can_Reach48ABC0 admits the row. Rows exercise gravity 6, 6 halved by Floater, -6 and 0, a zero 2-D distance, and the bridge ceiling on a real cell, without and with retail gravity, and through a Dummy target centre that truncates into a real cell.', 'No constructors or complete PerCell/weapon selection/flight behavior claimed.'],
        substitutions=['GetWeapon+3F8 records requested slot and supplies one original-shaped weapon slot. No other callable substitution.']))
