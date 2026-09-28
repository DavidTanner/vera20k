"""Native references for a computer team's recruitment, centre and scripts.

Run python -m tools.team_recruit_oracle --check (or explicit --write).
Rust consumer: src/sim/team_script_vm/recruit_oracle_tests.rs.

Sections, each executed in a fresh emulator per case:
- recalc: TeamClass::Recalc 0x6EA3E0 with the TaskForce total 0x6E8160 (the
  full and under-strength bytes, a Reinforce= TeamType's third, the empty
  team's destruction).
- center: TeamClass::Calc_Center 0x6EAEE0 off action 10 (the member tests,
  the GuardSlower= double count, the mean, the closest member by 0x5F6500
  and the head fallback).
- recruit: TeamClass::Recruit 0x6EAA90 over the Infantry, Unit and Aircraft
  arrays (the group filter and penalty, the 0x5F6560 key, the Unit search's
  house and type test).
- distance: ObjectClass::Distance 0x5F6360 (Sqrt_Approx 0x4CAC40, ftol
  0x7C5F00, a building's foundation deduction over every foundation).
- guard: TeamClass::AI's action 5 timer 0x6E97CE..0x6E97EC.
- regroup: action 54 0x6EFA10 up to its FNPC call 0x6EFC3A (the enemy's
  base centre and the atan2 facing 0x4CAE30, or the RandomRanged(0, 255)
  facing; the sin/cos step 0x4CACB0/0x4CAD00 and the cell quotients).
- own_building: FindOwnBuilding 0x6EEEA0.
- gather: action 53 0x6EF700 with `first` up to its FNPC call 0x6EF98A, or
  to its return when it finishes at once (the enemy's and own base centres,
  the leader's location without a base, the same atan2/sin/cos step as
  action 54 from the enemy's centre).
- quarry: Quarry_To_Threat 0x645BB0 and its jump table 0x645BF8.
- attack_cadence: Coordinate_Attack's signed Frame % 8 == 4 test
  0x6EB59A..0x6EB5AE.

Control flow the rows do not vary (Calc_Center's action-10 branch, the
TeamType waypoint origin, the recruited unit's passengers, action 54's leader
loop) rests on instruction reading; see src/sim/team_script_vm/.
"""
from pathlib import Path
import random
import struct

from unicorn.x86_const import (UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_ESI,
                               UC_X86_REG_ESP, UC_X86_REG_FPCW)

from tools.ai_base_building_oracle import FAKE, FRAME, RULES, STUBS, Emu, u32
from tools.native_oracle import (NATIVE_FPCW, RET_MAGIC, STACK_BASE, STACK_SIZE,
                                 finish_vectors, provenance, run_checked)

TEAM = FAKE + 0x100000
TEAM_VTABLE = FAKE + 0x100800
TEAM_TYPE = FAKE + 0x101000
TASK_FORCE = FAKE + 0x102000
SCRIPT = FAKE + 0x103000
HOUSES = FAKE + 0x104000
HOUSE_SIZE = 0x6000
TYPES = FAKE + 0x120000
TYPE_SIZE = 0x2000
TYPE_VTABLE = FAKE + 0x160000
OBJECTS = FAKE + 0x200000
OBJECT_SIZE = 0x800
OBJECT_VTABLE = FAKE + 0x300000
BUILDING_VTABLE = FAKE + 0x300800
BUILDING_TYPES = FAKE + 0x310000
ARRAYS = FAKE + 0x340000
CELL = FAKE + 0x350000

STUB_RTTI = STUBS + 0x100
STUB_TECHNO_TYPE = STUBS + 0x110
STUB_WEIGHT = STUBS + 0x120
STUB_CAN_ENTER = STUBS + 0x130
STUB_SET_TARGET = STUBS + 0x140
STUB_DELETE = STUBS + 0x150
STUB_BUILDING_COORDS = STUBS + 0x160

GET_COORDS = 0x5F65A0
SCENARIO_INIT = 0xA8E7AC
HOUSE_ARRAY = 0xA8022C
INFANTRY = (0xA83DEC, 0xA83DF8)
UNITS = (0x8B410C, 0x8B4118)
AIRCRAFT = (0xA8E394, 0xA8E3A0)

RTTI_UNIT, RTTI_AIRCRAFT, RTTI_BUILDING, RTTI_INFANTRY = 1, 2, 6, 15
TYPE_RTTI = {'infantry': 0x10, 'unit': 0x28, 'aircraft': 0x03}
CLASS_ARRAY = {'infantry': INFANTRY, 'unit': UNITS, 'aircraft': AIRCRAFT}


