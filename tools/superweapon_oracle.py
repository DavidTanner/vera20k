"""Native references for the nuclear missile's launch chain.

Run python -m tools.superweapon_oracle --check (or explicit --write).
Rust consumers: src/sim/superweapon/fire_tests.rs (click_fire,
defense_alert), src/sim/world/techno_ai/building_missile.rs (mission_missile),
src/sim/projectile/launch.rs (both velocities), src/sim/combat/nuke_maker_tests.rs
(nuke_maker) and src/sim/building_art_super.rs (super_anim, opening_super_anim).

Sections, each case in a fresh emulator (tools.ai_base_building_oracle's
fixture machinery):
- click_fire: SuperClass::ClickFire 0x6CB920 without charge drain or a
  one-time grant: admission, the Lightning Storm deferment refusal, the
  Launch call, readiness and the recharge timer writes.
- defense_alert: the computer house's launch alert 0x4FAF00 (Fire_SW's
  house loop): its gates, CoordStruct::Distance3D 0x41C380 of base minus
  cell, the Scenario RandomRanged(0, 99) draw and the stores.
- mission_missile: BuildingClass::Mission_Missile 0x44C980 in each status of
  a NukeSilo building (and a building that is not one): Begin_Mode modes,
  the PSIWARN and take-off anims, the bullet's construction and launch
  (coordinate and velocity bits, from the table sine/cosine 0x4CACB0 and
  0x4CAD00) and each returned delay.
- nuke_maker: BulletClass::NukeMaker 0x46B310: the NukePayload bullet's
  construction, launch coordinate and velocity bits.
- super_anim: BuildingClass::UpdateAnimation's SuperAnim block
  0x450F9E..0x451145 (GetHealthPercentage 0x5F5C60 and the slot clear
  0x451E40 run natively).
- opening_super_anim: OnConstructionComplete's first-opening block
  0x4463F0..0x446580.
"""
from pathlib import Path
import math
import struct

from unicorn.x86_const import (UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX,
                               UC_X86_REG_EDI, UC_X86_REG_EDX, UC_X86_REG_ESI,
                               UC_X86_REG_ESP, UC_X86_REG_FPCW)

from tools.ai_base_building_oracle import FAKE, RULES, STUBS, Emu, u32
from tools.native_oracle import (NATIVE_FPCW, STACK_BASE, STACK_SIZE, OracleError,
                                 finish_vectors, provenance, run_checked)

FRAME = 0xA8ED84
SW_TYPES = 0xA8E334
ANIM_TYPES = 0x8B4154
WEAPON_TYPES = 0x88756C
SCENARIO_PTR = 0xA8B230

# Fixture objects past tools.ai_base_building_oracle's regions.
BASE = FAKE + 0x400000
SUPER = BASE
SW_TYPE = BASE + 0x1000
HOUSE = BASE + 0x2000
HOUSE_TYPE = BASE + 0x20000
BUILDING = BASE + 0x21000
BUILDING_TYPE = BASE + 0x22000
BUILDING_VT = BASE + 0x24000
CELL = BASE + 0x25000
CELL_VT = BASE + 0x26000
BULLET = BASE + 0x27000
BULLET_VT = BASE + 0x28000
WEAPON = BASE + 0x29000
BULLET_TYPE = BASE + 0x2A000
WARHEAD = BASE + 0x2B000
ITEMS = BASE + 0x2C000
ANIM_TYPE_PSIWARN = BASE + 0x2D000
ANIM_TYPE_TAKEOFF = BASE + 0x2E000
YARD = BASE + 0x2F000
YARD_VT = BASE + 0x30000
TARGET = BASE + 0x31000
TARGET_VT = BASE + 0x32000
UP_WEAPON = BASE + 0x33000
UP_BULLET_TYPE = BASE + 0x34000
ANIM_VT = BASE + 0x35000
SLOT_ANIMS = BASE + 0x36000
NAMES = BASE + 0x38000
CELL_ARG = BASE + 0x3F000

STUB_LAUNCH = 0x6CC390
STUB_B_COORDS = STUBS + 0x100
STUB_B_FLH = STUBS + 0x110
STUB_QUEUE = STUBS + 0x120
STUB_C_COORDS = STUBS + 0x130
STUB_LIMBO = STUBS + 0x140
STUB_FIRE = STUBS + 0x150
STUB_DELETE = STUBS + 0x160
STUB_Y_COORDS = STUBS + 0x170
STUB_T_COORDS = STUBS + 0x180
STUB_MISSION = STUBS + 0x190
STUB_TYPE = STUBS + 0x1A0
STUB_OCCUPANTS = STUBS + 0x1B0
STUB_ANIM_DELETE = STUBS + 0x1C0

LEVEL_LEPTONS = 104


def i32(value):
    return struct.unpack('<i', u32(value))[0]


def f32_bits(value):
    return struct.unpack('<I', struct.pack('<f', value))[0]


def f64_bits(value):
    return struct.unpack('<Q', struct.pack('<d', value))[0]


def write8(emu, address, value):
    emu.uc.mem_write(address, bytes([int(value) & 0xFF]))


def read8(emu, address):
    return emu.uc.mem_read(address, 1)[0]


