"""Original enqueue-counter ore queue rebuilds over the shared harvest fixture.

Run ``python -m tools.spatial_oracle.ore_queue --check`` (or ``--write``).
Prepared map/queue state is explicit; AddToGrowth/Spread and both rebuild bodies,
their CellIterator/predicates, heap writes and Scenario Random execute unchanged.
"""
import hashlib
from pathlib import Path
import struct

from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_ESP

from tools.native_oracle import finish_vectors, provenance
from tools.spatial_oracle.harvest_field import (
    ADD_GROWTH, ADD_SPREAD, FIELD, REDUCE, TIBS, fixture,
)
from tools.spatial_oracle.map_queries import dwords
from tools.spatial_oracle.refinery_dock import cell, cell_xy
from tools.spatial_oracle.unit_source_scatter import MAP, SCENARIO, TABLE

REBUILD_GROWTH, REBUILD_SPREAD, RANDOM = 0x7233A0, 0x7228B0, 0x65C780
GROWTH_PROCESSOR, GROW_CELL = 0x722F00, 0x483710
ARENA, ARENA_SIZE = FIELD + 0x200000, 0x20000
FIELDS = {'growth': (0x10C, 0x110, 0x114, 0x118), 'spread': (0xF0, 0xF4, 0xF8, 0xFC)}
DEFAULT_CELLS = [
    dict(cell=[4, 4], type_id=0, variant=0, density=3, slope=0, occupied=False),
    dict(cell=[1, 4], type_id=0, variant=0, density=2, slope=0, occupied=False),
    dict(cell=[2, 4], type_id=0, variant=0, density=5, slope=0, occupied=False),
    dict(cell=[3, 3], type_id=1, variant=0, density=2, slope=0, occupied=False),
    dict(cell=[5, 3], type_id=0, variant=0, density=11, slope=0, occupied=False),
    dict(cell=[3, 5], type_id=0, variant=0, density=3, slope=1, occupied=False),
    dict(cell=[3, 4], type_id=0, variant=0, density=4, slope=0, occupied=True),
]


def prepared_case(name, queue, count, **overrides):
    case = dict(name=name, entry='enqueue', queue=queue, receiver=0, size=[4, 4],
                target=[4, 4], frame=100, seed=1, grows=True, spreads=True,
                growth_percentages=[0.125] * 4, spread_percentages=[0.125] * 4,
                pre_count=count, pre_cell=[3, 3], pre_priority_bits=0x42480000,
                pre_heap_indices=[0], pre_bitmap=[[3, 3]], cells=DEFAULT_CELLS)
    return dict(case, **overrides)


def changed_cells(**updates):
    return [dict(c, **updates) if c['cell'] == [4, 4] else dict(c) for c in DEFAULT_CELLS]


