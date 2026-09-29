"""Original Foot+4C wrapper over live Walk/Drive/Ship/Hover heads and Tube exit.

The default 44-row coordinate corpus is unchanged. Use --bridge-layers to check
the separate original Foot+BC/Object bridge-layer and base-response call seams:
python -m tools.spatial_oracle.foot_navigation_coordinate --bridge-layers --check
See foot_bridge_layer.md for fixture inputs and evidence bounds.
"""
import argparse
from pathlib import Path
import struct

from unicorn import UC_HOOK_CODE, UC_HOOK_MEM_WRITE
from unicorn.x86_const import (
    UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_ESI,
    UC_X86_REG_ESP, UC_X86_REG_FPCW, UC_X86_REG_EIP, UC_X86_REG_EDI,
)
from tools.native_oracle import (
    STACK_BASE, STACK_SIZE, RET_MAGIC, SCRATCH, NATIVE_SHA256, file_span,
    image_bytes, run_checked, finish_vectors, provenance,
)
from tools.spatial_oracle.map_queries import TABLE, EMPTY_TABLE, DUMMY, dwords, packed
from tools.spatial_oracle.locomotor_at_coord import OriginalQuery, FAMILIES, LOCO, FOOT, OUTPUT


def query(row):
    fixture = OriginalQuery(row)
    u = fixture.uc
    # Original concrete owner table, including +48 Object-coordinate dispatch.
    u.mem_write(FOOT, dwords(0x7EB058 if row['family'] == 'walk' else 0x7F5C70))
    u.mem_write(FOOT + 0x674, dwords(LOCO))
    u.mem_write(0x8B3DA8, dwords(0, 0, 0))
    tube = row.get('tube')
    if tube is None:
        u.mem_write(FOOT + 0x684, b'\xff')
    else:
        table, descriptor = SCRATCH + 0x7000, SCRATCH + 0x7100
        u.mem_write(FOOT + 0x684, b'\x00')
        u.mem_write(0x8B413C, dwords(table))
        u.mem_write(table, dwords(descriptor))
        u.mem_write(descriptor + 0x28, packed(*tube))
    sp = STACK_BASE + STACK_SIZE - 0x1000
    u.mem_write(sp, dwords(RET_MAGIC, OUTPUT, 0))
    u.reg_write(UC_X86_REG_ESP, sp)
    u.reg_write(UC_X86_REG_ECX, FOOT)
    required = [0x4DBDF0, 0x4DBE01] if tube is not None else [0x4DBDF0, FAMILIES[row['family']]['head']]
    run_checked(u, 0x4DBDF0, RET_MAGIC, count=5000, required_addresses=required)
    assert u.reg_read(UC_X86_REG_ESP) == sp + 12
    assert u.reg_read(UC_X86_REG_EAX) == OUTPUT
    return dict(input=row, coordinate=fixture.coord(OUTPUT),
                retained_head=fixture.coord(LOCO + FAMILIES[row['family']]['head_offset']),
                current=fixture.coord(FOOT + 0x9C))


def generate():
    rows = []
    for family in FAMILIES:
        base = dict(family=family, current=[1408, 1408, 416],
                    stored_head=[1664, 1408, 17], turn_index=-1, cursor=0)
        variants = [dict(name='retained_without_selector'),
                    dict(name='null_head_current', stored_head=[0, 0, 0]),
                    dict(name='both_null', stored_head=[0, 0, 0], current=[0, 0, 0]),
                    dict(name='signed_head', stored_head=[-257, -1, -2147483648]),
                    dict(name='selector_independent', turn_index=1, cursor=-1)]
        variants += [dict(name=f'partial_null_{axis}', stored_head=[int(i == axis) for i in range(3)]) for axis in range(3)]
        variants += [dict(name=f'tube_{x}_{y}', tube=[x, y])
                     for x, y in [(10, 20), (-1, -32768), (32767, 0)]]
        rows += [query(base | variant) for variant in variants]
    return rows


MAP = 0x87F7E8
SP = STACK_BASE + STACK_SIZE - 0x1000
PROBE, VICTIM, VICTIM_LOCO = SCRATCH + 0x2F00, SCRATCH + 0x3000, SCRATCH + 0x4004
NAV_LOCO, NAV_FOOT, TYPE, CELLS = SCRATCH + 0x4804, SCRATCH + 0x5000, SCRATCH + 0x6000, SCRATCH + 0x8000
ZONE_BASE, ZONE_RAW, ZONE_RECORDS = 0xD10000, 0xD11000, 0xD12000
CONCRETE_TABLES = {'walk': 0x7EB058, 'drive': 0x7F5C70,
                   'ship': 0x7F5C70, 'hover': 0x7F5C70}