def i32(value):
    return struct.unpack('<i', u32(value))[0]


class TeamEmu(Emu):
    """A fresh image with one team, its TeamType and TaskForce, and objects
    whose GetCoords (vt+0x48) is the original 0x5F65A0."""

    def __init__(self):
        super().__init__()
        self.rtti = {}
        self.techno_type = {}
        self.weight = {}
        self.can_enter = {}
        for slot, stub in ((0x2C, STUB_RTTI), (0x84, STUB_TECHNO_TYPE), (0x4E0, STUB_WEIGHT),
                           (0x1AC, STUB_CAN_ENTER), (0x3C8, STUB_SET_TARGET)):
            self.write32(OBJECT_VTABLE + slot, stub)
            self.write32(BUILDING_VTABLE + slot, stub)
        self.write32(OBJECT_VTABLE + 0x48, GET_COORDS)
        self.write32(BUILDING_VTABLE + 0x48, STUB_BUILDING_COORDS)
        self.write32(TYPE_VTABLE + 0x2C, STUB_RTTI)
        self.write32(TEAM_VTABLE + 0x20, STUB_DELETE)
        self.write32(TEAM, TEAM_VTABLE)
        self.write32(TEAM + 0x24, TEAM_TYPE)
        self.write32(TEAM + 0x28, SCRIPT)
        self.write32(TEAM_TYPE + 0xE4, TASK_FORCE)
        self.hook(STUB_RTTI, lambda e: e.rtti[e.uc.reg_read(UC_X86_REG_ECX)], 0)
        self.hook(STUB_TECHNO_TYPE, lambda e: e.techno_type[e.uc.reg_read(UC_X86_REG_ECX)], 0)
        self.hook(STUB_WEIGHT, lambda e: e.weight[e.uc.reg_read(UC_X86_REG_ECX)], 0)
        self.hook(STUB_CAN_ENTER, self.can_enter_stub, 20)
        self.hook(STUB_SET_TARGET, lambda e: e.events.append(
            ['set_target', self.index_of(e.uc.reg_read(UC_X86_REG_ECX)), e.arg(0)]), 4)
        self.hook(STUB_DELETE, lambda e: e.events.append(['delete', e.arg(0)]), 4)

    def can_enter_stub(self, e):
        member = e.uc.reg_read(UC_X86_REG_ECX)
        e.events.append(['can_enter', self.index_of(member), e.arg(0) == CELL,
                         i32(e.arg(1)), i32(e.arg(2)), e.arg(3), e.arg(4)])
        return e.can_enter[member]

    @staticmethod
    def index_of(address):
        return (address - OBJECTS) // OBJECT_SIZE

    def write8(self, address, value):
        self.uc.mem_write(address, bytes([value & 0xFF]))

    def read8(self, address):
        return self.uc.mem_read(address, 1)[0]

    def object(self, index, *, xyz=(0, 0, 0), vtable=OBJECT_VTABLE, rtti=RTTI_INFANTRY):
        address = OBJECTS + index * OBJECT_SIZE
        self.write32(address, vtable)
        for offset, value in zip((0x9C, 0xA0, 0xA4), xyz):
            self.write32(address + offset, value)
        self.rtti[address] = rtti
        return address

    def techno_type_at(self, index, *, rtti=None, passengers=0, naval=False):
        address = TYPES + index * TYPE_SIZE
        self.write32(address, TYPE_VTABLE)
        self.write32(address + 0x5E0, passengers)
        self.write8(address + 0xCCE, int(naval))
        if rtti is not None:
            self.rtti[address] = rtti
        return address

    def start(self, entry, end, *, ecx, args=()):
        """Run `entry` (thiscall) from a fresh stack to `end` (one address or
        several); answer the address reached."""
        uc = self.uc
        sp = STACK_BASE + STACK_SIZE - 0x1000
        for value in reversed(args):
            sp -= 4
            uc.mem_write(sp, u32(value))
        sp -= 4
        uc.mem_write(sp, u32(RET_MAGIC))
        uc.reg_write(UC_X86_REG_ESP, sp)
        uc.reg_write(UC_X86_REG_FPCW, NATIVE_FPCW)
        uc.reg_write(UC_X86_REG_ECX, ecx)
        return run_checked(uc, entry, end)


# ---------------------------------------------------------------- recalc

RECALC = 0x6EA3E0
TEAM_BYTES = (0x76, 0x78, 0x79, 0x7A, 0x7B, 0x7D, 0x7E)