def cases():
    rows = []
    for queue, counts in [('growth', (53, 54, 55)), ('spread', (43, 44, 45))]:
        for count in counts:
            rows.append(prepared_case(f'{queue}_counter_{count}', queue, count))
        edge = counts[-1]
        rows += [prepared_case(f'{queue}_empty_heap_rebuild', queue, edge, pre_heap_indices=[]),
                 prepared_case(f'{queue}_late_heap_reference', queue, edge,
                               pre_heap_indices=[edge - 1]),
                 prepared_case(f'{queue}_seed_31', queue, edge, seed=31),
                 prepared_case(f'{queue}_seed_42', queue, edge, seed=42),
                 prepared_case(f'{queue}_negative_frame', queue, edge, frame=-100),
                 prepared_case(f'{queue}_large_frame', queue, edge, frame=16777217),
                 prepared_case(f'{queue}_chopped_f32_frame', queue, edge, frame=16777220),
                 prepared_case(f'{queue}_frame_wrap', queue, edge, frame=2147483647),
                 prepared_case(f'{queue}_frame_min', queue, edge, frame=-2147483648),
                 prepared_case(f'{queue}_crossclass_receiver', queue, edge, receiver=1),
                 prepared_case(f'{queue}_dormant_receiver', queue, edge, receiver=2)]
        for raw in [0, 0x7FFFFFFF, 0x80000000, 0xFFFFFFFF]:
            rows.append(prepared_case(f'{queue}_raw_{raw:08x}', queue, edge, next_raw=raw))
    rows += [
        prepared_case('growth_density_11_rejects_before_rebuild', 'growth', 55,
                      cells=changed_cells(density=11)),
        prepared_case('growth_density_10_admits', 'growth', 55,
                      cells=changed_cells(density=10)),
        prepared_case('growth_queued_bit_does_not_dedupe', 'growth', 55,
                      pre_bitmap=[[3, 3], [4, 4]]),
        prepared_case('growth_disabled_rebuild_then_append', 'growth', 55, grows=False),
        prepared_case('growth_zero_percentage_rebuild_then_append', 'growth', 55,
                      growth_percentages=[0.0] * 4),
        prepared_case('growth_sloped_request_rebuild_then_append', 'growth', 55,
                      cells=changed_cells(slope=1)),
        prepared_case('spread_queued_rejects_before_rebuild', 'spread', 45,
                      pre_bitmap=[[3, 3], [4, 4]]),
        prepared_case('spread_disabled_rejects_before_rebuild', 'spread', 45, spreads=False),
        prepared_case('spread_zero_percentage_rejects_before_rebuild', 'spread', 45,
                      spread_percentages=[0.0] * 4),
        prepared_case('spread_receiver_zero_own_percentage_admits', 'spread', 45,
                      receiver=1, spread_percentages=[0.125, 0.0, 0.125, 0.125]),
        prepared_case('spread_sloped_rejects_before_rebuild', 'spread', 45,
                      cells=changed_cells(slope=1)),
        prepared_case('spread_occupied_rejects_before_rebuild', 'spread', 45,
                      cells=changed_cells(occupied=True)),
        prepared_case('spread_density_zero_rejects_before_rebuild', 'spread', 45,
                      cells=changed_cells(density=0)),
        prepared_case('spread_gem_density_zero_rejects_before_rebuild', 'spread', 45,
                      cells=changed_cells(type_id=1, density=0)),
        prepared_case('spread_gem_density_one_admits', 'spread', 45,
                      cells=changed_cells(type_id=1, density=1)),
        prepared_case('spread_growth_flag_disabled_still_rebuilds', 'spread', 45, grows=False),
    ]
    # Complete Reduce_Tiberium full-removal route: the removed Gem receiver
    # rebuilds its own class, then appends surviving Ore neighbors in native
    # N..NW order. The old receiver bitmap is clear on these neighbors.
    ring = [(4, 3), (5, 3), (5, 4), (5, 5), (4, 5), (3, 5), (3, 4), (3, 3)]
    removed = dict(cell=[4, 4], type_id=1, variant=0, density=0, slope=0, occupied=False)
    neighbors = [dict(cell=list(c), type_id=0, variant=0, density=3, slope=0,
                      occupied=False) for c in ring]
    rows += [prepared_case('reduce_empty_gem_rebuilds_gem_receiver_with_ore_neighbors',
                           'spread', 44, entry='reduce', receiver=1,
                           pre_bitmap=[[4, 4]], cells=[removed, *neighbors]),
             prepared_case('reduce_empty_ore_rebuild_then_skip_reseeded_neighbors',
                           'spread', 44, entry='reduce', receiver=0,
                           pre_bitmap=[[4, 4]], cells=[dict(removed, type_id=0), *neighbors]),
             prepared_case('reduce_density11_growth_guard_precedes_partial_removal',
                           'growth', 55, entry='reduce', amount=1,
                           cells=changed_cells(density=11))]
    for frame in [100, 16777220, -2147483648]:
        rows.append(prepared_case(f'growth_processor_rebuilds_spread_frame_{frame}',
                                   'spread', 44, entry='growth_processor', frame=frame,
                                   pre_cell=[4, 4]))
    # The processor reinserts directly into its own growth array; AddToGrowth's
    # threshold is not a processor-array guard. The same turn can rebuild spread.
    rows.append(prepared_case('growth_processor_inline_reinsert_crosses_growth_counter',
                               'spread', 44, entry='growth_processor', pre_cell=[4, 4],
                               other_pre_counts={'growth': 55}))
    rows.append(prepared_case('growth_processor_negative_budget_drops_first_root',
                               'spread', 44, entry='growth_processor', pre_cell=[4, 4],
                               next_raw=0x80000000))
    return rows


