"""Original sidebar comparator, admitted insertion and retained removal order.

The comparator and insertion execute whole, with original RTTI, cost virtuals
and wcscmp. AddCameo stops after insertion or at its rejection epilogue; its
presentation tail is outside this fixture. Retained histories stop Recalculate
after filtering; separate scroll controls execute it through its return.
See cameo_order.md for boundaries.
No Python implementation supplies an expected comparison or ordered list.
"""

from itertools import product
from pathlib import Path
import struct

from unicorn import UC_HOOK_CODE
from unicorn.x86_const import (
    UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX,
    UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP,
    UC_X86_REG_FPCW,
)

from tools.native_oracle import (
    RET_MAGIC, STACK_BASE, STACK_SIZE, finish_vectors, provenance, run_checked,
)
from tools.sidebar_oracle.geometry import machine, put32, read
from tools.spatial_oracle.cost_of import VTABLE, f32
from tools.spatial_oracle.building_body_rules import (
    Fixture as IniFixture, INI, SP as INI_SP, TYPE as INI_TYPE, dwords,
)

COMPARE = 0x6A8420
INSERT = 0x6A8710
ADD = 0x6A6300
ADD_ACCEPTED, ADD_REJECTED = 0x6A6423, 0x6A65FF
RECALCULATE, FILTER_DONE = 0x6AA600, 0x6AAAB3
SIDEBAR = 0x87F7E8
HEAP, HEAP_SIZE = 0x21000000, 0x00400000
STACK = STACK_BASE + STACK_SIZE - 0x1000
STRIPS = [SIDEBAR + 0x1544 + index * 0xF94 for index in range(4)]
ARRAYS = {40: 0xA83CE4, 3: 0xA8B21C, 7: 0xA83C6C,
          16: 0xA8E34C, 31: 0xA8E334}
TYPE_NAMES = {40: 'unit', 3: 'aircraft', 7: 'building', 16: 'infantry'}
SUPER_KINDS = (31, 32, 57)


def item(name, kind=40, *, side=0, tech_level=1, cost=500, ui_name='Same',
         considered_aircraft=False, naval=False, build_cat=0,
         recharge_frames=4500, free_unit=None):
    return dict(id=name, kind=kind, side=side, tech_level=tech_level, cost=cost,
                ui_name=ui_name, considered_aircraft=considered_aircraft,
                naval=naval, build_cat=build_cat, recharge_frames=recharge_frames,
                free_unit=free_unit)