def write_coord(emu, address, coord):
    emu.uc.mem_write(address, struct.pack('<iii', *coord))


def read_coord(emu, address):
    return list(struct.unpack('<iii', emu.uc.mem_read(address, 12)))


def read_cell(emu, address):
    return list(struct.unpack('<hh', emu.uc.mem_read(address, 4)))


def read_name(emu, address):
    raw = bytes(emu.uc.mem_read(address, 0x40))
    return raw.split(b'\0', 1)[0].decode('ascii')


def cell_coords(cell, levels):
    """The coordinates a flat cell's GetCoords reports: its centre, raised by
    its level (`CellClass::GetCoords` vt+0x48 is supplied)."""
    x, y = cell
    return [x * 256 + 128, y * 256 + 128, levels.get(tuple(cell), 0) * LEVEL_LEPTONS]


class Cells:
    """MapClass::operator[] 0x5657A0 answering one fixture CellClass per
    lookup, whose GetCoords (vt+0x48) reports the looked-up cell's centre
    raised by its supplied level."""

    def __init__(self, emu, levels):
        self.levels = levels
        self.cells = {}
        emu.write32(CELL_VT + 0x48, STUB_C_COORDS)
        emu.hook(0x5657A0, self.lookup, 4)
        emu.hook(STUB_C_COORDS, self.coords, 4)

    def lookup(self, emu):
        looked = read_cell(emu, emu.arg(0))
        this = CELL + 0x40 * len(self.cells)
        emu.write32(this, CELL_VT)
        self.cells[this] = looked
        emu.events.append(['cell', looked])
        return this

    def coords(self, emu):
        out = emu.arg(0)
        cell = self.cells[emu.uc.reg_read(UC_X86_REG_ECX)]
        write_coord(emu, out, cell_coords(cell, self.levels))
        return out


def coords_stub(coords):
    """vt+0x48 GetCoords(out): write `coords` to the out pointer and return it."""
    def answer(emu):
        out = emu.arg(0)
        write_coord(emu, out, coords() if callable(coords) else coords)
        return out
    return answer


# ---------------------------------------------------------------- click_fire

TYPE_MULTI_MISSILE = 0
TYPE_LIGHTNING_STORM = 2


def click_fire_row(*, kind=TYPE_MULTI_MISSILE, pre_click=False, post_click=False,
                   manual=False, recharge=900, start=-1, left=0, granted=True,
                   charged=True, on_hold=False, frame=5000, deferment=False, player=True):
    emu = Emu()
    emu.write32(FRAME, frame)
    emu.write32(SUPER + 0x24, -1)
    emu.write32(SUPER + 0x28, SW_TYPE)
    emu.write32(SUPER + 0x2C, HOUSE)
    emu.write32(SUPER + 0x30, start)
    emu.write32(SUPER + 0x34, 0)
    emu.write32(SUPER + 0x38, left)
    emu.write32(SUPER + 0x68, 0)
    write8(emu, SUPER + 0x6C, 0)
    write8(emu, SUPER + 0x6D, granted)
    write8(emu, SUPER + 0x6E, 0)
    write8(emu, SUPER + 0x6F, charged)
    write8(emu, SUPER + 0x70, on_hold)
    emu.write32(SUPER + 0x78, 7)
    emu.write32(SUPER + 0x7C, 7)
    emu.write32(SW_TYPE + 0xB0, recharge)
    emu.write32(SW_TYPE + 0xB4, kind)
    write8(emu, SW_TYPE + 0xE5, 0)
    write8(emu, SW_TYPE + 0xED, pre_click)
    write8(emu, SW_TYPE + 0xEE, post_click)
    write8(emu, SW_TYPE + 0xF5, manual)
    emu.uc.mem_write(CELL_ARG, struct.pack('<hh', 33, 44))

    def launch(e):
        if e.uc.reg_read(UC_X86_REG_ECX) != SUPER or e.arg(0) != CELL_ARG:
            raise OracleError('Launch called with an unexpected Super or cell')
        e.events.append(['launch', e.arg(1) & 0xFF])
        return 0

    def has_deferment(e):
        e.events.append(['has_deferment'])
        return int(deferment)

    def storm_message(e):
        e.events.append(['storm_message'])

    def psydom_active(e):
        e.events.append(['psychic_dominator_active'])
        return 0

    emu.hook(STUB_LAUNCH, launch, 8)
    emu.hook(0x53A0E0, has_deferment, 0)
    emu.hook(0x53AE00, storm_message, 0)
    emu.hook(0x53B400, psydom_active, 0)
    result = emu.invoke(0x6CB920, ecx=SUPER, args=[int(player), CELL_ARG]) & 0xFF
    return dict(kind=kind, pre_click=pre_click, post_click=post_click, manual=manual,
                recharge=recharge, start=start, left=left, granted=granted,
                charged=charged, on_hold=on_hold, frame=frame, deferment=deferment,
                player=player, result=result, events=emu.events,
                start_after=emu.read_i32(SUPER + 0x30),
                left_after=emu.read_i32(SUPER + 0x38),
                granted_after=bool(read8(emu, SUPER + 0x6D)),
                charged_after=bool(read8(emu, SUPER + 0x6F)),
                on_hold_after=bool(read8(emu, SUPER + 0x70)),
                cameo_after=emu.read_i32(SUPER + 0x78))


