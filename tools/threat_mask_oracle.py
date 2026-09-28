"""Native references for Greatest_Threat's mask pieces.

Run python -m tools.threat_mask_oracle --check (or explicit --write).
Rust consumer: src/sim/combat/greatest_threat_mask_oracle_tests.rs.

Sections, each executed in a fresh emulator per case:
- flags: Greatest_Threat 0x6F8DF0's flags word 0x6F8F29..0x6F8F76 from the
  effective mask.
- quarry_terms: Evaluate_Candidate 0x6F7CA0 after its VHP transform,
  0x6F875F..0x6F88BF (All-To-Hunt, the quarry's building terms and rejects),
  then its final acceptance 0x6F8928..0x6F8948.
- enemy_bonus: Calculate_Threat_Score 0x70CD10's SpecialThreatValue term and
  EnemyHouseThreatBonus, 0x70CEDC..0x70CF1D.

Control flow the rows do not vary (the ThreatAvoidance factor between the two
quarry slices, dormant without a Supress= weapon) rests on instruction
reading; see src/sim/combat/greatest_threat.rs.
"""
from pathlib import Path
import random
import struct

from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX,
                               UC_X86_REG_ECX, UC_X86_REG_EDI, UC_X86_REG_ESI,
                               UC_X86_REG_ESP, UC_X86_REG_FPCW)

from tools.ai_base_building_oracle import FAKE, RULES, STUBS, Emu, u32
from tools.native_oracle import (NATIVE_FPCW, STACK_BASE, STACK_SIZE, finish_vectors,
                                 provenance, run_checked)

SP = STACK_BASE + STACK_SIZE - 0x1000
SCANNER = FAKE + 0x400000
CANDIDATE = FAKE + 0x401000
CANDIDATE_VTABLE = FAKE + 0x402000
TYPE = FAKE + 0x404000
HOUSES = FAKE + 0x410000
HOUSE_SIZE = 0x6000
ARRAY = FAKE + 0x430000
WEAPON = FAKE + 0x431000
HOUSE_ARRAY = 0xA8022C
STUB_WHAT_AM_I = STUBS + 0x200
STUB_OCCUPANTS = STUBS + 0x210
STUB_WEAPON = STUBS + 0x220
BUILDING, UNIT = 6, 0x28


def i32(value):
    return struct.unpack('<i', u32(value))[0]


def bits(value):
    return struct.pack('>d', value).hex()


class MaskEmu(Emu):
    def __init__(self):
        super().__init__()
        self.uc.reg_write(UC_X86_REG_FPCW, NATIVE_FPCW)
        self.uc.reg_write(UC_X86_REG_ESP, SP)

    def write8(self, address, value):
        self.uc.mem_write(address, bytes([value & 0xFF]))

    def write_double(self, address, value):
        self.uc.mem_write(address, struct.pack('<d', value))

    def read_double_bits(self, address):
        return bytes(self.uc.mem_read(address, 8))[::-1].hex()


# ---------------------------------------------------------------- flags

FLAGS, FLAGS_END = 0x6F8F29, 0x6F8F76


def flags():
    rows = []
    masks = [0, 1, 2, 3, 4, 5, 8, 0x10, 0x20, 0x40, 0x80, 0x100, 0x200, 0x800, 0x1000,
             0x2000, 0x4000, 0x8000, 0x10000, 0x4008, 0x4010, 0x4009, 0xB8, 0xB9, 0xBC,
             0x1BA60, 0x18200, 0xFFFFFFFF]
    rng = random.Random(0x6F8F29)
    masks += [rng.getrandbits(18) for _ in range(40)]
    for mask in masks:
        emu = MaskEmu()
        emu.uc.reg_write(UC_X86_REG_EBX, mask)
        emu.uc.reg_write(UC_X86_REG_EDI, 0)
        emu.write32(SP + 0x70, mask)
        run_checked(emu.uc, FLAGS, FLAGS_END, count=40)
        rows.append(dict(mask=mask, flags=emu.read32(SP + 0x14)))
    return rows