# Bounded original instruction spans, never callable replacements. Cell47B3A0
# lazily initializes its own native slope coefficients; those data writes are
# part of the original execution and are not an instruction patch.
BRIDGE_SPANS = {
    'foot_coordinate': (0x4DBDF0, 0xDF),
    'object_coordinate': (0x5F65A0, 0x21),
    'foot_bridge_layer': (0x4DDC40, 0x1C),
    'object_bridge_layer': (0x5F6A70, 0xE0),
    'map_ground': (0x578080, 0x77),
    'cell_ground': (0x47B3A0, 0x7BB),
    'map_coordinate_cell': (0x565730, 0x68),
    'ftol': (0x7C5F00, 0x3D),
    'response_infantry_seam': (0x708291, 0xB4),
    'response_unit_seam': (0x7084E9, 0xB8),
    'reach_zone': (0x56D100, 0x122),
    'get_zone': (0x56D230, 0x16A),
    'record_lookup': (0x56DA10, 0xC4),
    'playfield': (0x578460, 0x190),
    'harvest_seam': (0x4DCF26, 0x71),
    'locomotor_base_constructor': (0x55A6C0, 0x23),
    'drive_constructor': (0x4AF540, 0x9A),
    'ship_constructor': (0x69EC50, 0x9A),
    **{family + '_head': (state['head'], 0x80) for family, state in FAMILIES.items()},
}


