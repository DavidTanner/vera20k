"""Original ParasiteClass CanInfect water gate for a Naval owner.

CanInfect @ 0x0062A8E0 runs whole, with the victim and owner as UnitClass
objects on the original vtable (0x007F5C70): GetTechnoType (vt+0x84 ->
0x006F3270 -> 0x00741490) reads +0x6C4, and GetCell (vt+0x1BC -> 0x005F6960)
looks the victim's Location up through MapClass @ 0x00565730, which answers
the shared dummy (0x00ABDC50) for an index past the cell count or a NULL slot.
The WaterSet test 0x00485060 reads the cell's tile (+0x38) against the theater
base at 0x00AA0738. Every other CanInfect gate is supplied passing.
"""
from pathlib import Path
import struct

from tools.native_oracle import SCRATCH, call, finish_vectors, provenance

CAN_INFECT = 0x0062A8E0
UNIT_VTABLE = 0x007F5C70
MAP = 0x0087F7E8
WATER_SET = 0x00AA0738
DUMMY = 0x00ABDC50

PARASITE, OWNER, OWNER_TYPE, VICTIM, VICTIM_TYPE, CELL, CELLS = (
    SCRATCH + offset for offset in (0x0000, 0x0800, 0x1000, 0x2000, 0x3000, 0x4000, 0x5000))
# The real cell sits at (5, 2), slot 2 * 512 + 5; slot (4, 2) is NULL and the
# map holds slots 0..1029, so (6, 2) is past the count.
REAL_CELL, HOLE_CELL, PAST_CELL = (5, 2), (4, 2), (6, 2)
CELL_COUNT = 2 * 512 + 6


def dword(value):
    return struct.pack('<I', value & 0xFFFFFFFF)


def run(name, *, water_set, tile, owner, naval, victim_cell):
    """One CanInfect call; returns the row with its AL result."""
    x, y = victim_cell
    writes = {
        WATER_SET: dword(water_set),
        # Cell constructor 0x0047BC11 gives the shared dummy tile 0xFFFF.
        DUMMY + 0x38: dword(0xFFFF),
        MAP + 0x13C: dword(CELLS),
        MAP + 0x140: dword(CELL_COUNT),
        CELLS + 4 * (REAL_CELL[1] * 512 + REAL_CELL[0]): dword(CELL),
        CELL + 0x38: dword(tile),
        PARASITE + 0x24: dword(OWNER if owner else 0),
        OWNER: dword(UNIT_VTABLE),
        OWNER + 0x6C4: dword(OWNER_TYPE),
        OWNER_TYPE + 0xCCE: bytes([1 if naval else 0]),
        VICTIM: dword(UNIT_VTABLE),
        VICTIM + 0x6C: dword(100),  # Health
        VICTIM + 0x90: b'\x01',  # IsAlive
        VICTIM + 0x6C4: dword(VICTIM_TYPE),
        VICTIM + 0x9C: struct.pack('<3i', x * 256 + 128, y * 256 + 128, 0),
        VICTIM_TYPE + 0xD38: b'\x01',  # Parasiteable
    }
    result = call(CAN_INFECT, ecx=PARASITE, stack_args=[VICTIM], writes=writes)
    return dict(name=name, water_set=water_set, tile=tile, owner=owner, naval=naval,
                victim_cell=list(victim_cell), admits=bool(result['eax'] & 0xFF))


def generate():
    rows = []
    for water_set in (314, -1):
        for offset in (-1, 0, 13, 14):
            tile = water_set + offset
            rows.append(run(f'naval_tile_{offset:+d}_base_{water_set}', water_set=water_set,
                            tile=tile, owner=True, naval=True, victim_cell=REAL_CELL))
        rows.append(run(f'land_owner_tile_-1_base_{water_set}', water_set=water_set,
                        tile=water_set - 1, owner=True, naval=False, victim_cell=REAL_CELL))
    rows.append(run('no_owner_tile_-1', water_set=314, tile=313, owner=False, naval=False,
                    victim_cell=REAL_CELL))
    for name, cell in (('naval_null_slot_dummy', HOLE_CELL), ('naval_past_count_dummy', PAST_CELL)):
        rows.append(run(name, water_set=314, tile=314, owner=True, naval=True, victim_cell=cell))
    rows.append(run('naval_null_slot_dummy_lunar', water_set=-1, tile=0, owner=True, naval=True,
                    victim_cell=HOLE_CELL))
    return dict(rows=rows)


def metadata():
    return provenance(
        scope=__doc__,
        assumptions=[
            'Victim and owner are UnitClass objects on the original vtable; the parasite '
            'object supplies only its Owner (+0x24).',
            'Victim gates other than the water test are supplied passing: not in limbo '
            '(+0x81 = 0), alive (+0x90 = 1), Health (+0x6C) = 100, no parasite (+0x694 = 0), '
            'Parasiteable type (+0xD38 = 1), no bunker link (+0x2E4 = 0).',
            'The shared dummy cell holds the tile 0xFFFF its constructor (0x0047BC11) stores.',
        ],
        substitutions=['None: CanInfect, the vtable getters, GetCell, the map lookup and the '
                       'WaterSet test all run original code.'],
        entry_points={'can_infect': CAN_INFECT, 'get_cell': 0x005F6960, 'map_cell': 0x00565730,
                      'water_set_test': 0x00485060})


if __name__ == '__main__':
    finish_vectors(generate, Path(__file__).with_suffix('.json'), provenance=metadata)