def recalc_row(*, entries, total, reinforce, guard_slower, has_been, under):
    emu = TeamEmu()
    emu.write32(TASK_FORCE + 0x9C, len(entries))
    for slot, amount in enumerate(entries):
        emu.write32(TASK_FORCE + 0xA4 + slot * 8, amount)
    emu.write8(TEAM_TYPE + 0xAB, int(reinforce))
    emu.write8(TEAM_TYPE + 0xA7, int(guard_slower))
    emu.write32(TEAM + 0x48, total)
    emu.write32(TEAM + 0x34, CELL)
    for offset, value in ((0x76, 0), (0x78, has_been), (0x79, 0), (0x7A, under), (0x7B, 0),
                          (0x7D, 1), (0x7E, 1), (0x82, 0)):
        emu.write8(TEAM + offset, int(value))
    result = emu.invoke(RECALC, ecx=TEAM) & 0xFF
    return dict(entries=entries, total=total, reinforce=reinforce, guard_slower=guard_slower,
                has_been=has_been, under=under, result=result,
                bytes={f'{offset:#x}': emu.read8(TEAM + offset) for offset in TEAM_BYTES},
                zone_cleared=emu.read32(TEAM + 0x34) == 0,
                deleted=any(event[0] == 'delete' for event in emu.events))


def recalc():
    rows = []
    for entries in ([], [0], [1], [2], [3], [2, 1], [1, 1, 1], [4, 2], [7], [9], [5, 4, 3],
                    [-1, 3], [-9]):
        desired = sum(entries)
        totals = sorted({0, 1, 2, 3, desired - 1, desired, desired + 1} & set(range(0, 13)))
        for total in totals:
            for reinforce in (False, True):
                for has_been in (False, True):
                    for under in (False, True):
                        rows.append(recalc_row(entries=entries, total=total,
                                               reinforce=reinforce,
                                               guard_slower=(total + len(entries)) % 2 == 1,
                                               has_been=has_been, under=under))
    return rows


# ---------------------------------------------------------------- center

CALC_CENTER = 0x6EAEE0
CURRENT_ACTION = 0x691500
GET_CELL_AT = 0x565730


def center_row(*, members, focus=None, guard_slower=False, action=0):
    """`members`: dicts of location, flags, the weight and cell answers."""
    emu = TeamEmu()
    emu.write8(TEAM_TYPE + 0xA7, int(guard_slower))

    def current(e):
        out = e.arg(0)
        e.write32(out, action)
        e.write32(out + 4, 0)
        return out
    emu.hook(CURRENT_ACTION, current, 4)

    def cell_at(e):
        coord = e.arg(0)
        e.events.append(['cell_at', [i32(e.read32(coord + offset)) for offset in (0, 4, 8)]])
        return CELL
    emu.hook(GET_CELL_AT, cell_at, 4)
    previous = None
    for index, member in enumerate(members):
        address = emu.object(index, xyz=member['xyz'],
                             rtti=RTTI_AIRCRAFT if member['aircraft'] else RTTI_INFANTRY)
        emu.write8(address + 0x90, int(member['active']))
        emu.write32(address + 0x6C, member['health'])
        emu.write8(address + 0x81, int(member['limbo']))
        emu.write8(address + 0x689, int(member['initiated']))
        emu.write8(address + 0x3D5, int(member['in_playfield']))
        emu.techno_type[address] = emu.techno_type_at(index, passengers=member['passengers'],
                                                      naval=member['naval'])
        emu.weight[address] = int(member['weight'])
        emu.can_enter[address] = member['can_enter']
        if previous is None:
            emu.write32(TEAM + 0x54, address)
        else:
            emu.write32(previous + 0x5D8, address)
        previous = address
    if focus is not None:
        emu.write32(TEAM + 0x40, emu.object(len(members), xyz=focus))
    outputs = FAKE + 0x360000
    emu.write32(outputs, 0x1234)
    emu.write32(outputs + 4, 0x1234)
    emu.invoke(CALC_CENTER, ecx=TEAM, args=(outputs, outputs + 4))
    center, closest = emu.read32(outputs), emu.read32(outputs + 4)

    def reference(address):
        if address == 0:
            return None
        if address == CELL:
            return 'cell'
        return TeamEmu.index_of(address)
    return dict(members=members, focus=focus, guard_slower=guard_slower,
                center=reference(center), closest=reference(closest), events=emu.events)