class OriginalBridgeQuery(OriginalQuery):
    """Supplied Foot/map state with observations of original bridge queries."""

    def __init__(self, case):
        super().__init__(case)
        u = self.uc
        self.stage, self.queries, self.pending = 'setup', [], {}
        self.reach_queries, self.reach_pending, self.dummy_writes = [], {}, []
        self.coordinate_calls, self.coordinate_pending, self.bridge_calls = [], {}, []
        self.cell_ids = {DUMMY: 'dummy'}
        self.setup_foot(FOOT, LOCO, case)
        u.mem_write(FOOT + 0x5A4, dwords(NAV_FOOT if case['nav_com_coord'] is not None else 0))
        if case['nav_com_coord'] is not None:
            self.setup_foot(NAV_FOOT, NAV_LOCO, dict(
                family='drive', current=case['nav_com_coord'], stored_head=case['nav_com_coord'],
                on_bridge=False, tube=None, locomotor_present=True))
        u.mem_write(TABLE, EMPTY_TABLE)
        u.mem_write(MAP + 0x13C, dwords(TABLE, case['table_capacity']))
        for name, value in case['scalar_globals'].items():
            u.mem_write(int(name, 16), dwords(value))
        u.reg_write(UC_X86_REG_FPCW, case['fpcw'])
        u.mem_write(0x822D80, dwords(case['ftol_control_word']))
        u.mem_write(0x89E770, b'\0')
        self.write_cell(DUMMY, case['dummy'])
        self.cell_snapshots = {}
        for index, cell in enumerate(case['cells']):
            pointer = CELLS + index * 0x200
            x, y = cell['xy']
            slot = y * 512 + x
            assert 0 <= slot < case['table_capacity'] <= 0x40000
            self.write_cell(pointer, cell)
            self.cell_ids[pointer] = f'cell_{index}'
            u.mem_write(TABLE + slot * 4, dwords(pointer))
            self.cell_snapshots[pointer] = bytes(u.mem_read(pointer, 0x200))
        self.originals = {name: bytes(u.mem_read(address, size))
                          for name, (address, size) in BRIDGE_SPANS.items()}
        self.foot_snapshot = bytes(u.mem_read(FOOT, 0x700))
        self.loco_snapshot = bytes(u.mem_read(LOCO, 0x100))
        u.hook_add(UC_HOOK_CODE, self.observe_bridge)
        u.hook_add(UC_HOOK_MEM_WRITE, self.observe_dummy_write)

    def setup_foot(self, foot, loco, row):
        u = self.uc
        table = CONCRETE_TABLES[row['family']]
        assert self.read32(table + 0x4C) == 0x4DBDF0
        assert self.read32(table + 0xBC) == 0x4DDC40
        assert self.read32(table + 0x48) == 0x5F65A0
        u.mem_write(foot, dwords(table))
        u.mem_write(foot + 0x9C, dwords(*row['current']))
        u.mem_write(foot + 0x8C, bytes((int(row['on_bridge']),)))
        u.mem_write(foot + 0x674, dwords(loco if row['locomotor_present'] else 0))
        u.mem_write(0x8B3DA8, dwords(0, 0, 0))
        family = FAMILIES[row['family']]
        u.mem_write(loco, dwords(family['vtable']))
        u.mem_write(loco + 8, dwords(foot))
        u.mem_write(loco + family['head_offset'], dwords(*row['stored_head']))
        u.mem_write(family['null'], dwords(0, 0, 0))
        u.mem_write(family['step'], dwords(104))
        tube = row['tube']
        u.mem_write(foot + 0x684, b'\xff' if tube is None else b'\0')
        if tube is not None:
            table, descriptor = SCRATCH + 0x7000, SCRATCH + 0x7100
            u.mem_write(0x8B413C, dwords(table))
            u.mem_write(table, dwords(descriptor))
            u.mem_write(descriptor + 0x28, packed(*tube))

    def write_cell(self, pointer, cell):
        self.uc.mem_write(pointer, bytes(0x200))
        self.uc.mem_write(pointer + 0x24, packed(*cell['xy']))
        self.uc.mem_write(pointer + 0x11B, bytes((cell['level'] & 255, cell['slope'])))
        self.uc.mem_write(pointer + 0x140, dwords(cell['flags']))

    def cell_state(self, pointer):
        return dict(identity=self.cell_ids[pointer],
                    xy=list(struct.unpack('<hh', self.uc.mem_read(pointer + 0x24, 4))),
                    level=struct.unpack('<b', self.uc.mem_read(pointer + 0x11B, 1))[0],
                    slope=self.uc.mem_read(pointer + 0x11C, 1)[0],
                    flags=self.read32(pointer + 0x140))

    def finish_query(self, address):
        u = self.uc
        if address in self.coordinate_pending:
            row = self.coordinate_calls[self.coordinate_pending.pop(address)]
            row['coordinate'] = self.coord(u.reg_read(UC_X86_REG_EAX))
        if address in self.pending:
            row = self.queries[self.pending.pop(address)]
            if row['entry'] == '0x00578080':
                row['ground'] = struct.unpack('<i', dwords(u.reg_read(UC_X86_REG_EAX)))[0]
            else:
                row['cell'] = self.cell_state(u.reg_read(UC_X86_REG_EAX))
            row['dummy_after'] = self.cell_state(DUMMY)
        if address in self.reach_pending:
            row = self.reach_queries[self.reach_pending.pop(address)]
            eax = u.reg_read(UC_X86_REG_EAX)
            row['result'] = bool(eax & 255) if row['entry'] == '0x00578460' else struct.unpack('<i', dwords(eax))[0]
            row['dummy_after'] = self.cell_state(DUMMY)

    def observe_bridge(self, u, address, _size, _data):
        self.finish_query(address)
        if address in (0x578080, 0x565730):
            sp = u.reg_read(UC_X86_REG_ESP)
            argument = self.read32(sp + 4)
            self.queries.append(dict(stage=self.stage, entry=f'0x{address:08X}',
                                     xyz=self.coord(argument)))
            self.pending[self.read32(sp)] = len(self.queries) - 1
        elif address == 0x47B3A0:
            self.queries[-1]['cell'] = self.cell_state(u.reg_read(UC_X86_REG_ECX))
        elif address == 0x4DBDF0:
            sp = u.reg_read(UC_X86_REG_ESP)
            receiver = u.reg_read(UC_X86_REG_ECX)
            assert receiver in (FOOT, VICTIM)
            self.coordinate_calls.append(dict(stage=self.stage,
                                             receiver='candidate' if receiver == FOOT else 'victim'))
            self.coordinate_pending[self.read32(sp)] = len(self.coordinate_calls) - 1
        elif address in (0x4DDC40, 0x5F6A70):
            self.bridge_calls.append(dict(stage=self.stage, entry=f'0x{address:08X}'))
        elif address in (0x578460, 0x56D230, 0x56DA10):
            sp = u.reg_read(UC_X86_REG_ESP)
            argument = self.read32(sp + 4)
            count = 1 if address == 0x578460 else 2
            self.reach_queries.append(dict(entry=f'0x{address:08X}',
                                           xy=list(struct.unpack('<hh', u.mem_read(argument, 4))),
                                           arguments=[self.read32(sp + 8 + i * 4) for i in range(count)]))
            self.reach_pending[self.read32(sp)] = len(self.reach_queries) - 1

    def observe_dummy_write(self, u, _access, address, size, value, _data):
        if address == DUMMY + 0x24:
            assert size == 4
            self.dummy_writes.append(dict(stage=self.stage,
                                          pc=f'0x{u.reg_read(UC_X86_REG_EIP):08X}',
                                          xy=list(struct.unpack('<hh', dwords(value)))))

    def invoke(self, entry, receiver, arguments, required):
        self.uc.reg_write(UC_X86_REG_ECX, receiver)
        self.call(entry, arguments, required)
        self.finish_query(RET_MAGIC)
        assert not self.pending and not self.coordinate_pending
        return self.uc.reg_read(UC_X86_REG_EAX)

    def validate_retained_state(self):
        assert self.originals == {name: bytes(self.uc.mem_read(address, size))
                                  for name, (address, size) in BRIDGE_SPANS.items()}
        assert self.foot_snapshot == bytes(self.uc.mem_read(FOOT, 0x700))
        assert self.loco_snapshot == bytes(self.uc.mem_read(LOCO, 0x100))
        assert all(blob == bytes(self.uc.mem_read(pointer, 0x200))
                   for pointer, blob in self.cell_snapshots.items())
        dummy = self.cell_state(DUMMY)
        assert all(dummy[key] == self.case['dummy'][key] for key in ('level', 'slope', 'flags'))

    def run_bridge(self):
        self.stage = 'prefix'
        for xyz in self.case['prefix_queries']:
            self.uc.mem_write(PROBE, dwords(*xyz))
            self.invoke(0x565730, MAP, [PROBE], [0x565730])
        self.stage = 'coordinate'
        required = [0x4DBDF0, 0x4DBE01] if self.case['tube'] is not None else [
            0x4DBDF0, FAMILIES[self.case['family']]['head']]
        assert self.invoke(0x4DBDF0, FOOT, [OUTPUT, 0], required) == OUTPUT
        coordinate = self.coord(OUTPUT)
        self.stage = 'bridge_layer'
        required = [0x4DDC40] if self.case['tube'] is not None else [
            0x4DDC40, 0x5F6A70, 0x4DBDF0, 0x578080, 0x47B3A0]
        layer = bool(self.invoke(0x4DDC40, FOOT, [0], required) & 255)
        if self.case['tube'] is not None:
            assert not any(row['stage'] == 'bridge_layer' for row in self.queries)
            assert not any(row['stage'] == 'bridge_layer' for row in self.coordinate_calls)
            assert not any(row['entry'] == '0x005F6A70' for row in self.bridge_calls)
        self.validate_retained_state()
        return dict(coordinate=coordinate, should_be_on_bridge=layer,
                    queries=self.queries, coordinate_calls=self.coordinate_calls,
                    bridge_calls=self.bridge_calls, final_dummy=self.cell_state(DUMMY),
                    dummy_writes=self.dummy_writes,
                    retained_head=self.coord(LOCO + self.family['head_offset']),
                    current=self.coord(FOOT + 0x9C), on_bridge=bool(self.uc.mem_read(FOOT + 0x8C, 1)[0]))

    def run_constructed_bridge(self):
        """Original fresh Drive/Ship constructor, supplied Foot link, queries."""
        constructor = {'drive': 0x4AF540, 'ship': 0x69EC50}[self.case['family']]
        local = LOCO - 4
        # Constructor inputs are explicit. Nonzero backing bytes ensure its
        # NullCoord initialization is observed rather than zero-filled memory.
        self.uc.mem_write(local, bytes((self.case['constructor_prefill_byte'],)) * 0x70)
        self.stage = 'constructor'
        returned = self.invoke(constructor, local, [], [constructor, 0x55A6C0])
        assert returned == local
        constructed = dict(
            entry=f'0x{constructor:08X}', base_entry='0x0055A6C0',
            object_vtable=f'0x{self.read32(local):08X}',
            locomotion_vtable=f'0x{self.read32(local + 4):08X}',
            piggyback_vtable=f'0x{self.read32(local + 0x18):08X}',
            destination=self.coord(local + 0x34), head=self.coord(local + 0x40),
            linked_foot=self.read32(local + 0xC), frame=self.read32(local + 0x24),
            selector=self.coord(local + 0x58, 2),
            payload_before_link=bytes(self.uc.mem_read(local, 0x6C)).hex())
        assert self.read32(local + 4) == self.family['vtable']
        # Linking the live Foot remains fixture state. The entire original
        # constructor/base and its head/destination/vtable writes ran above.
        self.uc.mem_write(local + 0xC, dwords(FOOT))
        self.loco_snapshot = bytes(self.uc.mem_read(LOCO, 0x100))
        return dict(input=self.case, constructor=constructed, output=self.run_bridge())

    def response_seam(self, victim, movement_zone, zones):
        """Execute the original scan-local block through original 56D100.

        Earlier scan admission and ranking are not executed or substituted.
        Stop before threat scoring, or at the reachability-refusal continuation.
        """
        self.setup_foot(VICTIM, VICTIM_LOCO, victim)
        infantry = self.case['family'] == 'walk'
        start, call_site = (0x708291, 0x70833C) if infantry else (0x7084E9, 0x708594)
        candidate_coord, layer_call = (0x7082DB, 0x708319) if infantry else (0x708533, 0x708571)
        self.uc.mem_write(FOOT + (0x6C0 if infantry else 0x6C4), dwords(TYPE))
        self.uc.mem_write(TYPE + 0x5B4, dwords(movement_zone))
        self.setup_zones(movement_zone, zones)
        self.foot_snapshot = bytes(self.uc.mem_read(FOOT, 0x700))
        self.stage = 'response_seam'
        self.uc.reg_write(UC_X86_REG_EBX, VICTIM)
        self.uc.reg_write(UC_X86_REG_ESI, FOOT)
        self.uc.reg_write(UC_X86_REG_ESP, SP)
        admitted, refused = (0x708345, 0x7083BC) if infantry else (0x7085A1, 0x708622)
        return self.run_reach_seam(start, movement_zone, (admitted, refused),
                                   (start, candidate_coord, layer_call, call_site, 0x4DBDF0, 0x4DDC40))

    def harvest_seam(self, clicked_cell, movement_zone, zones):
        """Original IsCellHarvestable coordinate/layer/zone block only."""
        type_offset = 0x6C0 if self.case['family'] == 'walk' else 0x6C4
        self.uc.mem_write(FOOT + type_offset, dwords(TYPE))
        self.uc.mem_write(TYPE + 0x5B4, dwords(movement_zone))
        self.setup_zones(movement_zone, zones)
        self.foot_snapshot = bytes(self.uc.mem_read(FOOT, 0x700))
        self.uc.mem_write(PROBE, packed(*clicked_cell))
        self.stage = 'harvest_seam'
        self.uc.reg_write(UC_X86_REG_ESI, FOOT)
        self.uc.reg_write(UC_X86_REG_EDI, PROBE)
        self.uc.reg_write(UC_X86_REG_ESP, SP)
        type_accessor = self.read32(CONCRETE_TABLES[self.case['family']] + 0x84)
        return self.run_reach_seam(0x4DCF26, movement_zone, 0x4DCF97,
                                   (0x4DCF31, 0x4DCF6F, 0x4DCF7A, 0x4DCF92,
                                    0x4DBDF0, 0x4DDC40, type_accessor))

    def run_reach_seam(self, start, movement_zone, ends, required):
        """Observe the same native six-argument protocol in either caller."""
        run_checked(self.uc, start, 0x56D100, count=5000,
                    required_addresses=required)
        sp = self.uc.reg_read(UC_X86_REG_ESP)
        arguments = [self.read32(sp + 4 + 4 * i) for i in range(6)]
        assert self.read32(sp) in (0x708341, 0x708599, 0x4DCF97)
        assert arguments[2] == movement_zone
        source_cell = list(struct.unpack('<hh', self.uc.mem_read(arguments[0], 4)))
        destination_cell = list(struct.unpack('<hh', self.uc.mem_read(arguments[1], 4)))
        dummy_at_reach = self.cell_state(DUMMY)
        endpoint = run_checked(self.uc, 0x56D100, ends, count=10000,
                               required_addresses=(0x56D100, 0x578460))
        assert not self.pending and not self.reach_pending and not self.coordinate_pending
        assert self.uc.reg_read(UC_X86_REG_ESP) == SP
        self.validate_retained_state()
        return dict(start=f'0x{start:08X}', stop_before=f'0x{endpoint:08X}',
                    source_cell=source_cell, destination_cell=destination_cell,
                    movement_zone=arguments[2], source_on_bridge=bool(arguments[3] & 255),
                    destination_on_bridge=bool(arguments[4] & 255),
                    allow_destination_fringe=bool(arguments[5] & 255),
                    raw_scalar_arguments=arguments[2:], queries=self.queries,
                    coordinate_calls=self.coordinate_calls, bridge_calls=self.bridge_calls,
                    reachable=bool(self.uc.reg_read(UC_X86_REG_EAX) & 255), reach_queries=self.reach_queries,
                    dummy_at_reach=dummy_at_reach, dummy_writes=self.dummy_writes,
                    final_dummy=self.cell_state(DUMMY),
                    retained_head=self.coord(LOCO + self.family['head_offset']), current=self.coord(FOOT + 0x9C))

    def setup_zones(self, movement_zone, zones):
        # Supplied simple base/raw zone state follows the existing
        # walk_move_admission fixture contract. There is no flood-fill result
        # substitution: original56D100/56D230/56DA10 read this state themselves.
        width, height = zones['size']
        side = width + height + 1
        capacity = side * side
        self.uc.mem_write(MAP + 0xF4, dwords(width, height, *zones['bounds']))
        self.uc.mem_write(MAP + 0x68, dwords(ZONE_BASE, capacity))
        self.uc.mem_write(ZONE_BASE, bytes(capacity * 4))
        for cell in zones['group_cells']:
            x, y = cell['xy']
            assert 0 <= x + y * side < capacity
            self.uc.mem_write(ZONE_BASE + (x + y * side) * 4 + 2, struct.pack('<H', cell['group']))
        self.uc.mem_write(MAP + 0x18 + movement_zone * 4, dwords(ZONE_RAW))
        self.uc.mem_write(ZONE_RAW, struct.pack('<' + 'H' * len(zones['raw_labels']), *zones['raw_labels']))
        for index, record in enumerate(zones['records']):
            self.uc.mem_write(ZONE_RECORDS + index * 16,
                              packed(*record['a']) + packed(*record['b']) + dwords(int(record['active']), record['kind']))
        self.uc.mem_write(MAP + 0x54, dwords(ZONE_RECORDS, len(zones['records']), 0, len(zones['records'])))