def click_fire():
    rows = []
    frame = 5000
    timers = [(-1, 0), (-1, 300), (frame - 900, 900), (frame - 100, 900), (frame, 0)]
    for kind in (TYPE_MULTI_MISSILE, TYPE_LIGHTNING_STORM):
        for start, left in timers:
            for granted in (False, True):
                for charged in (False, True):
                    for on_hold in (False, True):
                        rows.append(click_fire_row(kind=kind, start=start, left=left,
                                                   granted=granted, charged=charged,
                                                   on_hold=on_hold, frame=frame))
    for deferment in (False, True):
        for player in (False, True):
            rows.append(click_fire_row(kind=TYPE_LIGHTNING_STORM, start=frame - 900,
                                       left=900, deferment=deferment, player=player))
    for pre_click, post_click, manual in ((True, False, False), (False, True, False),
                                          (False, False, True), (True, True, False),
                                          (False, True, True)):
        for start, left in ((-1, 0), (frame - 900, 900), (frame - 20, 900)):
            for charged in (False, True):
                rows.append(click_fire_row(pre_click=pre_click, post_click=post_click,
                                           manual=manual, start=start, left=left,
                                           charged=charged, frame=frame))
    for recharge in (0, 1, 4500, 27000):
        rows.append(click_fire_row(recharge=recharge, start=frame - 7, left=3))
        rows.append(click_fire_row(recharge=recharge, manual=True, start=frame - 7, left=3))
    return rows


# ---------------------------------------------------------------- defense_alert

def defense_alert_row(*, passive=False, human=False, defend=True, cell=(40, 40),
                      base=(30, 30), alternate=(0, 0), levels=None, distance=2560,
                      difficulty=0, probability=(50, 40, 30), answer=10, yard=None,
                      frame=777):
    levels = levels or {}
    emu = Emu()
    emu.write32(FRAME, frame)
    emu.write32(HOUSE + 0x34, HOUSE_TYPE)
    write8(emu, HOUSE_TYPE + 0x1A6, passive)
    write8(emu, HOUSE + 0x1EC, human)
    emu.write32(HOUSE + 0x184, difficulty)
    emu.uc.mem_write(HOUSE + 0x5490, struct.pack('<hh', *base))
    emu.uc.mem_write(HOUSE + 0x5494, struct.pack('<hh', *alternate))
    emu.uc.mem_write(HOUSE + 0x54F4, struct.pack('<hh', -7, -7))
    emu.write32(HOUSE + 0x54FC, -100)
    emu.write32(SUPER + 0x28, SW_TYPE)
    write8(emu, SW_TYPE + 0xEC, defend)
    emu.write32(RULES + 0xEE4, distance)
    emu.write32(RULES + 0xEC8, ITEMS)
    for slot, value in enumerate(probability):
        emu.write32(ITEMS + 4 * slot, value)
    if yard is not None:
        emu.write32(HOUSE + 0x54, ITEMS + 0x100)
        emu.write32(HOUSE + 0x60, 1)
        emu.write32(ITEMS + 0x100, YARD)
        emu.write32(YARD, YARD_VT)
        emu.write32(YARD_VT + 0x48, STUB_Y_COORDS)
        emu.hook(STUB_Y_COORDS, coords_stub(yard), 4)
    else:
        emu.write32(HOUSE + 0x60, 0)
    Cells(emu, levels)
    emu.uc.mem_write(CELL_ARG, struct.pack('<hh', *cell))
    emu.draws([answer])
    emu.invoke(0x4FAF00, ecx=HOUSE, args=[SUPER, CELL_ARG])
    draws = [event for event in emu.events if event[0] == 'draw']
    return dict(passive=passive, human=human, defend=defend, cell=list(cell),
                base=list(base), alternate=list(alternate),
                levels=[[x, y, level] for (x, y), level in sorted(levels.items())],
                distance=distance, difficulty=difficulty, probability=list(probability),
                answer=answer, yard=yard, frame=frame,
                draws=[[stream, i32(low), i32(high)] for _, stream, low, high, _ in draws],
                defense_cell=read_cell(emu, HOUSE + 0x54F4),
                defense_frame=emu.read_i32(HOUSE + 0x54FC))