def center_member(rng, *, far=False):
    span = 0x7FFF00 if far else 0x6000
    return dict(xyz=[rng.randrange(0x100, span), rng.randrange(0x100, span),
                     rng.randrange(0, 0x400)],
                active=rng.random() > 0.1, health=rng.choice((0, 1, 100, 100, 100)),
                limbo=rng.random() < 0.1, initiated=rng.random() > 0.3,
                aircraft=rng.random() < 0.15, in_playfield=rng.random() > 0.1,
                passengers=rng.choice((0, 0, 0, 5)), naval=rng.random() < 0.3,
                weight=rng.random() < 0.5, can_enter=rng.choice((0, 0, 0, 1, 2)))


def center():
    rng = random.Random(0x6EAEE0)
    rows = []
    for case in range(120):
        far = case % 4 == 0
        members = [center_member(rng, far=far) for _ in range(rng.randrange(1, 7))]
        focus = None if case % 3 == 0 else [rng.randrange(0, 0x7FFF00 if far else 0x6000),
                                            rng.randrange(0, 0x7FFF00 if far else 0x6000),
                                            0]
        rows.append(center_row(members=members, focus=focus, guard_slower=case % 2 == 1))
    # Equal distances keep the first; a zero distance is replaced by the next.
    same = dict(active=True, health=100, limbo=False, initiated=True, aircraft=False,
                in_playfield=True, passengers=0, naval=False, weight=False, can_enter=0)
    rows.append(center_row(members=[dict(same, xyz=[1000, 1000, 0]),
                                    dict(same, xyz=[3000, 1000, 0]),
                                    dict(same, xyz=[1000, 3000, 0])],
                           focus=[2000, 2000, 0]))
    rows.append(center_row(members=[dict(same, xyz=[2000, 2000, 0]),
                                    dict(same, xyz=[2100, 2000, 0])],
                           focus=[2000, 2000, 0]))
    rows.append(center_row(members=[dict(same, xyz=[1000, 1000, 0], passengers=5, naval=True),
                                    dict(same, xyz=[1200, 1000, 0], passengers=5, naval=True)],
                           focus=[2000, 2000, 0]))
    rows.append(center_row(members=[dict(same, xyz=[1000, 1000, 0], can_enter=1),
                                    dict(same, xyz=[1500, 1000, 0], can_enter=1)]))
    return rows


# ---------------------------------------------------------------- recruit

RECRUIT = 0x6EAA90
CAN_ADD = 0x6EA610
ADD_MEMBER = 0x6EA500
GET_GROUP = 0x6F1870
TEAM_WAYPOINT = 0x6F18A0
FIRST_PASSENGER = 0x473450


def recruit_row(*, kind, candidates, zone=None, group=-1, recruiter=False, amount=2, count=0,
                slot=0):
    """`candidates`: dicts of location, group, house/type match and the
    Can_Add answer, in array order."""
    emu = TeamEmu()
    house, other_house = HOUSES, HOUSES + HOUSE_SIZE
    emu.write32(TEAM + 0x2C, house)
    entry_type = emu.techno_type_at(0, rtti=TYPE_RTTI[kind])
    other_type = emu.techno_type_at(1, rtti=TYPE_RTTI[kind])
    emu.write32(TASK_FORCE + 0x9C, slot + 1)
    emu.write32(TASK_FORCE + 0xA4 + slot * 8, amount)
    emu.write32(TASK_FORCE + 0xA8 + slot * 8, entry_type)
    emu.write32(TEAM + 0x88 + slot * 4, count)
    emu.write8(TEAM_TYPE + 0xA8, int(recruiter))
    emu.hook(GET_GROUP, lambda _e: group, 0)

    def waypoint(e):
        out = e.arg(0)
        e.write32(out, 0)
        return out
    emu.hook(TEAM_WAYPOINT, waypoint, 4)
    answers = {}

    def can_add(e):
        candidate = e.arg(0)
        e.events.append(['can_add', TeamEmu.index_of(candidate), e.arg(2)])
        return int(answers[candidate])
    emu.hook(CAN_ADD, can_add, 12)
    emu.hook(ADD_MEMBER, lambda e: (e.events.append(['add', TeamEmu.index_of(e.arg(0)),
                                                     e.arg(1)]), 1)[1], 8)
    emu.hook(FIRST_PASSENGER, lambda _e: 0, 0)
    array = ARRAYS
    for index, candidate in enumerate(candidates):
        address = emu.object(index, xyz=candidate['xyz'])
        emu.write32(address + 0x214, candidate['group'])
        emu.write32(address + 0x21C, house if candidate['own_house'] else other_house)
        emu.write32(address + 0x6C4, entry_type if candidate['entry_type'] else other_type)
        answers[address] = candidate['can_add']
        emu.write32(array + index * 4, address)
    pointer, length = CLASS_ARRAY[kind]
    emu.write32(pointer, array)
    emu.write32(length, len(candidates))
    if zone is not None:
        emu.write32(TEAM + 0x34, emu.object(len(candidates), xyz=zone))
    result = emu.invoke(RECRUIT, ecx=TEAM, args=(slot,))
    return dict(kind=kind, candidates=candidates, zone=zone, group=group, recruiter=recruiter,
                amount=amount, count=count, slot=slot, result=result, events=emu.events)