def bridge_case(family, name, **updates):
    row = dict(family=family, name=name, current=[1280, 1280, 416],
               stored_head=[1792, 1280, 17], turn_index=-1, cursor=0,
               on_bridge=False, tube=None, locomotor_present=True,
               nav_com_coord=[2304, 1280, 999], table_capacity=0x40000,
               scalar_globals={'0x0089E7C0': 104, '0x00AC13C8': 104},
               fpcw=0x0E7F, ftol_control_word=0x0E7F, prefix_queries=[],
               cells=[dict(xy=[5, 5], level=3, slope=1, flags=0),
                      dict(xy=[7, 5], level=0, slope=0, flags=0x100),
                      dict(xy=[9, 5], level=9, slope=0, flags=0x400)],
               dummy=dict(xy=[1234, -2345], level=-7, slope=1, flags=0x500))
    row.update(updates)
    return row


def bridge_inputs():
    for family in FAMILIES:
        for on_bridge in (False, True):
            suffix = f'_on_bridge_{int(on_bridge)}'
            for name, local_x, level, slope in (
                    ('311', 1, 2, 3), ('312', 0, 3, 1), ('313', 3, 3, 1), ('416', 0, 4, 0)):
                yield bridge_case(family, 'down_' + name + suffix, on_bridge=on_bridge,
                                  current=[1280 + local_x, 1280, -777], cells=[
                                      dict(xy=[5, 5], level=level, slope=slope, flags=0),
                                      dict(xy=[7, 5], level=0, slope=0, flags=0x100)])
                yield bridge_case(family, 'up_' + name + suffix, on_bridge=on_bridge,
                                  current=[1280, 1280, 777], stored_head=[1792 + local_x, 1280, -999], cells=[
                                      dict(xy=[5, 5], level=0, slope=0, flags=0),
                                      dict(xy=[7, 5], level=level, slope=slope, flags=0x100)])
            for flags in (0, 0x400, 0x500):
                yield bridge_case(family, f'down_416_flags_{flags:x}' + suffix, on_bridge=on_bridge,
                                  cells=[dict(xy=[5, 5], level=4, slope=0, flags=0),
                                         dict(xy=[7, 5], level=0, slope=0, flags=flags)])
            for head in ([0, 0, 0], [1, 0, 0], [0, 1, 0], [0, 0, 1]):
                yield bridge_case(family, 'head_' + '_'.join(map(str, head)) + suffix,
                                  on_bridge=on_bridge, stored_head=head,
                                  cells=[dict(xy=[5, 5], level=4, slope=0, flags=0),
                                         dict(xy=[0, 0], level=0, slope=0, flags=0x100)])
            yield bridge_case(family, 'both_null' + suffix, on_bridge=on_bridge,
                              current=[0, 0, 0], stored_head=[0, 0, 0], cells=[
                                  dict(xy=[0, 0], level=-128, slope=0, flags=0x100)])
            yield bridge_case(family, 'head_equals_physical_nav_unrelated' + suffix,
                              on_bridge=on_bridge, stored_head=[1280, 1280, 416])
            yield bridge_case(family, 'null_nav_paid_head' + suffix,
                              on_bridge=on_bridge, nav_com_coord=None)
            for level in (-128, -1, 0, 1, 127):
                yield bridge_case(family, f'signed_level_{level}' + suffix, on_bridge=on_bridge,
                                  cells=[dict(xy=[5, 5], level=level, slope=0, flags=0),
                                         dict(xy=[7, 5], level=0, slope=0, flags=0x100)])
            for slope in range(21):
                yield bridge_case(family, f'slope_{slope}' + suffix, on_bridge=on_bridge,
                                  current=[1293, 1307, 123], stored_head=[1819, 1293, 456], cells=[
                                      dict(xy=[5, 5], level=3, slope=slope, flags=0),
                                      dict(xy=[7, 5], level=-1, slope=20 - slope, flags=0x100)])
            for name, current, head, cells in (
                    ('head_missing_uses_dummy', [1280, 1280, 416], [4864, 1280, 0],
                     [dict(xy=[5, 5], level=4, slope=0, flags=0)]),
                    ('current_missing_uses_dummy', [4864, 1280, 0], [1792, 1280, 416],
                     [dict(xy=[7, 5], level=4, slope=0, flags=0x100)]),
                    ('both_missing_retained_dummy', [-510, 256, 123], [131200, -300, 456], []),
                    ('negative_fraction_real_zero', [-1, -255, 0], [1792, 1280, 17],
                     [dict(xy=[0, 0], level=4, slope=0, flags=0),
                      dict(xy=[7, 5], level=0, slope=0, flags=0x100)]),
                    ('fixed_stride_alias', [1280, 1280, 416], [-256, 256, 17],
                     [dict(xy=[5, 5], level=4, slope=0, flags=0),
                      dict(xy=[511, 0], level=0, slope=0, flags=0x100)]),
                    ('signed_extreme_dummy', [-2147483648, 2147483647, 1],
                     [2147483647, -2147483648, -1], [])):
                yield bridge_case(family, name + suffix, on_bridge=on_bridge, current=current,
                                  stored_head=head, cells=cells,
                                  prefix_queries=[[-256, 0, 999], [2560, 2816, -999]])
            for present, tube in ((True, [10, 20]), (False, [-1, -32768])):
                yield bridge_case(family, f'tube_locomotor_{int(present)}' + suffix,
                                  on_bridge=on_bridge, tube=tube, locomotor_present=present,
                                  stored_head=[0, 0, 0])