def defense_alert():
    rows = [defense_alert_row(passive=True), defense_alert_row(human=True),
            defense_alert_row(defend=False)]
    # Distances on both sides of the limit, along an axis and a diagonal.
    for cell in ((40, 30), (39, 30), (41, 30), (37, 37), (38, 37), (38, 38), (30, 30)):
        for distance in (2560, 2559, 2561, 0):
            rows.append(defense_alert_row(cell=cell, distance=distance))
    # Height enters the distance.
    rows.append(defense_alert_row(cell=(40, 30), levels={(40, 30): 4}))
    rows.append(defense_alert_row(cell=(39, 30), levels={(30, 30): 3}))
    rows.append(defense_alert_row(cell=(39, 30), levels={(39, 30): 25}, distance=2600))
    # The draw against each difficulty's probability.
    for difficulty in (0, 1, 2):
        for answer in (0, 29, 30, 31, 40, 50, 51, 99):
            rows.append(defense_alert_row(cell=(32, 33), difficulty=difficulty,
                                          answer=answer))
    for probability in ((0, 0, 0), (100, 100, 100), (-1, -1, -1)):
        for answer in (0, 99):
            rows.append(defense_alert_row(cell=(32, 33), probability=probability,
                                          answer=answer))
    # The base: alternate centre over primary, the origin for none.
    rows.append(defense_alert_row(cell=(32, 33), alternate=(31, 31)))
    rows.append(defense_alert_row(cell=(32, 33), alternate=(60, 60)))
    rows.append(defense_alert_row(cell=(5, 5), base=(0, 0)))
    rows.append(defense_alert_row(cell=(1, 1), base=(0, 0), distance=500))
    rows.append(defense_alert_row(cell=(1, 1), base=(0, 0), alternate=(2, 2)))
    # The defended cell: the first construction yard's.
    # Building centres: a 1x1 yard on flat and raised ground, a 4x4 one.
    for yard in ([31 * 256 + 128, 29 * 256 + 128, 0], [31 * 256 + 128, 29 * 256 + 128, 208],
                 [12 * 256 + 512, 50 * 256 + 512, 0]):
        rows.append(defense_alert_row(cell=(32, 33), yard=yard))
        rows.append(defense_alert_row(cell=(32, 33), alternate=(31, 31), yard=yard))
    return rows


# ---------------------------------------------------------------- mission_missile

BULLET_SLOT = 1  # the SuperWeaponTypes index the silo fires (+0x5F8)