def recruit_candidate(rng, *, far=False, groups=(-1,)):
    span = 0x7FFF00 if far else 0x6000
    return dict(xyz=[rng.randrange(0, span), rng.randrange(0, span), 0],
                group=rng.choice(groups), own_house=rng.random() > 0.2,
                entry_type=rng.random() > 0.2, can_add=rng.random() > 0.3)


def recruit():
    rng = random.Random(0x6EAA90)
    rows = []
    for case in range(150):
        kind = ('infantry', 'unit', 'aircraft')[case % 3]
        far = case % 5 == 0
        groups = (-1,) if case % 4 else (-1, 3, 7)
        group = rng.choice((-1, -1, 3, -2)) if case % 4 == 0 else -1
        candidates = [recruit_candidate(rng, far=far, groups=groups)
                      for _ in range(rng.randrange(0, 8))]
        zone = None if case % 6 == 0 else [rng.randrange(0, 0x7FFF00 if far else 0x6000),
                                           rng.randrange(0, 0x7FFF00 if far else 0x6000), 0]
        rows.append(recruit_row(kind=kind, candidates=candidates, zone=zone, group=group,
                                recruiter=case % 7 == 3))
    # Equal keys keep the first; a full entry recruits nobody.
    same = dict(group=-1, own_house=True, entry_type=True, can_add=True)
    rows.append(recruit_row(kind='infantry', zone=[0x1000, 0x1000, 0],
                            candidates=[dict(same, xyz=[0x1100, 0x1000, 0]),
                                        dict(same, xyz=[0x1000, 0x1100, 0])]))
    rows.append(recruit_row(kind='unit', zone=[0x1000, 0x1000, 0], amount=2, count=2,
                            candidates=[dict(same, xyz=[0x1100, 0x1000, 0])]))
    rows.append(recruit_row(kind='unit', zone=[0x1000, 0x1000, 0], amount=-1, count=-3,
                            candidates=[dict(same, xyz=[0x1100, 0x1000, 0])]))
    return rows


# ---------------------------------------------------------------- distance

DISTANCE = 0x5F6360
FOUNDATIONS = 22


def distance_row(*, member, target, building=None):
    """`building`: the target's foundation index, else a Foot target."""
    emu = TeamEmu()
    self_address = emu.object(0, xyz=member)
    if building is None:
        target_address = emu.object(1, xyz=target, rtti=RTTI_UNIT)
    else:
        target_address = emu.object(1, vtable=BUILDING_VTABLE, rtti=RTTI_BUILDING)
        building_type = BUILDING_TYPES
        emu.write32(target_address + 0x520, building_type)
        emu.write32(building_type + 0xEF0, building)

        def coords(e):
            out = e.arg(0)
            for offset, value in zip((0, 4, 8), target):
                e.write32(out + offset, value)
            return out
        emu.hook(STUB_BUILDING_COORDS, coords, 4)
    result = i32(emu.invoke(DISTANCE, ecx=self_address, args=(target_address,)))
    return dict(member=member, target=target, building=building, result=result)


def distance():
    rng = random.Random(0x5F6360)
    rows = []
    for case in range(80):
        span = (0x400, 0x4000, 0x40000, 0x7FFF00)[case % 4]
        member = [rng.randrange(0, span), rng.randrange(0, span), rng.randrange(0, 0x800)]
        target = [rng.randrange(0, span), rng.randrange(0, span), rng.randrange(0, 0x800)]
        rows.append(distance_row(member=member, target=target))
    for foundation in range(FOUNDATIONS):
        for offset in (0, 64, 200, 700):
            rows.append(distance_row(member=[0x2000, 0x2000, 0],
                                     target=[0x2000 + offset, 0x2000, 0], building=foundation))
    return rows


# ---------------------------------------------------------------- guard

GUARD_CASE = 0x6E97CE
GUARD_END = 0x6E97EC