def generate_bridge_layers():
    rows = [dict(input=row, output=OriginalBridgeQuery(row).run_bridge()) for row in bridge_inputs()]
    seams = []
    for family in FAMILIES:
        for name, updates, zone_updates in (
                ('paid_head', dict(current=[1283, 1280, 416]), {}),
                ('paid_head_record_missing', dict(current=[1283, 1280, 416]),
                 dict(records=[], raw_labels=[65535, 65535])),
                ('paid_head_split', dict(current=[1283, 1280, 416]),
                 dict(group_cells=[dict(xy=[4, 5], group=1)])),
                ('paid_head_split_equal_labels', dict(current=[1283, 1280, 416]),
                 dict(group_cells=[dict(xy=[4, 5], group=1)], raw_labels=[2, 2])),
                ('up_from_bridge', dict(on_bridge=True, cells=[
                    dict(xy=[5, 5], level=0, slope=0, flags=0),
                    dict(xy=[7, 5], level=4, slope=0, flags=0x100)]), {}),
                ('null_head', dict(stored_head=[0, 0, 0]), {}),
                ('tube_no_locomotor', dict(tube=[-1, -32768], locomotor_present=False,
                                         on_bridge=True, stored_head=[0, 0, 0]), {})):
            row = bridge_case(family, 'response_' + name, **updates)
            victim = dict(family='drive', current=[384, 1408, 777], stored_head=[1152, 1408, -333],
                          on_bridge=False, tube=None, locomotor_present=True)
            movement_zone = {'drive': 0, 'walk': 4, 'ship': 10, 'hover': 3}[family]
            zones = dict(size=[8, 8], bounds=[0, 0, 8, 8], group_cells=[], raw_labels=[2, 3],
                         records=[dict(a=[5, 5], b=[9, 5], active=True, kind=0)])
            zones.update(zone_updates)
            seams.append(dict(input=row, victim=victim, movement_zone=movement_zone, zones=zones,
                              output=OriginalBridgeQuery(row).response_seam(victim, movement_zone, zones)))
    harvest_seams = [dict(input=row['input'], clicked_cell=[4, 5],
                          movement_zone=row['movement_zone'], zones=row['zones'],
                          output=OriginalBridgeQuery(row['input']).harvest_seam(
                              [4, 5], row['movement_zone'], row['zones']))
                     for row in seams if row['input']['family'] == 'drive']
    constructor_rows = []
    for family in ('drive', 'ship'):
        for name, current, on_bridge, frame in (
                ('physical', [1283, 1280, 777], False, 100),
                ('zero', [0, 0, 0], True, 0),
                ('signed', [-257, -1, -999], False, -1)):
            row = bridge_case(family, f'fresh_{family}_constructor_{name}', current=current,
                              stored_head=[0, 0, 0], on_bridge=on_bridge,
                              constructor_prefill_byte=0xA5,
                              scalar_globals={'0x0089E7C0': 104, '0x00AC13C8': 104, '0x00A8ED84': frame})
            constructor_rows.append(OriginalBridgeQuery(row).run_constructed_bridge())
    return dict(native_sha256=NATIVE_SHA256, rows=rows, response_seams=seams, harvest_seams=harvest_seams,
                constructor_rows=constructor_rows,
                original_spans={name: dict(address=f'0x{address:08X}', bytes=file_span(image_bytes(), address, size)[1].hex())
                                for name, (address, size) in BRIDGE_SPANS.items()},
                caller_roles={'infantry': dict(victim_coordinate='0x0070829C', candidate_coordinate='0x007082DB',
                                                candidate_bridge_layer='0x00708319', reach_zone='0x0070833C'),
                              'unit': dict(victim_coordinate='0x007084F4', candidate_coordinate='0x00708533',
                                           candidate_bridge_layer='0x00708571', reach_zone='0x00708594'),
                              'harvest': dict(candidate_coordinate='0x004DCF31', candidate_bridge_layer='0x004DCF6F',
                                              candidate_type='0x004DCF7A', reach_zone='0x004DCF92')})


