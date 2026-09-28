"""Original HouseClass::SetDifficulty 0x004F6EC0: the ROF bias and team timer it
stores.

Executes the whole body per case and records House+0x184 (the difficulty index),
House+0x1A8 (the ROF bias, a double) and the team timer's start and duration
(+0x5798, +0x57A0: the frame, and TeamDelays[difficulty] plus the house's
+0x30 index times 175, 0x004F70F0..0x004F712D) afterwards. Outside a campaign
(GameMode [0x00A8B238] nonzero) the bias is the difficulty row's ROF times the
HouseType's ROF (FLD; FMUL; FSTP qword, 0x004F6F6C..0x004F6F79); in a campaign
it is the row's value (0x004F7072..0x004F707B).

Supplied: the Rules object (the three difficulty rows at +0x1538, stride 0x50,
ROF at row +0x20; the other row fields and the Rules scalars SetDifficulty reads
are fixed; TeamDelays at +0x115C holds 11, 22, 33), the HouseType
(+0xC8..+0xF8 multipliers, ROF at +0xE8), the house's +0x30 index and +0x34
HouseType, the frame counter (100) and GameMode. Nothing is stubbed.

Rust consumer: src/sim/house_state.rs (HouseState::set_difficulty).
"""
import struct
from pathlib import Path

from tools.native_oracle import SCRATCH, call, finish_vectors, provenance

SET_DIFFICULTY = 0x4F6EC0
HOUSE, HOUSE_TYPE, RULES, TABLE = SCRATCH, SCRATCH + 0x6000, SCRATCH + 0x7000, SCRATCH + 0xF000
ONE = 0x3FF0000000000000


def f32d(value):
    """ReadDouble's value: the %f single widened to a double."""
    return struct.unpack('<Q', struct.pack('<d', struct.unpack('<f', struct.pack('<f', value))[0]))[0]


def qword(bits):
    return struct.pack('<Q', bits)


def dword(value):
    return struct.pack('<I', value & 0xFFFFFFFF)


# Retail rows ([Easy], [Normal], [Difficult]) from rulesmd.ini, in the row
# layout ReadDifficulty 0x0066D270 stores: FirePower, Groundspeed, Airspeed,
# Armor, ROF, Cost, BuildTime, RepairDelay, BuildDelay.
RETAIL_ROWS = [
    [ONE, f32d(1.0), f32d(1.0), f32d(1.2), f32d(.8), f32d(1.0), f32d(.8), f32d(.02), f32d(.03)],
    [ONE, f32d(1.0), f32d(1.0), f32d(1.0), f32d(1.0), f32d(1.0), f32d(1), f32d(.02), f32d(.03)],
    [ONE, f32d(1.0), f32d(1.0), f32d(.8), f32d(1.2), f32d(1.0), f32d(1.0), f32d(.05), f32d(.1)],
]


def run(case):
    rows = [list(row) for row in RETAIL_ROWS]
    for index, bits in enumerate(case['row_rof']):
        rows[index][4] = int(bits, 16)
    writes = {0x8871E0: dword(RULES), 0xA8B238: dword(case['mode']), 0xA8ED84: dword(100),
              HOUSE + 0x30: dword(case['array_index']), HOUSE + 0x34: dword(HOUSE_TYPE),
              HOUSE + 0x184: dword(7),
              RULES + 0x1418: qword(ONE), RULES + 0x115C: dword(TABLE),
              TABLE: dword(11) + dword(22) + dword(33)}
    for index in range(7):
        writes[HOUSE_TYPE + 0xC8 + 8 * index] = qword(ONE)
    writes[HOUSE_TYPE + 0xE8] = qword(int(case['country_rof'], 16))
    for index, row in enumerate(rows):
        writes[RULES + 0x1538 + index * 0x50] = b''.join(qword(value) for value in row)
    result = call(SET_DIFFICULTY, ecx=HOUSE, stack_args=[case['difficulty']], writes=writes,
                  dumps={'house': (HOUSE + 0x184, 0x2C), 'team_timer': (HOUSE + 0x5798, 0xC)})
    house = bytes.fromhex(result['dumps']['house'])
    team_timer = bytes.fromhex(result['dumps']['team_timer'])
    return dict(input=case,
                difficulty=struct.unpack_from('<i', house, 0)[0],
                rof_bias=f"{struct.unpack_from('<Q', house, 0x1A8 - 0x184)[0]:016x}",
                team_timer=[struct.unpack_from('<i', team_timer, 0)[0],
                            struct.unpack_from('<i', team_timer, 8)[0]])


# House array indexes, cycled over the cases; the last wraps the 175 product.
ARRAY_INDEXES = (0, 1, 7, 0x01000001)


def cases():
    for number, case in enumerate(rof_cases()):
        yield dict(case, array_index=ARRAY_INDEXES[number % len(ARRAY_INDEXES)])


def rof_cases():
    retail = [f'{f32d(.8):016x}', f'{ONE:016x}', f'{f32d(1.2):016x}']
    odd = [f'{f32d(.7):016x}', f'{f32d(1.3):016x}', '0000000000000000',
           f'{f32d(-.5):016x}', '3fefffffffffffff']
    for mode in (5, 0):
        for country_rof in (f'{ONE:016x}', f'{f32d(1.1):016x}', '3fefffffffffffff',
                            f'{f32d(.9):016x}'):
            for difficulty in (0, 1, 2):
                yield dict(mode=mode, country_rof=country_rof, difficulty=difficulty,
                           row_rof=retail)
                yield dict(mode=mode, country_rof=country_rof, difficulty=difficulty,
                           row_rof=odd[:3])
                yield dict(mode=mode, country_rof=country_rof, difficulty=difficulty,
                           row_rof=odd[2:])


def generate():
    return [run(case) for case in cases()]


if __name__ == '__main__':
    finish_vectors(generate, Path(__file__).with_suffix('.json'), provenance=lambda: provenance(
        scope=('SetDifficulty over difficulty 0..2, GameMode nonzero and zero, HouseType ROF '
               '(1.0, 1.1f, 0.9f, the largest double below 1), difficulty-row ROF values '
               '(the retail rows, and odd values including zero and negative) and house '
               'indexes 0, 1, 7 and 0x01000001: the stored difficulty index, ROF bias and '
               'team timer start and duration.'),
        assumptions=['x87 control word 0x0E7F (PC53, chop), the harness default.',
                     "Row values are ReadDouble's widened singles, as rulesmd.ini yields.",
                     'The other row fields and Rules+0x1418/+0x115C are fixed fixture values; '
                     'only +0x184, +0x1A8, +0x5798 and +0x57A0 are recorded.'],
        substitutions=[],
        entry_points={'set_difficulty': SET_DIFFICULTY},
    ))