def guard_row(argument, frame=1234):
    emu = TeamEmu()
    emu.write32(FRAME, frame)
    uc = emu.uc
    sp = STACK_BASE + STACK_SIZE - 0x1000
    emu.write32(sp + 0x24, 0x5A5A5A5A)
    uc.reg_write(UC_X86_REG_ESP, sp)
    uc.reg_write(UC_X86_REG_ESI, TEAM)
    uc.reg_write(UC_X86_REG_ECX, argument & 0xFFFFFFFF)
    uc.reg_write(UC_X86_REG_EBX, 1)
    run_checked(uc, GUARD_CASE, GUARD_END)
    return dict(argument=argument, frame=frame, start=i32(emu.read32(TEAM + 0x58)),
                duration=i32(emu.read32(TEAM + 0x60)))


def guard():
    return [guard_row(argument) for argument in
            (0, 1, 2, 4, 20, 60, -1, -3, 0x08888888, 0x08888889, 0x7FFFFFFF, -0x80000000)]


# ---------------------------------------------------------------- regroup

REGROUP_AT_BASE = 0x6EFA10
REGROUP_FNPC = 0x6EFC3A
BASE_CENTER = 0x50DF30


def regroup_row(*, own, enemy=None, safe_distance, draw=None):
    emu = TeamEmu()
    house, enemy_house = HOUSES, HOUSES + HOUSE_SIZE
    leader = emu.object(0, xyz=[0x3000, 0x3000, 0])
    emu.write32(TEAM + 0x54, leader)
    emu.write8(leader + 0x90, 1)
    emu.write32(leader + 0x6C, 100)
    emu.write8(leader + 0x689, 1)
    emu.write32(leader + 0x21C, house)
    emu.techno_type[leader] = emu.techno_type_at(0)
    emu.write32(RULES + 0xD74, safe_distance)
    emu.write32(HOUSE_ARRAY, ARRAYS)
    emu.write32(ARRAYS + 4, enemy_house)
    emu.write32(house + 0x5600, 1 if enemy is not None else -1)
    centers = {house: own, enemy_house: enemy}

    def base_center(e):
        out = e.arg(0)
        for offset, value in zip((0, 4, 8), centers[e.uc.reg_read(UC_X86_REG_ECX)] + [0]):
            e.write32(out + offset, value)
        return out
    emu.hook(BASE_CENTER, base_center, 4)
    emu.draws([] if draw is None else [draw])
    emu.start(REGROUP_AT_BASE, REGROUP_FNPC, ecx=TEAM, args=(0, 1))
    # At the call, the seed cell is FNPC's second argument.
    seed_pointer = emu.read32(emu.uc.reg_read(UC_X86_REG_ESP) + 4)
    seed = struct.unpack('<hh', emu.uc.mem_read(seed_pointer, 4))
    return dict(own=own, enemy=enemy, safe_distance=safe_distance, draw=draw, seed=list(seed),
                draws=[event[1:] for event in emu.events if event[0] == 'draw'])


def regroup():
    rng = random.Random(0x6EFA10)
    rows = []
    for draw in (0, 1, 31, 32, 63, 64, 96, 127, 128, 160, 191, 192, 224, 255):
        for safe_distance in (10, 50):
            rows.append(regroup_row(own=[0x4080, 0x4080], safe_distance=safe_distance,
                                    draw=draw))
    for _ in range(60):
        own = [rng.randrange(0x80, 0x30000), rng.randrange(0x80, 0x30000)]
        enemy = [rng.randrange(0x80, 0x30000), rng.randrange(0x80, 0x30000)]
        rows.append(regroup_row(own=own, enemy=enemy,
                                safe_distance=rng.choice((0, 1, 10, 30, 50, 100))))
    for enemy in ([0x4080, 0x4080], [0x5080, 0x4080], [0x4080, 0x5080], [0x3080, 0x4080],
                  [0x4080, 0x3080], [0x5080, 0x5080], [0x3080, 0x3080]):
        rows.append(regroup_row(own=[0x4080, 0x4080], enemy=enemy, safe_distance=50))
    rows.append(regroup_row(own=[0, 0], safe_distance=50, draw=128))
    rows.append(regroup_row(own=[0x80, 0x80], enemy=[0x30080, 0x80], safe_distance=-5))
    return rows


# ---------------------------------------------------------------- gather

GATHER_AT_ENEMY_BASE = 0x6EF700
GATHER_FNPC = 0x6EF98A