class Sidebar:
    """Supplied initialized type/House objects; original ordering instructions."""

    def __init__(self, catalog, *, side=0, plant=None, country=None):
        self.u = machine()
        self.u.mem_map(HEAP, HEAP_SIZE)
        self.cursor = HEAP
        self.catalog = {entry['id']: dict(entry) for entry in catalog}
        self.pointers = {}
        self.lookup = {}
        self.events = []
        self.scroll_observation = None
        self.available = {}
        self.can_build = {}
        self.house = self.alloc(0x6000)
        self.country = self.alloc(0x400)
        self.rules = self.alloc(0x2000)
        self.pad_aircraft = self.alloc(0x400)
        self.pad_vector = self.alloc(8)
        self.pad_dock = self.alloc(4)
        self.scenario = self.alloc(0x4000)
        self.factory = self.alloc(0x300)
        self.super_instances = self.alloc(512)
        self.visible_copy = self.alloc(0x1000)
        self.arrays = {kind: self.alloc(512) for kind in ARRAYS}
        u = self.u
        u.reg_write(UC_X86_REG_FPCW, 0x0E7F)
        put32(u, 0xA83D4C, self.house)
        put32(u, self.house + 0x34, self.country)
        put32(u, self.house + 0x258, self.super_instances)
        put32(u, self.factory + 0x21C, self.house)
        put32(u, self.country + 0xBC, side)
        put32(u, 0x8871E0, self.rules)
        put32(u, self.rules + 0xB5C, self.pad_vector)
        put32(u, self.pad_vector, self.pad_aircraft)
        put32(u, self.pad_aircraft + 0x3EC, self.pad_dock)
        put32(u, 0xA8B230, self.scenario)
        # The cost owner reads SeparateAircraft; no bundled pad fixture here.
        u.mem_write(self.rules + 0x17E8, b'\x01')
        self.set_factors(plant=plant, country=country)
        # Three visible rows in Recalculate's original geometry computation.
        put32(u, 0x886F9C, 341)
        put32(u, 0x886F94, 0)
        put32(u, 0xB0B4F8, 158)
        put32(u, 0xA8ED84, 123)
        u.mem_write(0xA8ED6B, b'\0')
        put32(u, 0xA8E7AC, 0)
        for kind, address in ARRAYS.items():
            put32(u, address, self.arrays[kind])
        counts = {kind: 0 for kind in ARRAYS}
        for entry in self.catalog.values():
            kind = 31 if entry['kind'] in SUPER_KINDS else entry['kind']
            index = counts[kind]
            counts[kind] += 1
            entry['type_index'] = index
            ptr = self.alloc(0x1800)
            self.pointers[entry['id']] = ptr
            self.lookup[(entry['kind'], index)] = entry['id']
            put32(u, self.arrays[kind] + index * 4, ptr)
            name = (entry['ui_name'] + '\0').encode('utf-16-le')
            text = self.alloc(len(name))
            u.mem_write(text, name)
            put32(u, ptr + 0x60, text)
            if kind == 31:
                put32(u, ptr + 0xB0, entry['recharge_frames'])
                super_ptr = self.alloc(0x80)
                put32(u, self.super_instances + index * 4, super_ptr)
                u.mem_write(super_ptr + 0x6D, b'\x01')
            else:
                put32(u, ptr, VTABLE[TYPE_NAMES[kind]])
                put32(u, ptr + 0x610, entry['cost'])
                put32(u, ptr + 0x634, entry['tech_level'])
                put32(u, ptr + 0x6D0, entry['side'])
                put32(u, ptr + 0xE08, entry['build_cat'])
                u.mem_write(ptr + 0xCCE, bytes([entry['naval']]))
                u.mem_write(ptr + 0xD96, bytes([entry['considered_aircraft']]))
                self.available[ptr] = True
                self.can_build[ptr] = 1
        put32(u, self.house + 0x264, counts[31])
        for entry in self.catalog.values():
            if entry['free_unit']:
                put32(u, self.pointers[entry['id']] + 0xEA0,
                      self.pointers[entry['free_unit']])
        self.factory_queries = {
            read(u, table + 0x94)[0] for table in VTABLE.values()
        }
        u.hook_add(UC_HOOK_CODE, self.hook)
        for tab, strip in enumerate(STRIPS):
            self.invoke(0x6A80A0, strip, (0,), context={'case': 'strip-constructor', 'tab': tab})
            put32(u, strip + 0x38, tab)
        self.events = []

    def alloc(self, size):
        ptr = self.cursor
        self.cursor += (size + 15) & ~15
        if self.cursor > HEAP + HEAP_SIZE:
            raise RuntimeError('sidebar oracle heap exhausted')
        return ptr

    def set_factors(self, *, plant=None, country=None):
        self.plant = list(plant or [1.0] * 5)
        self.country_factors = list(country or [1.0] * 5)
        self.u.mem_write(self.house + 0x5390, dwords(*(f32(x) for x in self.plant)))
        self.u.mem_write(self.country + 0x114, dwords(*(f32(x) for x in self.country_factors)))

    def native_return(self, value, pop):
        u = self.u
        sp = u.reg_read(UC_X86_REG_ESP)
        target = struct.unpack('<I', u.mem_read(sp, 4))[0]
        u.reg_write(UC_X86_REG_EAX, value & 0xFFFFFFFF)
        u.reg_write(UC_X86_REG_ESP, sp + 4 + pop)
        u.reg_write(UC_X86_REG_EIP, target)

    def hook(self, u, address, _size, _data):
        if address == 0x752700:
            self.events.append('EVA_NewConstructionOptions')
            self.native_return(0, 4)
        elif address == 0x7C8E17:
            self.events.append('visible-copy-allocation')
            sp = u.reg_read(UC_X86_REG_ESP)
            size = struct.unpack('<I', u.mem_read(sp + 4, 4))[0]
            if size > 0x1000:
                raise RuntimeError(f'unexpected visible-copy allocation {size}')
            u.mem_write(self.visible_copy, bytes(size))
            self.native_return(self.visible_copy, 0)
        elif address == 0x6A6610:
            self.events.append('UpdateScrollButtons')
            self.native_return(0, 0)
        elif address == 0x7C8B3D:
            sp = u.reg_read(UC_X86_REG_ESP)
            pointer = struct.unpack('<I', u.mem_read(sp + 4, 4))[0]
            if pointer != self.visible_copy:
                raise RuntimeError(f'unexpected Recalculate delete {pointer:#x}')
            self.events.append('visible-copy-delete')
            self.native_return(0, 0)
        elif address in self.factory_queries:
            ptr = u.reg_read(UC_X86_REG_ECX)
            self.events.append('supplied-FindFactory')
            self.native_return(self.factory if self.available[ptr] else 0, 16)
        elif address == 0x4F7870:
            sp = u.reg_read(UC_X86_REG_ESP)
            ptr = struct.unpack('<I', u.mem_read(sp + 4, 4))[0]
            self.events.append('supplied-CanBuild')
            self.native_return(self.can_build[ptr], 12)
        elif address in (0x711F00, 0x45EDD0):
            self.events.append('Cost_Of')
        elif address == 0x7CA5D3:
            self.events.append('wcscmp')
        elif address == COMPARE:
            self.events.append('CompareItems')
        elif self.scroll_observation is not None and address in (0x6AA711, FILTER_DONE):
            slots = read(u, self.visible_copy)[0]
            visible = []
            for index in range(slots):
                type_index, kind = read(u, self.visible_copy + 4 + index * 0x34, 2)
                visible.append(None if (kind, type_index) == (0, 0)
                               else self.lookup[(kind, type_index)])
            self.scroll_observation['visible_slots'] = slots
            key = 'snapshot_before' if address == 0x6AA711 else 'snapshot_after'
            self.scroll_observation[key] = visible

    def invoke(self, address, obj=0, args=(), *, ends=RET_MAGIC, context=None):
        u = self.u
        u.mem_write(STACK, dwords(RET_MAGIC, *args))
        u.reg_write(UC_X86_REG_ESP, STACK)
        u.reg_write(UC_X86_REG_ECX, obj)
        return run_checked(u, address, ends, count=200_000,
                           required_addresses=(address,), context=context)

    def compare(self, left, right):
        lhs, rhs = self.catalog[left], self.catalog[right]
        self.events = []
        self.invoke(COMPARE, args=(lhs['kind'], lhs['type_index'],
                                  rhs['kind'], rhs['type_index']),
                    context={'case': 'compare', 'left': left, 'right': right})
        return dict(left=left, right=right, before_or_equal=bool(self.u.reg_read(UC_X86_REG_EAX) & 255),
                    cost_calls=self.events.count('Cost_Of'), name_compared='wcscmp' in self.events)

    def cost(self, name):
        ptr = self.pointers[name]
        table = read(self.u, ptr)[0]
        target = read(self.u, table + 0x84)[0]
        self.invoke(target, ptr, (self.house,), context={'case': 'cost-input', 'id': name})
        return struct.unpack('<i', dwords(self.u.reg_read(UC_X86_REG_EAX)))[0]

    def order(self):
        result = []
        for strip in STRIPS:
            entries = []
            for index in range(read(self.u, strip + 0x54)[0]):
                type_index, kind = read(self.u, strip + 0x58 + index * 0x34, 2)
                entries.append(self.lookup[(kind, type_index)])
            result.append(entries)
        return result

    def add(self, name):
        entry = self.catalog[name]
        self.events = []
        reached = self.invoke(ADD, SIDEBAR, (entry['kind'], entry['type_index']),
                              ends=(ADD_ACCEPTED, ADD_REJECTED),
                              context={'case': 'add', 'id': name})
        return dict(action='add', id=name, accepted=reached == ADD_ACCEPTED,
                    order=self.order(), eva_calls=self.events.count('EVA_NewConstructionOptions'))

    def recalculate(self, tab, *, complete=False):
        self.events = []
        self.scroll_observation = {} if complete else None
        self.invoke(RECALCULATE, STRIPS[tab],
                    ends=RET_MAGIC if complete else (FILTER_DONE, RET_MAGIC),
                    context={'case': 'recalculate-complete' if complete else 'recalculate-filter',
                             'tab': tab})
        if 'CompareItems' in self.events or 'Cost_Of' in self.events or 'wcscmp' in self.events:
            raise RuntimeError('unexpected re-sort during native retained-entry filter')
        if complete:
            result = dict(self.scroll_observation, order=self.order()[tab],
                          top_row_after=read(self.u, STRIPS[tab] + 0x44)[0],
                          removed=bool(self.u.reg_read(UC_X86_REG_EAX) & 255),
                          needs_full_redraw=bool(self.u.mem_read(SIDEBAR + 0x53A7, 1)[0]),
                          strip_needs_redraw=bool(self.u.mem_read(STRIPS[tab] + 0x3C, 1)[0]),
                          scroll_button_updates=self.events.count('UpdateScrollButtons'),
                          visible_copy_frees=self.events.count('visible-copy-delete'))
            self.scroll_observation = None
            return result
        return dict(action='recalculate', tab=tab, order=self.order())