def mission_missile_row(*, status, silo=True, ready=False, target=(50, 60),
                        target_level=0, origin=(20 * 256 + 128, 30 * 256 + 128, 300),
                        fire_accepts=True, bullet=True, damage=1000, frame=1234):
    emu = Emu()
    emu.write32(FRAME, frame)
    emu.write32(BUILDING, BUILDING_VT)
    emu.write32(BUILDING + 0x520, BUILDING_TYPE)
    write8(emu, BUILDING_TYPE + 0x16BA, silo)
    emu.write32(BUILDING + 0xBC, status)
    write8(emu, BUILDING + 0x6DD, ready)
    emu.write32(BUILDING + 0x21C, HOUSE)
    emu.write32(BUILDING + 0x5F8, BULLET_SLOT)
    emu.write32(BUILDING + 0x54C, 0)
    emu.uc.mem_write(HOUSE + 0x5784, struct.pack('<hh', *target))
    emu.write32(SW_TYPES, ITEMS)
    emu.write32(ITEMS + 4 * BULLET_SLOT, SW_TYPE)
    emu.write32(SW_TYPE + 0x9C, WEAPON)
    emu.write32(WEAPON + 0xA0, BULLET_TYPE)
    emu.write32(WEAPON + 0xA4, damage)
    emu.write32(WEAPON + 0xAC, WARHEAD)
    emu.write32(ANIM_TYPES, ITEMS + 0x100)
    emu.write32(ITEMS + 0x100 + 4 * 3, ANIM_TYPE_PSIWARN)
    emu.write32(RULES + 0x98, ANIM_TYPE_TAKEOFF)
    emu.write32(BUILDING_VT + 0x48, STUB_B_COORDS)
    emu.write32(BUILDING_VT + 0xB0, STUB_B_FLH)
    emu.write32(BUILDING_VT + 0x1E8, STUB_QUEUE)
    cells = Cells(emu, {tuple(target): target_level})
    emu.write32(BULLET, BULLET_VT)
    emu.write32(BULLET_VT + 0xD4, STUB_LIMBO)
    emu.write32(BULLET_VT + 0x1F0, STUB_FIRE)
    emu.write32(BULLET_VT + 0x20, STUB_DELETE)
    anims = {}

    def begin_mode(e):
        e.events.append(['begin_mode', e.arg(0)])

    def find_anim_type(e):
        e.events.append(['anim_type', read_name(e, e.uc.reg_read(UC_X86_REG_ECX))])
        return 3

    def anim_ctor(e):
        this = e.uc.reg_read(UC_X86_REG_ECX)
        kind = {ANIM_TYPE_PSIWARN: 'PSIWARN', ANIM_TYPE_TAKEOFF: 'take_off'}[e.arg(0)]
        anims[this] = kind
        e.events.append(['anim', kind, read_coord(e, e.arg(1)), i32(e.arg(2)), i32(e.arg(3)),
                         e.arg(4), i32(e.arg(5)), e.arg(6) & 0xFF])
        return this

    def anim_bullet(e):
        attached = e.arg(0)
        e.events.append(['anim_bullet', anims[e.uc.reg_read(UC_X86_REG_ECX)],
                         'bullet' if attached == BULLET else attached])

    def anim_house(e):
        e.events.append(['anim_house', anims[e.uc.reg_read(UC_X86_REG_ECX)],
                         'house' if e.arg(0) == HOUSE else e.arg(0)])

    def create_bullet(e):
        if (e.uc.reg_read(UC_X86_REG_ECX) != BULLET_TYPE or e.arg(0) != BUILDING
                or e.arg(2) != WARHEAD):
            raise OracleError('CreateBullet called with unexpected type, owner or warhead')
        target_cell = cells.cells.get(e.uc.reg_read(UC_X86_REG_EDX))
        e.events.append(['create_bullet', target_cell, i32(e.arg(1)), i32(e.arg(3)),
                         e.arg(4) & 0xFF])
        return BULLET if bullet else 0

    def set_weapon(e):
        e.events.append(['set_weapon', 'weapon' if e.arg(0) == WEAPON else e.arg(0)])

    def limbo(e):
        e.events.append(['limbo'])
        return 0

    def fire(e):
        velocity = struct.unpack('<QQQ', e.uc.mem_read(e.arg(1), 24))
        e.events.append(['fire', read_coord(e, e.arg(0)), list(velocity)])
        return int(fire_accepts)

    def delete(e):
        e.events.append(['delete_bullet', e.arg(0)])

    def flh(e):
        if e.arg(1) != 0 or read_coord(e, e.uc.reg_read(UC_X86_REG_ESP) + 12) != [0, 0, 0]:
            raise OracleError('GetFLH called with an unexpected weapon or offset')
        e.events.append(['flh'])
        write_coord(e, e.arg(0), origin)
        return e.arg(0)

    def queue(e):
        e.events.append(['queue_mission', e.arg(0), e.arg(1) & 0xFF])

    emu.hook(0x447780, begin_mode, 4)
    emu.hook(0x427CB0, find_anim_type, 0)
    emu.hook(0x421EA0, anim_ctor, 0x1C)
    emu.hook(0x424C90, anim_bullet, 4)
    emu.hook(0x424CA0, anim_house, 4)
    emu.hook(0x46B050, create_bullet, 0x14)
    emu.hook(0x46B260, set_weapon, 4)
    emu.hook(STUB_LIMBO, limbo, 0)
    emu.hook(STUB_FIRE, fire, 8)
    emu.hook(STUB_DELETE, delete, 4)
    emu.hook(STUB_B_FLH, flh, 20)
    emu.hook(STUB_B_COORDS, coords_stub([1, 2, 3]), 4)
    emu.hook(STUB_QUEUE, queue, 8)
    if not silo:
        # The other arm reaches the mission rate; supply its MissionControl
        # row (0x5B3A00) with a Rate of one minute.
        emu.write32(BUILDING + 0x5F8, -1)
        row = ITEMS + 0x400
        emu.uc.mem_write(row + 0x10, struct.pack('<d', 1.0))
        emu.hook(0x5B3A00, lambda _e: row, 0)
    delay = i32(emu.invoke(0x44C980, ecx=BUILDING))
    anim_fields = {kind: dict(hidden=read8(emu, this + 0x19D),
                              z_adjust=emu.read_i32(this + 0x100))
                   for this, kind in anims.items()}
    warning = emu.read32(BUILDING + 0x54C)
    return dict(status=status, silo=silo, ready=ready, target=list(target),
                target_level=target_level, origin=list(origin), fire_accepts=fire_accepts,
                bullet=bullet, damage=damage, frame=frame, delay=delay, events=emu.events,
                status_after=emu.read_i32(BUILDING + 0xBC),
                ready_after=read8(emu, BUILDING + 0x6DD),
                warning_kept=anims.get(warning) if warning else None,
                psiwarn_hidden=anim_fields.get('PSIWARN', {}).get('hidden'),
                take_off_z_adjust=anim_fields.get('take_off', {}).get('z_adjust'))


def mission_missile():
    rows = []
    for status in range(5):
        for ready in (False, True):
            rows.append(mission_missile_row(status=status, ready=ready))
    rows.append(mission_missile_row(status=0, target=(3, 200), target_level=6,
                                    origin=(-5, 70000, -12)))
    rows.append(mission_missile_row(status=0, fire_accepts=False))
    rows.append(mission_missile_row(status=2, fire_accepts=False))
    rows.append(mission_missile_row(status=0, bullet=False))
    rows.append(mission_missile_row(status=0, silo=False))
    return rows


# ---------------------------------------------------------------- nuke_maker