def gather_row(*, own, enemy, safe_distance, leader=(0x3000, 0x3000, 0), has_enemy=True):
    """`own`/`enemy`: a base centre's XY, [0, 0] for none."""
    emu = TeamEmu()
    house, enemy_house = HOUSES, HOUSES + HOUSE_SIZE
    leader_address = emu.object(0, xyz=list(leader))
    emu.write32(TEAM + 0x54, leader_address)
    emu.write8(leader_address + 0x90, 1)
    emu.write32(leader_address + 0x6C, 100)
    emu.write8(leader_address + 0x689, 1)
    emu.write32(leader_address + 0x21C, house)
    emu.techno_type[leader_address] = emu.techno_type_at(0)
    emu.write32(RULES + 0xD74, safe_distance)
    emu.write32(HOUSE_ARRAY, ARRAYS)
    emu.write32(ARRAYS + 4, enemy_house)
    emu.write32(house + 0x5600, 1 if has_enemy else -1)
    centers = {house: own, enemy_house: enemy}

    def base_center(e):
        out = e.arg(0)
        for offset, value in zip((0, 4, 8), centers[e.uc.reg_read(UC_X86_REG_ECX)] + [0]):
            e.write32(out + offset, value)
        return out
    emu.hook(BASE_CENTER, base_center, 4)
    stop = emu.start(GATHER_AT_ENEMY_BASE, (GATHER_FNPC, RET_MAGIC), ecx=TEAM, args=(0, 1))
    seed = None
    if stop == GATHER_FNPC:
        seed_pointer = emu.read32(emu.uc.reg_read(UC_X86_REG_ESP) + 4)
        seed = list(struct.unpack('<hh', emu.uc.mem_read(seed_pointer, 4)))
    return dict(own=own, enemy=enemy, leader=list(leader), has_enemy=has_enemy,
                safe_distance=safe_distance, seed=seed, finished=bool(emu.read8(TEAM + 0x80)))


def gather():
    rng = random.Random(0x6EF700)
    rows = []
    for _ in range(60):
        own = [rng.randrange(0x80, 0x30000), rng.randrange(0x80, 0x30000)]
        enemy = [rng.randrange(0x80, 0x30000), rng.randrange(0x80, 0x30000)]
        rows.append(gather_row(own=own, enemy=enemy,
                               safe_distance=rng.choice((0, 1, 10, 30, 50, 100))))
    for own in ([0x4080, 0x4080], [0x5080, 0x4080], [0x4080, 0x5080], [0x3080, 0x4080],
                [0x4080, 0x3080], [0x5080, 0x5080], [0x3080, 0x3080]):
        rows.append(gather_row(own=own, enemy=[0x4080, 0x4080], safe_distance=50))
    for leader in ((0x2345, 0x6789, 0x100), (0x4080, 0x4080, 0)):
        rows.append(gather_row(own=[0, 0], enemy=[0x4080, 0x4080], leader=leader,
                               safe_distance=30))
    rows.append(gather_row(own=[0x80, 0x80], enemy=[0x30080, 0x80], safe_distance=-5))
    rows.append(gather_row(own=[0x4080, 0x4080], enemy=[0, 0], safe_distance=50))
    rows.append(gather_row(own=[0x4080, 0x4080], enemy=[0x8080, 0x8080], safe_distance=50,
                           has_enemy=False))
    return rows


# ---------------------------------------------------------------- quarry

QUARRY_TO_THREAT = 0x645BB0


def quarry():
    return [dict(quarry=argument,
                 mask=TeamEmu().invoke(QUARRY_TO_THREAT, ecx=argument & 0xFFFFFFFF))
            for argument in list(range(-2, 14)) + [0x7FFFFFFF, -0x80000000]]


# ---------------------------------------------------------------- attack_cadence

CADENCE = 0x6EB59A
CADENCE_ASKS, CADENCE_SKIPS = 0x6EB5B0, 0x6EB5D9


def attack_cadence():
    rows = []
    for frame in (list(range(0, 20)) + list(range(-12, 0))
                  + [0x7FFFFFFC, 0x7FFFFFFF, -0x80000000, -0x7FFFFFFC]):
        emu = TeamEmu()
        emu.write32(FRAME, frame)
        emu.uc.reg_write(UC_X86_REG_ESP, STACK_BASE + STACK_SIZE - 0x1000)
        stop = run_checked(emu.uc, CADENCE, (CADENCE_ASKS, CADENCE_SKIPS))
        rows.append(dict(frame=frame, asks=stop == CADENCE_ASKS))
    return rows


# ---------------------------------------------------------------- own_building

FIND_OWN_BUILDING = 0x6EEEA0
THREAT_AT = 0x56BCD0