# ---------------------------------------------------------------- quarry_terms

TERMS, TERMS_CONTINUE = 0x6F875F, 0x6F88BF
TERMS_REJECTS = (0x6F894F, 0x6F888F)
FINISH, FINISH_ACCEPT, FINISH_REJECT = 0x6F8928, 0x6F8939, 0x6F8948


def quarry_terms_row(*, mask, score, building, power, max_occupants, needs_engineer,
                     factory, can_be_occupied, occupants, armed, bias, enemy, enemy_owns):
    """`enemy`: whether the scanner's house has a current enemy (index 1)."""
    emu = MaskEmu()
    uc = emu.uc
    scanner_house, enemy_house, other_house = (HOUSES + i * HOUSE_SIZE for i in range(3))
    emu.write32(HOUSE_ARRAY, ARRAY)
    emu.write32(ARRAY + 4, enemy_house)
    emu.write32(SCANNER + 0x21C, scanner_house)
    emu.write8(scanner_house + 0x249, int(bias))
    emu.write32(scanner_house + 0x5600, 1 if enemy else -1)
    emu.write32(CANDIDATE, CANDIDATE_VTABLE)
    emu.write32(CANDIDATE + 0x21C, enemy_house if enemy_owns else other_house)
    emu.write32(CANDIDATE + 0x520, TYPE)
    for slot, stub in ((0x2C, STUB_WHAT_AM_I), (0x408, STUB_OCCUPANTS), (0x3F4, STUB_WEAPON)):
        emu.write32(CANDIDATE_VTABLE + slot, stub)
    what_am_i = BUILDING if building else UNIT
    emu.hook(STUB_WHAT_AM_I, lambda _e: what_am_i, 0)
    emu.hook(STUB_OCCUPANTS, lambda _e: occupants, 0)
    emu.write32(WEAPON, 0x12345678 if armed else 0)
    emu.hook(STUB_WEAPON, lambda _e: WEAPON, 0)
    emu.write32(TYPE + 0xEE0, power)
    emu.write32(TYPE + 0x1580, max_occupants)
    emu.write8(TYPE + 0x1552, int(needs_engineer))
    emu.write32(TYPE + 0xEB8, 3 if factory else 0)
    emu.write8(TYPE + 0x157B, int(can_be_occupied))
    score_address = SP + 0x200
    emu.write32(score_address, score)
    emu.write32(SP + 0x40, mask)
    emu.write32(SP + 0x4C, what_am_i)
    emu.write32(SP + 0x54, TYPE)
    uc.reg_write(UC_X86_REG_EDI, SCANNER)
    uc.reg_write(UC_X86_REG_ESI, CANDIDATE)
    uc.reg_write(UC_X86_REG_EBP, score_address)
    result = None
    if run_checked(uc, TERMS, (TERMS_CONTINUE,) + TERMS_REJECTS, count=200) == TERMS_CONTINUE:
        uc.reg_write(UC_X86_REG_ESP, SP)
        if run_checked(uc, FINISH, (FINISH_ACCEPT, FINISH_REJECT), count=20) == FINISH_ACCEPT:
            result = i32(uc.reg_read(UC_X86_REG_EAX))
    return dict(mask=mask, score=score, building=building, power=power,
                max_occupants=max_occupants, needs_engineer=needs_engineer, factory=factory,
                can_be_occupied=can_be_occupied, occupants=occupants, armed=armed, bias=bias,
                enemy=enemy, enemy_owns=enemy_owns, result=result)