def nuke_maker_row(*, target_coords=(50 * 256 + 128, 60 * 256 + 128, 0), cell_level=0,
                   altitude=7000, payload_speed=50, payload_damage=1000):
    emu = Emu()
    up = BULLET + 0x800
    emu.write32(up + 0x10C, TARGET)
    emu.write32(up + 0xB0, BUILDING)
    emu.write32(up + 0x130, UP_WEAPON)
    emu.write32(UP_WEAPON + 0xA0, UP_BULLET_TYPE)
    emu.write32(UP_BULLET_TYPE + 0x2BC, altitude)
    emu.write32(TARGET, TARGET_VT)
    emu.write32(TARGET_VT + 0x48, STUB_T_COORDS)
    emu.hook(STUB_T_COORDS, coords_stub(list(target_coords)), 4)
    Cells(emu, {(target_coords[0] // 256, target_coords[1] // 256): cell_level})

    def find_weapon(e):
        e.events.append(['weapon_type', read_name(e, e.uc.reg_read(UC_X86_REG_ECX))])
        return 2

    emu.hook(0x773030, find_weapon, 0)
    emu.write32(WEAPON_TYPES, ITEMS)
    emu.write32(ITEMS + 8, WEAPON)
    emu.write32(WEAPON + 0xA0, BULLET_TYPE)
    emu.write32(WEAPON + 0xA4, payload_damage)
    emu.write32(WEAPON + 0xA8, payload_speed)
    emu.write32(WEAPON + 0xAC, WARHEAD)
    emu.write32(BULLET, BULLET_VT)
    emu.write32(BULLET_VT + 0xD4, STUB_LIMBO)
    emu.write32(BULLET_VT + 0x1F0, STUB_FIRE)

    def co_create(e):
        # CoCreateInstance(CLSID, outer, context, IID, out): the new bullet.
        emu.write32(e.arg(4), BULLET)
        e.events.append(['co_create'])
        return 0

    def construct(e):
        if (e.uc.reg_read(UC_X86_REG_ECX) != BULLET or e.arg(0) != BULLET_TYPE
                or e.arg(1) != TARGET or e.arg(2) != BUILDING or e.arg(4) != WARHEAD):
            raise OracleError('Construct called with unexpected type, target, owner or warhead')
        e.events.append(['construct', i32(e.arg(3)), i32(e.arg(5)), e.arg(6) & 0xFF])

    def limbo(e):
        e.events.append(['limbo'])
        return 0

    def fire(e):
        velocity = struct.unpack('<QQQ', e.uc.mem_read(e.arg(1), 24))
        e.events.append(['fire', read_coord(e, e.arg(0)), list(velocity)])
        return 1

    emu.write32(0x7E15FC, STUBS + 0x1D0)
    emu.hook(STUBS + 0x1D0, co_create, 0x14)
    emu.hook(0x4664C0, construct, 0x1C)
    emu.hook(STUB_LIMBO, limbo, 0)
    emu.hook(STUB_FIRE, fire, 8)
    emu.invoke(0x46B310, ecx=up)
    return dict(target_coords=list(target_coords), cell_level=cell_level, altitude=altitude,
                payload_speed=payload_speed, payload_damage=payload_damage,
                events=emu.events,
                payload_weapon='weapon' if emu.read32(BULLET + 0x130) == WEAPON else None)


def nuke_maker():
    return [nuke_maker_row(),
            nuke_maker_row(target_coords=(50 * 256 + 255, 60 * 256, 0)),
            nuke_maker_row(target_coords=(7 * 256 + 128, 9 * 256 + 128, 416), cell_level=4,
                           altitude=0),
            nuke_maker_row(target_coords=(12927, 15487, 0), altitude=-30),
            nuke_maker_row(payload_speed=255, payload_damage=7)]


# ---------------------------------------------------------------- super anims

SLOT_NAMES = {14: (0x1304, 0x1314, 0x1324), 15: (0x1348, 0x1358, None),
              16: (0x138C, 0x139C, 0x13AC), 17: (0x13D0, 0x13E0, None)}


def install_super_anim_fixture(emu, *, kind, cat_bits, mission, supers, slots, health,
                               strength, names, frame, yellow=0.5):
    emu.write32(FRAME, frame)
    emu.write32(BUILDING, BUILDING_VT)
    emu.write32(BUILDING + 0x520, BUILDING_TYPE)
    emu.write32(BUILDING + 0x21C, HOUSE)
    emu.write32(BUILDING + 0x6C, health)
    emu.write32(BUILDING_TYPE + 0x16F0, kind)
    emu.write32(BUILDING_TYPE + 0x16E8, cat_bits)
    emu.write32(BUILDING_TYPE + 0xA0, strength)
    emu.write32(BUILDING_VT + 0x184, STUB_MISSION)
    emu.write32(BUILDING_VT + 0x88, STUB_TYPE)
    emu.write32(BUILDING_VT + 0x408, STUB_OCCUPANTS)
    emu.hook(STUB_MISSION, lambda _e: mission, 0)
    emu.hook(STUB_TYPE, lambda _e: BUILDING_TYPE, 0)
    emu.uc.mem_write(RULES + 0x1700, struct.pack('<d', yellow))
    emu.write32(HOUSE + 0x258, ITEMS)
    emu.write32(HOUSE + 0x264, len(supers))
    for index, (super_kind, start, left) in enumerate(supers):
        this = SUPER + 0x100 * index
        sw_type = SW_TYPE + 0x100 * index
        emu.write32(ITEMS + 4 * index, this)
        emu.write32(this + 0x28, sw_type)
        emu.write32(sw_type + 0xB4, super_kind)
        emu.write32(this + 0x30, start)
        emu.write32(this + 0x38, left)
    for slot in range(0x15):
        emu.write32(BUILDING + 0x55C + 4 * slot, 0)
    for slot in slots:
        anim = SLOT_ANIMS + 0x100 * slot
        emu.write32(anim, ANIM_VT)
        emu.write32(BUILDING + 0x55C + 4 * slot, anim)
    emu.write32(ANIM_VT + 0x20, STUB_ANIM_DELETE)

    def anim_delete(e):
        e.events.append(['delete_slot', (e.uc.reg_read(UC_X86_REG_ECX) - SLOT_ANIMS) // 0x100])

    emu.hook(STUB_ANIM_DELETE, anim_delete, 4)
    for slot, offsets in SLOT_NAMES.items():
        for variant, offset in enumerate(offsets):
            if offset is None:
                continue
            text = names.get((slot, variant), '')
            emu.uc.mem_write(BUILDING_TYPE + offset, text.encode('ascii') + b'\0')

    def play(e):
        name = read_name(e, e.arg(0))
        e.events.append(['play', name, i32(e.arg(1)), e.arg(2) & 0xFF, e.arg(3) & 0xFF,
                         i32(e.arg(4))])

    emu.hook(0x451890, play, 0x14)


def default_names():
    return {(14, 0): 'SA14', (14, 1): 'SA14D', (14, 2): 'SA14G', (15, 0): 'SA15',
            (15, 1): 'SA15D', (16, 0): 'SA16', (16, 1): 'SA16D', (16, 2): 'SA16G',
            (17, 0): 'SA17', (17, 1): 'SA17D'}


def super_anim_row(*, cat=1.0, cat_bits=None, kind=0, mission=1, supers=((0, -1, 899),),
                   slots=(14, 16), health=100, strength=100, names=None, frame=9000):
    cat_bits = f32_bits(cat) if cat_bits is None else cat_bits
    names = default_names() if names is None else names
    emu = Emu()
    install_super_anim_fixture(emu, kind=kind, cat_bits=cat_bits, mission=mission,
                               supers=supers, slots=slots, health=health,
                               strength=strength, names=names, frame=frame)
    uc = emu.uc
    sp = STACK_BASE + STACK_SIZE - 0x1000
    uc.reg_write(UC_X86_REG_ESP, sp)
    uc.reg_write(UC_X86_REG_ESI, BUILDING)
    uc.reg_write(UC_X86_REG_FPCW, NATIVE_FPCW)
    run_checked(uc, 0x450F9E, 0x451145, count=200_000)
    return dict(cat_bits=cat_bits, kind=kind, mission=mission,
                supers=[list(row) for row in supers], slots=list(slots), health=health,
                strength=strength, frame=frame,
                names={f'{slot}/{variant}': name for (slot, variant), name in names.items()},
                events=emu.events)


def super_anim():
    rows = []
    # The near-charged decision around ChargedAnimTime minutes.
    for cat, lefts in ((1.0, (0, 899, 900, 901, 5000)), (0.5, (449, 450, 451)),
                       (0.0, (0, 1)), (-1.0, (0, 1)), (2.25, (2024, 2025, 2026)),
                       (0.1, (89, 90, 91)), (990.0, (890999, 891000, 891001)),
                       (5.0 / 3.0, (1499, 1500, 1501))):
        for left in lefts:
            rows.append(super_anim_row(cat=cat, supers=((0, -1, left),)))
    # The gate on ChargedAnimTime itself, NaN and the infinities.
    for cat_bits in (f32_bits(990.0001), f32_bits(999.0), 0x7FC00000, 0x7F800000,
                     0xFF800000):
        rows.append(super_anim_row(cat_bits=cat_bits, supers=((0, -1, 10),)))
        rows.append(super_anim_row(cat_bits=cat_bits, supers=((0, -1, 10_000_000),)))
    # A running timer: elapsed against its duration.
    for start, left in ((9000 - 100, 999), (9000 - 100, 1000), (9000 - 100, 1001),
                        (9000 - 5000, 900), (9000, 0)):
        rows.append(super_anim_row(supers=((0, start, left),)))
    # Construction and Selling skip; the slots must be occupied.
    for mission in (0x12, 0x13, 0, 5):
        rows.append(super_anim_row(mission=mission, supers=((0, -1, 10),)))
    for slots in ((), (14,), (16,), (15, 17)):
        for left in (10, 5000):
            rows.append(super_anim_row(slots=slots, supers=((0, -1, left),)))
    # Only the building's weapon's Supers, each in turn.
    rows.append(super_anim_row(kind=-1, supers=((0, -1, 10),)))
    rows.append(super_anim_row(kind=2, supers=((0, -1, 10), (2, -1, 5000), (2, -1, 10))))
    rows.append(super_anim_row(kind=0, supers=((0, -1, 10), (0, -1, 5000))))
    # Health against ConditionYellow (0.5), and the names.
    for health in (51, 50, 49, 0):
        for left in (10, 5000):
            rows.append(super_anim_row(health=health, supers=((0, -1, left),)))
    # A replacement with no name (in its variant) plays nothing.
    rows.append(super_anim_row(names={(14, 0): 'SA14', (16, 0): 'SA16'},
                               supers=((0, -1, 10),)))
    rows.append(super_anim_row(names={(14, 0): 'SA14', (14, 1): 'SA14D', (15, 0): 'SA15'},
                               health=10, slots=(14,), supers=((0, -1, 10),)))
    return rows


def opening_row(*, kind=0, supers=((0, -1, 0),), health=100, strength=100,
                occupants=0, names=None, frame=9000):
    names = default_names() if names is None else names
    emu = Emu()
    install_super_anim_fixture(emu, kind=kind, cat_bits=f32_bits(999.0), mission=0x12,
                               supers=supers, slots=(), health=health, strength=strength,
                               names=names, frame=frame)
    emu.hook(STUB_OCCUPANTS, lambda _e: occupants, 0)
    uc = emu.uc
    sp = STACK_BASE + STACK_SIZE - 0x1000
    uc.reg_write(UC_X86_REG_ESP, sp)
    uc.reg_write(UC_X86_REG_EBP, BUILDING)
    uc.reg_write(UC_X86_REG_EDI, 0xFFFFFFFF)
    uc.reg_write(UC_X86_REG_EBX, 0)
    uc.reg_write(UC_X86_REG_FPCW, NATIVE_FPCW)
    run_checked(uc, 0x4463F0, 0x446580, count=200_000)
    return dict(kind=kind, supers=[list(row) for row in supers], health=health,
                strength=strength, occupants=occupants, frame=frame,
                names={f'{slot}/{variant}': name for (slot, variant), name in names.items()},
                events=emu.events)


def opening_super_anim():
    rows = []
    for left in (0, 1, 14, 15, 16, 29, 30, 9000, -1, -14, -15, -16):
        rows.append(opening_row(supers=((0, -1, left),)))
    for start, left in ((9000 - 10, 20), (9000 - 10, 24), (9000 - 10, 25), (9000 - 10, 26),
                        (9000 - 10, 10), (9000, 15)):
        rows.append(opening_row(supers=((0, start, left),)))
    rows.append(opening_row(kind=-1))
    rows.append(opening_row(kind=2, supers=((0, -1, 0), (2, -1, 500), (2, -1, 3))))
    for health, occupants in ((50, 0), (51, 0), (100, 2), (10, 2), (100, -1)):
        for left in (0, 500):
            rows.append(opening_row(health=health, occupants=occupants,
                                    supers=((0, -1, left),)))
    rows.append(opening_row(names={}))
    rows.append(opening_row(names={(16, 0): 'SA16'}, health=10))
    return rows


def generate():
    return {'source': 'unicorn/gamemd.exe', 'click_fire': click_fire(),
            'defense_alert': defense_alert(), 'mission_missile': mission_missile(),
            'nuke_maker': nuke_maker(), 'super_anim': super_anim(),
            'opening_super_anim': opening_super_anim()}


if __name__ == '__main__':
    here = Path(__file__)
    finish_vectors(generate, here.with_suffix('.json'), source_paths={
        'superweapon_oracle': here, 'ai_base_building_oracle':
            here.with_name('ai_base_building_oracle.py')},
        provenance=lambda: provenance(
        scope=('nuclear missile launch chain: ClickFire admission/refusal/recharge writes, '
               'the computer launch alert (distance, draw, stores), Mission_Missile by '
               'status with its anims, bullet and velocity bits, NukeMaker\'s payload '
               'bullet and velocity bits, and both SuperAnim blocks'),
        assumptions=['fresh emulator per case; fixture Super/House/Building/Bullet layouts '
                     'from live disassembly',
                     'x87 control word 0x0E7F (53-bit chop) at each entry',
                     'ClickFire: no charge drain, no one-time grant, CustomChargeTime -1',
                     'the opening block starts with EDI -1, as OnConstructionComplete '
                     'sets it at 0x445FCB'],
        substitutions=['Launch 0x6CC390, LightningStorm::HasDeferment 0x53A0E0 and '
                       'PrintMessage 0x53AE00, PsyDom::Active 0x53B400 (false) are recorded stubs',
                       'MapClass::operator[] 0x5657A0 answers one fixture cell; its GetCoords '
                       'vt+0x48 answers the centre raised 104 leptons per supplied level',
                       'object GetCoords vt+0x48, the silo GetFLH vt+0xB0 and the yard '
                       'GetCoords answer supplied coordinates',
                       'Begin_Mode 0x447780, Queue_Mission vt+0x1E8, the anim constructor '
                       '0x421EA0 and its setters 0x424C90/0x424CA0, CreateBullet 0x46B050, '
                       'SetWeaponType 0x46B260, CoCreateInstance, BulletClass::Construct '
                       '0x4664C0, Limbo vt+0xD4, Fire vt+0x1F0 and the bullet/anim deletes '
                       'are recorded stubs; the type finders 0x427CB0 and 0x773030 answer '
                       'fixture indexes',
                       'RandomRanged 0x65C7E0 answers from the row; MissionControl 0x5B3A00 '
                       'answers a one-minute Rate row',
                       'PlayAnim 0x451890, GetCurrentMission vt+0x184 and the occupant '
                       'count vt+0x408 are recorded/supplied stubs'],
        entry_points={'ClickFire': 0x6CB920, 'defense_alert': 0x4FAF00,
                      'Mission_Missile': 0x44C980, 'NukeMaker': 0x46B310,
                      'UpdateAnimation_super_anim': 0x450F9E,
                      'OnConstructionComplete_super_anim': 0x4463F0}))