def own_building_row(*, leader, buildings, mode):
    """`buildings`: (location, of the searched type) in the house's order."""
    emu = TeamEmu()
    house = HOUSES
    searched, other = BUILDING_TYPES, BUILDING_TYPES + 0x2000
    leader_address = emu.object(0, xyz=leader)
    emu.write32(leader_address + 0x21C, house)
    array = ARRAYS
    for index, (location, of_type) in enumerate(buildings):
        address = emu.object(index + 1, xyz=location, vtable=OBJECT_VTABLE,
                             rtti=RTTI_BUILDING)
        emu.write32(address + 0x520, searched if of_type else other)
        emu.write32(array + index * 4, address)
    emu.write32(house + 0x6C, array)
    emu.write32(house + 0x78, len(buildings))
    emu.hook(THREAT_AT, lambda _e: 0, 8)
    result = emu.invoke(FIND_OWN_BUILDING, ecx=searched, args=(leader_address, mode, 0))
    return dict(leader=leader, buildings=buildings, mode=mode,
                result=None if result == 0 else TeamEmu.index_of(result) - 1)


def own_building():
    rng = random.Random(0x6EEEA0)
    rows = []
    for case in range(60):
        span = 0x7FFF00 if case % 5 == 0 else 0x8000
        leader = [rng.randrange(0, span), rng.randrange(0, span), rng.randrange(0, 0x400)]
        buildings = [([rng.randrange(0, span), rng.randrange(0, span), rng.randrange(0, 0x400)],
                      rng.random() > 0.3) for _ in range(rng.randrange(0, 7))]
        rows.append(own_building_row(leader=leader, buildings=buildings,
                                     mode=(0, 1, 2, 3, 2, 3, 4, 0x10000)[case % 8]))
    tied = [([0x2100, 0x2000, 0], True), ([0x2000, 0x2100, 0], True)]
    for mode in (2, 3):
        rows.append(own_building_row(leader=[0x2000, 0x2000, 0], buildings=tied, mode=mode))
    return rows


def generate():
    return dict(recalc=recalc(), center=center(), recruit=recruit(), distance=distance(),
                guard=guard(), regroup=regroup(), own_building=own_building(),
                gather=gather(), quarry=quarry(), attack_cadence=attack_cadence())


if __name__ == '__main__':
    finish_vectors(generate, Path(__file__).with_suffix('.json'), provenance=lambda: provenance(
        scope=('TeamClass::Recalc over TaskForce totals, member counts and strength bytes; '
               'Calc_Center off action 10 over synthetic members and move targets; Recruit '
               'over one class array with supplied Can_Add answers; ObjectClass::Distance to '
               'a Foot or a building of every foundation; the action 5 guard timer; action '
               '54\'s point up to its FNPC call; FindOwnBuilding over one house\'s buildings; '
               'action 53 up to its FNPC call or its early return; Quarry_To_Threat; '
               'Coordinate_Attack\'s frame test.'),
        assumptions=['x87 control word 0x0E7F (PC53, chop), the harness default.',
                     'Fixture objects carry only the fields the entries read.',
                     'ScenarioInit (0xA8E7AC), the empty coordinate 0xB0E968 and the empty '
                     'cell 0xB0E948 read zero, as at load (BSS).'],
        substitutions=['WhatAmI (vt+0x2C), GetTechnoType (vt+0x84), the GuardSlower weight '
                       '(vt+0x4E0), Can_Enter_Cell (vt+0x1AC) and SetTarget (vt+0x3C8) answer '
                       'from the case; objects\' GetCoords is the original 0x5F65A0, a '
                       'building\'s answers the case\'s coordinate.',
                       'ScriptClass::Current 0x691500 answers action 0; '
                       'MapClass::GetCellAt 0x565730 records its coordinate and answers one '
                       'cell; the team destructor (vt+0x20) records its call.',
                       'Can_Add 0x6EA610 answers from the case and Add_Member 0x6EA500 '
                       'records its call; Get_Group 0x6F1870 answers the case\'s group; the '
                       'TeamType waypoint 0x6F18A0 answers the empty cell; the first '
                       'passenger 0x473450 answers none.',
                       'Base_Center 0x50DF30 answers the case\'s centres ([0, 0] for none, '
                       'z 0); RandomRanged 0x65C7E0 answers from the case and records its '
                       'range.',
                       'FindOwnBuilding\'s threat map read 0x56BCD0 answers 0.'],
        entry_points={'recalc': RECALC, 'calc_center': CALC_CENTER, 'recruit': RECRUIT,
                      'distance': DISTANCE, 'guard_case': GUARD_CASE,
                      'regroup_at_base': REGROUP_AT_BASE, 'find_own_building':
                      FIND_OWN_BUILDING, 'gather_at_enemy_base': GATHER_AT_ENEMY_BASE,
                      'quarry_to_threat': QUARRY_TO_THREAT, 'attack_cadence': CADENCE},
    ))