def queue_address(kind, name):
    return ARENA + kind * 0x8000 + (0x4000 if name == 'growth' else 0)


def read_queue(u, read32, kind, name, capacity, bitmap_coordinates):
    counter, _, _, _ = FIELDS[name]
    q = queue_address(kind, name)
    count, heap_count = read32(TIBS + kind * 0x200 + counter), read32(q)
    assert count <= capacity + 1 and heap_count < capacity
    entries = [dict(cell=list(struct.unpack('<hh', u.mem_read(q + 0x100 + i * 8, 4))),
                    priority_bits=read32(q + 0x104 + i * 8)) for i in range(count)]
    heap = []
    for slot in range(1, heap_count + 1):
        delta = read32(q + 0x1000 + slot * 4) - (q + 0x100)
        assert delta >= 0 and delta % 8 == 0 and delta // 8 < count
        heap.append(delta // 8)
    bitmap = [i for i, value in enumerate(u.mem_read(q + 0x2000, capacity)) if value]
    assert all(i in bitmap_coordinates for i in bitmap)
    bitmap_cells = sorted([bitmap_coordinates[i] for i in bitmap])
    return dict(array_count=count, entries=entries, heap_indices=heap, bitmap_indices=bitmap,
                bitmap_cells=bitmap_cells)


def output_state(u, read32, capacity, bitmap_coordinates):
    classes = []
    for kind in range(4):
        tib = TIBS + kind * 0x200
        classes.append(dict(type_id=kind,
                            growth=read_queue(u, read32, kind, 'growth', capacity, bitmap_coordinates),
                            spread=read_queue(u, read32, kind, 'spread', capacity, bitmap_coordinates),
                            growth_timer=list(struct.unpack('<3i', u.mem_read(tib + 0x11C, 12))),
                            spread_timer=list(struct.unpack('<3i', u.mem_read(tib + 0x100, 12)))))
    rng = bytes(u.mem_read(SCENARIO + 0x218, 0x3F4))
    return dict(classes=classes, rng_indices=[read32(SCENARIO + 0x21C), read32(SCENARIO + 0x220)],
                rng_sha256=hashlib.sha256(rng).hexdigest())


def execute(case):
    inherited = dict(case, west_building=False,
                     ore=[[*c['cell'], c['type_id'], c['variant'], c['density']]
                          for c in case['cells']])
    u, call, read32, _events, _unused = fixture(inherited)
    u.mem_map(ARENA, ARENA_SIZE)
    width, height = case['size']
    u.mem_write(MAP + 0xF4, dwords(width, height))
    call(0x42B1F0, 0, [])
    capacity = u.reg_read(UC_X86_REG_EAX)
    assert capacity == (height + 4) * width * 2
    # Reuse the shared real Cell vtables and backing array, supplying the
    # successful Resize diamond. The original iterator stops at its first null.
    u.mem_write(TABLE, bytes(0x100000))
    bitmap_coordinates, bitmap_ordinals = {}, {}
    for y in range(32):
        for x in range(32):
            if width < x + y <= width + 2 * height and abs(x - y) < width:
                u.mem_write(TABLE + (y * 512 + x) * 4, dwords(cell(x, y)))
                call(0x42B1C0, cell(x, y) + 0x24, [])
                ordinal = u.reg_read(UC_X86_REG_EAX)
                assert ordinal < capacity and ordinal not in bitmap_coordinates
                bitmap_coordinates[ordinal] = [x, y]
                bitmap_ordinals[(x, y)] = ordinal
    u.mem_write(SCENARIO + 0x34A6, bytes([case['grows']]))
    for c in case['cells']:
        u.mem_write(cell(*c['cell']) + 0x11C, bytes([c['slope']]))
        u.mem_write(cell(*c['cell']) + 0xE4, dwords(1 if c['occupied'] else 0))
    for kind in range(4):
        tib = TIBS + kind * 0x200
        u.mem_write(tib + 0xA0, struct.pack('<d', case['spread_percentages'][kind]))
        u.mem_write(tib + 0xB0, struct.pack('<d', case['growth_percentages'][kind]))
        u.mem_write(tib + 0xE4, dwords(12))
        u.mem_write(tib + 0x100, struct.pack('<3i', 11 + kind, 0, 200 + kind))
        u.mem_write(tib + 0x11C, struct.pack('<3i', 21 + kind, 0, 300 + kind))
        for name, fields in FIELDS.items():
            q = queue_address(kind, name)
            selected = kind == case['receiver'] and name == case['queue']
            count = case['pre_count'] if selected else case.get('other_pre_counts', {}).get(name, 1)
            if kind != case['receiver']:
                count = 1
            heap = case['pre_heap_indices'] if selected else [0]
            bitmaps = case['pre_bitmap'] if selected else [[2, 3]]
            u.mem_write(tib + fields[0], dwords(count, q, q + 0x2000, q + 0x100))
            u.mem_write(q, dwords(len(heap), capacity, q + 0x1000, 0, 0xFFFFFFFF))
            packed = struct.pack('<hhI', *case['pre_cell'], case['pre_priority_bits'])
            u.mem_write(q + 0x100, packed * count)
            for slot, index in enumerate(heap, 1):
                assert 0 <= index < count
                u.mem_write(q + 0x1000 + slot * 4, dwords(q + 0x100 + index * 8))
            for x, y in bitmaps:
                ordinal = bitmap_ordinals[(x, y)]
                assert 0 <= ordinal < capacity
                u.mem_write(q + 0x2000 + ordinal, b'\x01')
    if 'next_raw' in case:
        # Same supplied native-state boundary as tibtre's probability controls;
        # the original Random::Next XOR/store/indices still execute unchanged.
        first, second = read32(SCENARIO + 0x21C), read32(SCENARIO + 0x220)
        a, b = SCENARIO + 0x224 + first * 4, SCENARIO + 0x224 + second * 4
        u.mem_write(a, dwords(case['next_raw'] ^ read32(b)))
    before = output_state(u, read32, capacity, bitmap_coordinates)
    events, pending_random = [], []

    def observe(_u, address, _size, _data):
        sp, this = u.reg_read(UC_X86_REG_ESP), u.reg_read(UC_X86_REG_ECX)
        if pending_random and address == pending_random[-1][0]:
            _, index = pending_random.pop()
            events[index]['raw'] = u.reg_read(UC_X86_REG_EAX)
            events[index]['rng_after'] = [read32(SCENARIO + 0x21C), read32(SCENARIO + 0x220)]
        if address == RANDOM:
            events.append(dict(kind='random', rng_before=[read32(SCENARIO + 0x21C),
                                                         read32(SCENARIO + 0x220)]))
            pending_random.append((read32(sp), len(events) - 1))
        elif address in (ADD_GROWTH, ADD_SPREAD):
            events.append(dict(kind='enqueue_growth' if address == ADD_GROWTH else 'enqueue_spread',
                               receiver=(this - TIBS) // 0x200,
                               cell=list(struct.unpack('<hh', u.mem_read(read32(sp + 4), 4)))))
        elif address in (REBUILD_GROWTH, REBUILD_SPREAD):
            events.append(dict(kind='rebuild_growth' if address == REBUILD_GROWTH else 'rebuild_spread',
                               receiver=(this - TIBS) // 0x200))
        elif address == GROW_CELL:
            events.append(dict(kind='grow_cell', cell=cell_xy(this)))
        elif address == GROWTH_PROCESSOR:
            events.append(dict(kind='growth_processor', receiver=(this - TIBS) // 0x200))

    u.hook_add(UC_HOOK_CODE, observe)
    if case['entry'] == 'reduce':
        call(REDUCE, cell(*case['target']), [case.get('amount', 1)])
        returned = u.reg_read(UC_X86_REG_EAX)
    elif case['entry'] == 'growth_processor':
        call(GROWTH_PROCESSOR, TIBS + case['receiver'] * 0x200, [])
        returned = None
    else:
        entry = ADD_GROWTH if case['queue'] == 'growth' else ADD_SPREAD
        call(entry, TIBS + case['receiver'] * 0x200, [cell(*case['target']) + 0x24])
        returned = None
    assert not pending_random
    final = output_state(u, read32, capacity, bitmap_coordinates)
    observed_events = list(events)
    next_random = []
    for _ in range(4):
        call(RANDOM, SCENARIO + 0x218, [])
        next_random.append(u.reg_read(UC_X86_REG_EAX))
    return dict(input=case, capacity=capacity, before=before, state=final, events=observed_events,
                bitmap_coordinate_map=[dict(index=index, cell=coord)
                                       for index, coord in sorted(bitmap_coordinates.items())],
                draw_count=sum(e['kind'] == 'random' for e in observed_events),
                next_random=next_random, returned=returned,
                cells=[dict(cell=c['cell'], overlay=struct.unpack('<i', u.mem_read(cell(*c['cell']) + 0x44, 4))[0],
                            density=u.mem_read(cell(*c['cell']) + 0x11E, 1)[0]) for c in case['cells']])


def generate():
    return dict(source='unicorn/gamemd.exe', cases=[execute(case) for case in cases()])


if __name__ == '__main__':
    finish_vectors(generate, Path(__file__).with_suffix('.json'), provenance=lambda: provenance(
        scope='Bounded original AddToGrowth7235A0/AddToSpread722AF0 enqueue-counter rebuilds, '
              'full RebuildGrowth7233A0/RebuildSpread7228B0, gates, reseeding order, duplicate '
              'append, cross-class receivers, timer retention and Scenario RNG continuation; '
              'three full Reduce_Tiberium480A80 caller controls and five complete '
              'GrowthProcessor722F00 calls through GrowTiberium483710/PlaceTiberium487190 '
              'and spread-counter rebuild admission.',
        entry_points={'add_growth': ADD_GROWTH, 'add_spread': ADD_SPREAD,
                      'rebuild_growth': REBUILD_GROWTH, 'rebuild_spread': REBUILD_SPREAD,
                      'can_grow': 0x483620, 'can_spread': 0x483690,
                      'iterator_init': 0x578350, 'iterator_next': 0x578290,
                      'capacity': 0x42B1F0, 'bitmap_ordinal': 0x42B1C0,
                      'reduce_tiberium': REDUCE, 'random': RANDOM,
                      'growth_processor': GROWTH_PROCESSOR, 'grow_tiberium': GROW_CELL,
                      'place_tiberium': 0x487190},
        assumptions=['Shared harvest_field/refinery_dock fixture: original Cell/Unit vtables, '
                     'original Scenario seeder, declared successful Size4x4 diamond over '
                     'the existing 32x32 backing table; CellIterator executes until first null.',
                     'Four synthetic TiberiumClass instances retain the shared fixture overlay '
                     'registry (Riparius102..121, Cruentus27..38); MaxDensity12 and per-case '
                     'percentage/Scenario gates, cell density/slope/occupancy are supplied data.',
                     'Adequate external entries/heap/bitmap stores, explicit append count and '
                     'valid heap references represent prior queue state; initialization/allocator '
                     'failure, whole Logic/map startup and actual long-session history are excluded.',
                     'Timer+4 padding is supplied zero; sentinel frame/duration values are compared '
                     'for every class. X87 control0x0E7F comes from the shared fixture; signed frame '
                     'wrap and >2^24 chopped FILD/FSTP boundaries execute. Four next_raw controls '
                     'replace one seeded RNG data word before original Random::Next, including '
                     'INT_MIN signed-absolute overflow; no draw return is hooked.'],
        substitutions=['Shared fixture OS Interlocked imports and unused refinery presentation '
                       'sinks remain. Reduce_Tiberium observers supply empty tactical rectangles, '
                       'RecalcAttributes bare-land publication and radar/tactical dirtiness sinks. '
                       'No ore admission, queue, rebuild, iterator or RNG return is substituted.']),
        source_paths={'producer': Path(__file__), 'harvest_field': Path(__file__).with_name('harvest_field.py'),
                      'refinery_dock': Path(__file__).with_name('refinery_dock.py')})