def recalculate_scroll_controls():
    """Whole native filter/snapshot/row-adjustment with observed UI/delete sinks.

    All expected orders, snapshots, capacities and rows come from execution.
    Python only selects supplied layouts, starting rows and removal inputs.
    Each source strip is first populated through original AddCameo/InsertEntry.
    """
    catalog = [item(f'unit_{i:02}', cost=100 + i, ui_name=f'Unit {i:02}')
               for i in range(25)]
    m = Sidebar(catalog)
    for tab in range(4):
        m.invoke(0x69DCF0, 0xB07C48 + tab * 0x60,
                 context={'case': 'tab-gadget-constructor', 'tab': tab})
    # Original static initializer6A4CE0 uses these same four ctor calls. They
    # allow the all-entries-gone path to execute actual Gadget disable dispatch.
    layouts = [
        ('two_slots', 1, dict(body_height=241, body_y=0, sidebar_top=158, scenario_side=0)),
        ('four_slots', 2, dict(body_height=291, body_y=0, sidebar_top=158, scenario_side=0)),
        ('six_slots', 3, dict(body_height=341, body_y=0, sidebar_top=158, scenario_side=0)),
        ('ten_slots', 5, dict(body_height=441, body_y=0, sidebar_top=158, scenario_side=0)),
    ]
    specs = []
    six_slots = layouts[2][2]
    for label, removed in (
        ('none', []), ('remove0', [0]), ('remove1', [1]), ('remove2', [2]),
        ('remove3', [3]), ('remove6', [6]), ('remove7', [7]),
        ('remove8', [8]), ('remove19', [19]), ('remove0_1', [0, 1]),
        ('remove0_2', [0, 2]), ('remove2_3', [2, 3]),
        ('visible_gone', list(range(2, 8))), ('prefix_gone', list(range(8))),
        ('all_gone', list(range(20))),
    ):
        specs.append((f'twenty_six_slots_row1_{label}', 20, 1, six_slots, removed))
    for count, layout in product((1, 2, 5, 6, 7, 8, 12, 19, 20, 21, 24, 25), layouts):
        layout_name, rows, geometry = layout
        # Only input selection: the last ordinary scroll row is admitted while
        # (row + visible_rows) * 2 < count at6A8BFF..6A8C09, then row increments.
        last_row = max(0, (count + 1) // 2 - rows)
        for row in sorted({0, last_row}):
            for label, removed in (
                ('none', []), ('last', [count - 1]),
                ('first_visible', [row * 2]),
                ('visible_gone', list(range(row * 2, min(count, row * 2 + rows * 2)))),
                ('all_gone', list(range(count))),
            ):
                specs.append((f'count{count}_{layout_name}_row{row}_{label}',
                              count, row, geometry, removed))
    # Nonzero sidebar origin and either non-Allied scenario side exercise the
    # original eight-pixel footer difference without supplying its output.
    for side in (1, 2):
        geometry = dict(body_height=309, body_y=24, sidebar_top=158, scenario_side=side)
        specs.append((f'nonzero_origin_side{side}_remove19', 20, 1, geometry, [19]))
    # Exact division boundary and a zero-slot viewport use the original
    # capacity arithmetic; the latter must take the no-survivor branch.
    for height in (240, 340, 342):
        geometry = dict(body_height=height, body_y=0, sidebar_top=158, scenario_side=0)
        specs.append((f'capacity_boundary_height{height}', 20, 1, geometry, [19]))

    source_strips = {}
    cases = []
    for label, count, row, geometry, removed in specs:
        if count not in source_strips:
            m.invoke(0x6A81B0, STRIPS[3], context={'case': 'scroll-source-clear'})
            for name in list(m.catalog)[:count]:
                if not m.add(name)['accepted']:
                    raise RuntimeError(f'native scroll fixture failed to admit {name}')
            source_strips[count] = bytes(m.u.mem_read(STRIPS[3], 0xF94))
        # Restore a previously executed native source state for independent
        # cases; do not synthesize or sort a candidate result in Python.
        m.u.mem_write(STRIPS[3], source_strips[count])
        put32(m.u, STRIPS[3] + 0x44, row)
        put32(m.u, SIDEBAR + 0x539C, 3)
        m.u.mem_write(SIDEBAR + 0x53A7, b'\0')
        m.u.mem_write(STRIPS[3] + 0x3C, b'\0')
        for key, address in (('body_height', 0x886F9C), ('body_y', 0x886F94),
                             ('sidebar_top', 0xB0B4F8), ('scenario_side', m.scenario + 0x34B8)):
            put32(m.u, address, geometry[key])
        initial_order = m.order()[3]
        remove_ids = [initial_order[index] for index in removed]
        for name, pointer in m.pointers.items():
            m.available[pointer] = name not in remove_ids
        observed = m.recalculate(3, complete=True)
        cases.append(dict(id=label, initial_order=initial_order, remove_ids=remove_ids,
                          top_row_before=row, geometry=geometry, **observed))
    return dict(types=list(m.catalog.values()), cases=cases)


def comparison_catalog():
    return [
        item('ground'), item('air', considered_aircraft=True),
        item('naval', naval=True), item('air_naval', considered_aircraft=True, naval=True),
        item('aircraft_ground', 3), item('aircraft_air', 3, considered_aircraft=True),
        item('aircraft_naval', 3, naval=True),
        item('aircraft_both', 3, considered_aircraft=True, naval=True),
        item('infantry', 16), item('infantry_flags_ignored', 16, considered_aircraft=True, naval=True),
        item('building', 7), item('defense', 7, build_cat=5),
        item('other_side', side=1, tech_level=-1, cost=-10, ui_name='A'),
        item('third_side', side=2, tech_level=-1, cost=-10, ui_name='A'),
        item('negative_tech', tech_level=-1), item('high_tech', tech_level=2147483647),
        item('low_cost', cost=1), item('high_cost', cost=1001), item('negative_cost', cost=-10),
        item('empty_name', ui_name=''), item('upper_name', ui_name='Zulu'),
        item('lower_name', ui_name='alpha'), item('accented_name', ui_name='\u00c5ngstr\u00f6m'),
        item('bmp_name', ui_name='\ue000'), item('supplementary_name', ui_name='\U00010000'),
        item('same_tie'), item('super', 31), item('short_super', 31, recharge_frames=900),
        item('negative_super', 31, recharge_frames=-1), item('super_same_name', 31),
        item('super_name', 31, ui_name='Zulu'), item('super_kind32', 32), item('super_kind57', 57),
    ]


def pair_controls():
    catalog = comparison_catalog()
    m = Sidebar(catalog)
    names = list(m.catalog)
    rows = [m.compare(left, right) for left, right in product(names, repeat=2)]
    return dict(types=list(m.catalog.values()), current_side=0, country=[1.0] * 5,
                plant=[1.0] * 5, cases=rows)


def cost_contexts():
    catalog = [item('unit', cost=500), item('aircraft', 3, cost=400),
               item('infantry', 16, cost=300), item('plain_building', 7, cost=900),
               item('free_unit_building', 7, cost=1000, free_unit='unit'),
               item('defense', 7, cost=800, build_cat=5)]
    rows = []
    for label, country, plant in (
        ('unadjusted', [1.0] * 5, [1.0] * 5),
        ('unit_plant_discount', [1.0] * 5, [1.0, 0.75, 1.0, 1.0, 1.0]),
        ('different_country_slots', [1.5, 2.0, 0.5, 0.75, 0.25], [1.0] * 5),
        ('combined_factors', [1.5, 2.0, 0.5, 0.75, 0.25], [0.5, 0.75, 1.0, 0.5, 1.0]),
    ):
        m = Sidebar(catalog, country=country, plant=plant)
        rows.append(dict(id=label, types=list(m.catalog.values()), current_side=0,
                         country=country, plant=plant,
                         native_costs={name: m.cost(name) for name in m.catalog},
                         cases=[m.compare(a, b) for a, b in product(m.catalog, repeat=2)]))
    return rows


def histories():
    catalog = [
        item('power', 7, tech_level=1, cost=800, ui_name='Power'),
        item('refinery', 7, tech_level=1, cost=600, ui_name='Refinery'),
        item('defense', 7, build_cat=5, ui_name='Defense'),
        item('gi', 16, cost=200, ui_name='GI'), item('engineer', 16, cost=500, ui_name='Engineer'),
        item('tank', cost=1000, ui_name='Tank'), item('aircraft', 3, cost=800, ui_name='Aircraft'),
        item('new_vehicle', cost=1100, ui_name='New'), item('unit_tie_a', cost=1000, ui_name='Tank'),
        item('unit_tie_b', cost=1000, ui_name='Tank'),
        item('super_long', 31, recharge_frames=9000, ui_name='Long'),
        item('super_short', 31, recharge_frames=4500, ui_name='Short'),
    ]
    rows = []
    for label, sequence in (
        ('registration_order', [entry['id'] for entry in catalog]),
        ('reverse_registration', [entry['id'] for entry in reversed(catalog)]),
        ('mixed_unlock_batches', ['tank', 'gi', 'power', 'defense', 'super_long', 'aircraft',
                                 'engineer', 'unit_tie_a', 'refinery', 'super_short', 'unit_tie_b']),
    ):
        m = Sidebar(catalog)
        initial_costs = {name: m.cost(name) for name, entry in m.catalog.items()
                         if entry['kind'] not in SUPER_KINDS}
        steps = [m.add(name) for name in sequence]
        steps += [m.add(sequence[0]), m.add(sequence[-1])]
        # A late Industrial Plant changes the cross-class cost ordering but does
        # not trigger native comparisons for existing entries during Recalculate.
        m.set_factors(plant=[1.0, 0.75, 1.0, 1.0, 1.0])
        steps.append(dict(action='plant_factors', values=m.plant, order=m.order(),
                          native_costs={name: m.cost(name) for name, entry in m.catalog.items()
                                        if entry['kind'] not in SUPER_KINDS}))
        steps.extend(m.recalculate(tab) for tab in range(4))
        steps.append(m.add('new_vehicle'))
        m.available[m.pointers['tank']] = False
        steps.append(dict(action='factory_available', id='tank', value=False))
        m.can_build[m.pointers['unit_tie_a']] = 0
        steps.append(dict(action='can_build', id='unit_tie_a', value=0))
        steps.append(m.recalculate(3))
        m.available[m.pointers['tank']] = True
        m.can_build[m.pointers['unit_tie_a']] = 1
        steps.append(dict(action='factory_available', id='tank', value=True))
        steps.append(dict(action='can_build', id='unit_tie_a', value=1))
        steps.extend((m.add('tank'), m.add('unit_tie_a')))
        sw = m.catalog['super_short']['type_index']
        pointer = struct.unpack('<I', m.u.mem_read(m.super_instances + 4 * sw, 4))[0]
        m.u.mem_write(pointer + 0x6D, b'\0')
        steps.append(dict(action='super_granted', id='super_short', value=False))
        steps.append(m.recalculate(1))
        m.u.mem_write(pointer + 0x6D, b'\x01')
        steps.append(dict(action='super_granted', id='super_short', value=True))
        steps.append(m.add('super_short'))
        for strip in STRIPS:
            m.invoke(0x6A81B0, strip, context={'case': 'strip-init-clear'})
        steps.append(dict(action='init_clear', order=m.order()))
        steps.extend(m.add(name) for name in ('unit_tie_a', 'tank', 'unit_tie_b'))
        rows.append(dict(id=label, types=list(m.catalog.values()), current_side=0,
                         initial_native_costs=initial_costs, steps=steps))
    return rows


def capacity_history():
    # Original AddCameo admits count75, despite the original 75-record strip
    # allocation. These boundary controls deliberately observe that extra record
    # in mapped adjacent storage. The AddCameo/Remove UI tails do not execute and
    # no claim is made that the resulting complete native Sidebar is valid.
    catalog = [item(f'unit_{i:02}', cost=100 + i, ui_name=f'Unit {i:02}')
               for i in range(77)]
    m = Sidebar(catalog)
    initial = list(m.catalog)[:76]
    for name in initial:
        result = m.add(name)
        if not result['accepted']:
            raise RuntimeError(f'native capacity fixture failed to admit {name}')
    before = m.order()
    m.available[m.pointers['unit_00']] = False
    steps = [dict(action='factory_available', id='unit_00', value=False),
             m.add('unit_76'), m.recalculate(3), m.add('unit_76')]
    return dict(types=list(m.catalog.values()), current_side=0, initial_adds=initial,
                initial_order=before, steps=steps,
                limitation='The native gate allows76 records in75-record storage; this fixture '
                           'observes adjacent mapped bytes only and excludes the UI tail. '
                           'A safe Rust vector may reproduce admission without native memory corruption.')


def recharge_reader():
    """Original ctor stores and reader slice; supplied signed-CRC INI cache."""
    m = IniFixture()
    u = m.u
    u.mem_write(INI_SP, dwords(RET_MAGIC))
    u.reg_write(UC_X86_REG_ESP, INI_SP)
    run_checked(u, 0x7C8F5E, RET_MAGIC)
    u.reg_write(UC_X86_REG_EBP, INI_TYPE)
    run_checked(u, 0x6CE5C0, 0x6CE681, required_addresses=(0x6CE5E9,))
    initial = read(u, INI_TYPE + 0xB0)[0]
    rows = []
    for raw in (None, '0', '5', '10', '3.5', '0.01', '-1', '50%', '0', None):
        before = read(u, INI_TYPE + 0xB0)[0]
        m.ini(0x842634, raw)
        u.reg_write(UC_X86_REG_ESP, INI_SP)
        u.reg_write(UC_X86_REG_EBP, INI_TYPE)
        u.reg_write(UC_X86_REG_EBX, INI)
        u.reg_write(UC_X86_REG_ESI, INI_TYPE + 0x1F8)
        run_checked(u, 0x6CED5C, 0x6CED95, required_addresses=(0x5283D0,),
                    context={'case': 'RechargeTime-reader', 'raw': raw, 'retained': before})
        rows.append(dict(raw=raw, before=before, recharge_frames=read(u, INI_TYPE + 0xB0)[0]))
    return dict(constructor_frames=initial, cases=rows)


def generate():
    return dict(schema=1, comparisons=pair_controls(), histories=histories(),
                cost_contexts=cost_contexts(), capacity_history=capacity_history(),
                recharge_reader=recharge_reader(),
                recalculate_scroll=recalculate_scroll_controls())


def metadata():
    return provenance(
        scope='Original whole Strip CompareItems/InsertEntry, AddCameo admission through insertion, '
              'Recalculate retained filter/removal and complete pruning-scroll controls, '
              'and SuperWeaponType RechargeTime reader block. '
              'Synthetic initialized inputs; not a complete tech-tree, native scene, or whole-sidebar claim.',
        assumptions=[
            'Fresh PE mapping per case family. Actual RTTI_To_TypeArray, original type vtables, '
            'Cost_Of country/FactoryPlant arithmetic and UTF-16 wcscmp execute. Type/House inputs '
            'are supplied fields, not a claim of native constructor/INI initialization. FPCW0E7F.',
            'All 33 by33 pair combinations include type-class flag gating, both flags set, same-side '
            'priority, signed TechLevel/cost/recharge, name case, supplementary UTF-16 and exact ties.',
            'Four six-type cost contexts execute24 complete cost queries and144 comparisons with '
            'country/plant factors, all five cost slots and the BuildingType FreeUnit adjustment. '
            'PadAircraft contains an initialized nonmatching first Dock; SeparateAircraft is true.',
            'Four strips are constructed by original6A80A0. AddCameo starts at6A6300 and stops at '
            '6A6423 after original insertion or6A65FF before rejection return. Its remaining UI '
            'presentation tail is excluded. The three ordinary histories remain within75 slots.',
            'A separate capacity history executes the native76th-entry overrun into mapped adjacent '
            'storage, then rejection at76, prune to75 and retry. It proves the admission gate and '
            'addition-before-removal consequence, not integrity of a native oversized Sidebar. '
            'Rust must keep memory safe; adjacent overwritten state is not a golden.',
            'Recalculate starts at6AA600 and stops at6AAAB3 after the complete retained filter. '
            'Supers granted bits are supplied. Entries have no linked production factories and '
            'House primary-factory fields are zero; original500510 executes on removals. '
            'The original histories exclude cancellation events, tab selection and scroll adjustment.',
            'The additive recalculate_scroll cases instead execute whole6AA600 to its RET. '
            'Original AddCameo populates the source vehicle strip; cases restore those executed '
            'bytes and supply TopRow, geometry and unavailable IDs. Actual prologue capacity, '
            'visible record snapshots, snapshot erasure and6AABC9..6AAC76 row adjustment execute. '
            'No Python algorithm supplies expected rows. Original69DCF0 initializes all four '
            'tab gadgets so the all-entries-gone disable path runs. Other strips remain empty; '
            'switching to another populated tab and linked-factory cancellation remain excluded.',
            'RechargeTime uses original constructor6CE5C0..6CE681 field initialization and '
            'reader6CED5C..6CED95 with original ReadDouble and ftol, supplied cached INI lookup, '
            'and original floating-scanner startup. Whole SuperWeaponType reader is excluded.',
        ],
        substitutions=[
            'EVA752700 is an observed no-op sink with its actual RET4 convention.',
            'Recalculate allocator7C8E17 supplies a zeroed visible-copy buffer. Type virtual+94 '
            'FindFactory and House4F7870 CanBuild return declared availability inputs. '
            'Admission producer bodies and complete House/Building lifecycle are not executed.',
            'Complete pruning-scroll controls observe UpdateScrollButtons6A6610 and '
            'operator_delete7C8B3D as no-op sinks with their actual caller-cleaned conventions. '
            'Delete must receive the original supplied allocation. No button appearance or '
            'allocator-internal claim; zero-filled unused snapshot slots are an explicit input.',
        ], entry_points={'compare': COMPARE, 'insert': INSERT, 'add': ADD,
                         'recalculate': RECALCULATE, 'strip_ctor': 0x6A80A0,
                         'strip_clear': 0x6A81B0, 'wcscmp': 0x7CA5D3,
                         'prune_scroll_tail': 0x6AABC9, 'tab_gadget_ctor': 0x69DCF0,
                         'super_ctor_fields': 0x6CE5C0, 'recharge_reader': 0x6CED5C})


if __name__ == '__main__':
    finish_vectors(generate, Path(__file__).with_suffix('.json'), provenance=metadata,
                   source_paths={
                       'producer': Path(__file__),
                       'sidebar_machine': Path(__file__).with_name('geometry.py'),
                       'cost_owner': Path(__file__).parents[1] / 'spatial_oracle/cost_of.py',
                       'ini_fixture': Path(__file__).parents[1] / 'spatial_oracle/building_body_rules.py',
                       'runner': Path(__file__).parents[1] / 'native_oracle.py',
                   })