def quarry_terms():
    rng = random.Random(0x6F875F)
    masks = (0, 0x800, 0x8000, 0x10000, 0x1000, 0x2000, 0x1800, 0x9000, 0x3000, 0x18800)
    scores = (-7, 0, 1, 2, 999, 100000, 0x7FFFFFFF, -0x80000000)
    rows = []
    for number in range(400):
        rows.append(quarry_terms_row(
            mask=masks[number % len(masks)], score=rng.choice(scores),
            building=rng.random() < 0.8, power=rng.choice((-100, 0, 1, 50, 2200000)),
            max_occupants=rng.choice((-1, 0, 5, 3000000)), needs_engineer=rng.random() < 0.5,
            factory=rng.random() < 0.5, can_be_occupied=rng.random() < 0.5,
            occupants=rng.choice((0, 3)), armed=rng.random() < 0.5, bias=rng.random() < 0.3,
            enemy=rng.random() < 0.7, enemy_owns=rng.random() < 0.5))
    return rows


# ---------------------------------------------------------------- enemy_bonus

BONUS, BONUS_END = 0x70CEDC, 0x70CF1D


def enemy_bonus_row(*, score, coefficient, special, bonus, enemy, enemy_owns):
    emu = MaskEmu()
    uc = emu.uc
    scanner_house, enemy_house, other_house = (HOUSES + i * HOUSE_SIZE for i in range(3))
    emu.write32(SCANNER + 0x21C, scanner_house)
    emu.write32(scanner_house + 0x5600, 1 if enemy else -1)
    emu.write32(enemy_house + 0x30, 1)
    emu.write32(other_house + 0x30, 2)
    emu.write32(CANDIDATE + 0x21C, enemy_house if enemy_owns else other_house)
    emu.write_double(TYPE + 0x2C0, special)
    emu.write_double(RULES + 0x1090, bonus)
    emu.write_double(SP + 0x10, score)
    emu.write_double(SP + 0x20, coefficient)
    uc.reg_write(UC_X86_REG_EAX, TYPE)
    uc.reg_write(UC_X86_REG_EDI, SCANNER)
    uc.reg_write(UC_X86_REG_ESI, CANDIDATE)
    run_checked(uc, BONUS, BONUS_END, count=20)
    return dict(score_bits=bits(score), coefficient_bits=bits(coefficient),
                special_bits=bits(special), bonus_bits=bits(bonus), enemy=enemy,
                enemy_owns=enemy_owns, result_bits=emu.read_double_bits(SP + 0x10))


def enemy_bonus():
    rng = random.Random(0x70CEE6)
    rows = []
    for _ in range(60):
        rows.append(enemy_bonus_row(
            score=rng.choice((0.0, -3.5, 12345.678, 1e300, rng.uniform(-1e6, 1e6))),
            coefficient=rng.choice((0.0, 1.0, 0.1, rng.uniform(-10, 10))),
            special=rng.choice((0.0, 1.0, 5.0, rng.uniform(0, 100))),
            bonus=rng.choice((0.0, 400.0, 0.1, -50.0, 1e308)),
            enemy=rng.random() < 0.8, enemy_owns=rng.random() < 0.6))
    return rows


def generate():
    return dict(flags=flags(), quarry_terms=quarry_terms(), enemy_bonus=enemy_bonus())


if __name__ == '__main__':
    finish_vectors(generate, Path(__file__).with_suffix('.json'), provenance=lambda: provenance(
        scope=('Greatest_Threat\'s flags word over masks; Evaluate_Candidate\'s All-To-Hunt '
               'and quarry terms over synthetic candidates, then its final acceptance; '
               'Calculate_Threat_Score\'s SpecialThreatValue term and EnemyHouseThreatBonus.'),
        assumptions=['x87 control word 0x0E7F (PC53, chop), the harness default.',
                     'Fixture objects carry only the fields the slices read.'],
        substitutions=['The candidate\'s WhatAmI (vt+0x2C), occupant count (vt+0x408) and '
                       'current weapon (vt+0x3F4) answer from the case.',
                       'Between the quarry slices, the ThreatAvoidance factor '
                       '(0x6F88BF..0x6F8926) is skipped: 1.0, no score change.'],
        entry_points={'flags': FLAGS, 'quarry_terms': TERMS, 'finish': FINISH,
                      'enemy_bonus': BONUS},
    ))