def bridge_metadata():
    return provenance(
        scope='Original Foot4DBDF0 and Foot4DDC40→Object5F6A70 with original Map578080/565730 and Cell47B3A0; original708080 and4DCE80 coordinate/layer blocks through original56D100 reachability; six original fresh Drive/Ship constructor controls.',
        assumptions=[
            'The416 supplied-state rows use live Walk/Drive/Ship/Hover ILocomotion and original concrete Infantry/Unit tables. Physical XYZ, retained head XYZ, OnBridge, Tube and NavCom are independent inputs. These rows do not execute object construction, head producers or NavCom lifecycle.',
            'NullCoord=(0,0,0), Cell level scalar89E7C0=104, Object level scalarAC13C8=104 and FPCW/ftol control word0E7F supplied explicitly. OriginalCell47B3A0 lazily initializes its own original double slope coefficients; no height or decision replacements.',
            'Sparse fixed512-stride table with capacity40000 and explicit real-cell signed level/slope/flags. Actual shared Dummy retains supplied level/slope/flags while native lookups stamp its packed coordinate. No map loader, topology, passability or zone-map construction claimed.',
            'Original coordinate and bridge queries preserve candidate/locomotor/real-cell bytes. Outputs record original navigation coordinate, AL verdict, ordered exact-XYZ height/structural queries with selected cell, height result and final Dummy state. Tube rows with a null locomotor execute coordinate override and layer bypass without ground queries.',
            'Boundary311/312/313/416 rows use explicit level/slope/local-coordinate inputs established by original ground execution. Signed/extreme and alias rows are scalar witnesses, not a reachability claim on stock maps.',
            'Response seams supply the surrounding scan register/stack context, candidate typeMovementZone and a concrete Unit victim with independent retained/physical XYZ. Original caller instructions execute through original56D100/56D230/56DA10 with explicit Size8,8 Local0,0,8,8 base/raw/active-record state and stop before threat scoring or at reachability refusal. Earlier eligibility, ranking, inactive-record traversal, map loader/zone producer and dispatch are excluded.',
            'Caller victim+4C addresses are70829C/7084F4; candidate+4C are7082DB/708533; candidate+BC are708319/708571; CanReachZone calls are70833C/708594. All six native reachability arguments are observed; fixture movement-zone choices are supplied rather than INI-loaded.',
            'Seven Drive harvest seams execute original4DCF26→56D100→4DCF97 with originalUnit+84 type accessor. The clicked Cell4,5 and prior harvest admission are supplied. Native LandType, CanEnterCell, scan candidate ordering and ore selection are excluded. These are the same layer/zone protocol, not whole harvesting parity.',
            'Six constructor_rows execute original Drive4AF540 or Ship69EC50 and original base55A6C0 on explicitA5 backing bytes, with NullCoord0 and supplied frame100/0/-1. They record original destination/head/vtables/link/frame/selector and full constructor payload before linking. Linking the Foot pointer is supplied afterward; original4DBDF0/4DDC40 then execute. Physical/zero/signed Foot XYZ and OnBridge are supplied. No complete Unit constructor, COM creation/link lifecycle or producer/scheduler claim.',
        ], substitutions=[], entry_points={
            'foot_coordinate': 0x4DBDF0, 'foot_bridge_layer': 0x4DDC40,
            'object_bridge_layer': 0x5F6A70, 'map_ground': 0x578080,
            'cell_ground': 0x47B3A0, 'map_coordinate_cell': 0x565730,
            'response_infantry_start': 0x708291, 'response_unit_start': 0x7084E9,
            'response_infantry_victim_coordinate': 0x70829C,
            'response_infantry_candidate_coordinate': 0x7082DB,
            'response_infantry_bridge_layer': 0x708319, 'response_infantry_reach': 0x70833C,
            'response_unit_victim_coordinate': 0x7084F4,
            'response_unit_candidate_coordinate': 0x708533,
            'response_unit_bridge_layer': 0x708571, 'response_unit_reach': 0x708594,
            'reach_zone': 0x56D100, 'get_zone': 0x56D230,
            'record_lookup': 0x56DA10, 'playfield': 0x578460,
            'harvest_coordinate': 0x4DCF31, 'harvest_bridge_layer': 0x4DCF6F,
            'harvest_type': 0x4DCF7A, 'harvest_reach': 0x4DCF92, 'harvest_stop_boundary': 0x4DCF97,
            'locomotor_base_constructor': 0x55A6C0, 'drive_constructor': 0x4AF540,
            'ship_constructor': 0x69EC50,
        })


if __name__ == '__main__':
    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument('--bridge-layers', action='store_true')
    mode, arguments = parser.parse_known_args()
    if mode.bridge_layers:
        finish_vectors(generate_bridge_layers, Path(__file__).with_name('foot_bridge_layer.json'),
                       provenance=bridge_metadata, argv=arguments, source_paths={
                           'foot_navigation_coordinate.py': Path(__file__),
                           'locomotor_at_coord.py': Path(__file__).with_name('locomotor_at_coord.py'),
                           'map_queries.py': Path(__file__).with_name('map_queries.py'),
                           'native_oracle.py': Path(__file__).parents[1] / 'native_oracle.py',
                       })
    else:
        finish_vectors(generate, Path(__file__).with_suffix('.json'), provenance=lambda: provenance(
        scope='Original Foot4DBDF0 navigation coordinate wrapper, including Tube override and original active Walk/Drive/Ship/Hover ILocomotion+18 head queries. Supplied retained state only; no producers/FindPath/target callbacks.',
        entry_points={'foot_coordinate': 0x4DBDF0, 'tube_override': 0x4DBE01,
                      **{family + '_head': state['head'] for family, state in FAMILIES.items()}},
        assumptions=['Original concrete Infantry table for Walk and Unit table for the other three active families. Original ILocomotion tables and supplied link pointer; no callable replacement.',
                     'Native null-coordinate globals are supplied zero. Retained XYZ and physical XYZ are independent and observed after each call. Track selectors and cursors cannot gate HeadTo dispatch.',
                     'Foot+684=-1 without a Tube; explicit Tube rows supply index0 and a one-entry Tube table with signed exit CellStruct. Wrapper must bypass the locomotor query and return signed exit center with Z0.',
                     'Inherited OriginalQuery maps verified unmodified retail bytes and observes existing helper addresses; its separate IsAtCoord and track transform methods are not invoked.'],
        substitutions=[]), argv=arguments)
