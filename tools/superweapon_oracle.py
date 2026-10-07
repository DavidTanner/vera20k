"""Native references for the superweapon chains: the nuclear missile's launch
and the computer houses' use of their superweapons.

Run python -m tools.superweapon_oracle --check (or explicit --write).
Rust consumers: src/sim/superweapon/fire_tests.rs (click_fire,
defense_alert), src/sim/world/techno_ai/building_missile.rs (mission_missile),
src/sim/projectile/launch.rs (both velocities), src/sim/combat/nuke_maker_tests.rs
(nuke_maker), src/sim/building_art_super.rs (super_anim, opening_super_anim)
src/sim/superweapon/ai_fire_tests.rs (the ai_* sections),
src/sim/superweapon/chronosphere_tests.rs (the chrono_* sections) and
src/sim/superweapon/psychic_dominator_tests.rs (psydom_*, update_lighting,
ambient_step).

Sections, each case in a fresh emulator (tools.ai_base_building_oracle's
fixture machinery):
- click_fire: SuperClass::ClickFire 0x6CB920 without charge drain or a
  one-time grant: admission, the Lightning Storm deferment refusal, the
  Psychic Dominator's refusal while one is active, the Launch call,
  readiness and the recharge timer writes.
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
- ai_try_fire: HouseClass::AI_TryFireSW 0x5098F0: the human gate, the
  Supers loop and its Type= jump table 0x509AE8, the MultiMissile arm, the
  Lightning Storm arm 0x509E00 (its storm gate 0x53A100) and the Force Shield
  arm; the other pickers and Fire_SW are recorded stubs.
- ai_best_rally_target: HouseClass::AI_FindBestRallyTarget 0x50CBF0 over a
  fixture TechnoClass::Array: candidates, values by kind and difficulty, the
  playfield test 0x578460, the cloak draws and the final pick, with the
  Scenario Random run natively (seeded by 0x65C6D0).
- ai_ground_rally_point: HouseClass::AI_GroundRallyPoint 0x509CD0: the base
  cell, Find_Nearby_Passable_Cell's arguments (a recorded stub) and the
  fired cell.
- ai_genetic_mutator: HouseClass::AI_Fire_GenMutator 0x509F60 over a fixture
  InfantryClass::Array and cell lists (CellClass::GetInfantry 0x47EC40 runs
  natively). Its cell-offset table 0xABD490 lies in BSS: the static
  initializer 0x561910 fills it first.
- chrono_process: a Chrono Warp's TeleportLocomotionClass from the first
  frame after Launch case 4 to the end of its piggyback: Process 0x7192F0,
  TimerCheck 0x719BF0, Is_Ok_To_End 0x719F30 and the constructor, Link and
  Begin_Piggyback run natively; the driver follows UnitClass::AI's prologue
  (0x7362A7..0x73635A) and FootClass::AI (0x4DA877, 0x4DAE5F..0x4DAEC3).
- chrono_update_position: TeleportLocomotionClass::Update_Position 0x718260
  over fixture cells and objects: the placement (Marked, floor height,
  bridge height and OnBridge), the destination cell's kills and blocks, and
  the blocked retarget (its zone and nearby-cell arguments and the new
  +0x288).
- chrono_destination: Launch case 4's +0x288 for one object of the source
  block, the Unit's and the others' arithmetic run as slices of Launch
  (0x6CC9AF..0x6CCA4C, 0x6CCB6A..0x6CCC2D).
- psydom_process: PsychicDominator::Process 0x53AF40, one step of each
  status, and status 2's first firing stage for every percent 0..100 (and a
  few outside) and anim frame count 1..64.
- psydom_start: PsyDom::Start 0x53AE50: its globals, the first anim's
  constructor arguments, Timer_1248 and the UpdateLighting call.
- update_lighting: ScenarioClass::UpdateLighting 0x53C280 for every
  NukeFlash, ChronoScreen, storm and Dominator state: the ambient target and
  RecalcLighting's arguments.
- ambient_step: LogicClass::PerTickUpdate's ambient fade
  (0x55B33D..0x55B4D7) run as a slice: gates, the interval each lighting
  state picks for Timer_1248, the target clamp and the clamped step.
- dominator_lighting_read: ScenarioClass::Set_Defaults's Dominator lighting
  values and Read_INI_Basic's conversion of each Dominator key, for a
  missing key and authored tokens.
- relight: CellClass::ProcessColourComponents 0x484180's Ground/Level arms
  (0x48445F..0x4845A2) for each storm, Dominator and NukeFlash state: the
  Dominator's top scalar reads NukeLevel (+0x3574), its bottom DominatorLevel.
"""
from pathlib import Path
import math
import struct

from unicorn import UC_HOOK_CODE
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX,
                               UC_X86_REG_ECX, UC_X86_REG_EDI, UC_X86_REG_EDX,
                               UC_X86_REG_ESI, UC_X86_REG_ESP, UC_X86_REG_FPCW)

from tools.ai_base_building_oracle import FAKE, RULES, SCENARIO, STUBS, Emu, u32
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
TYPE_PSYCHIC_DOMINATOR = 7


def click_fire_row(*, kind=TYPE_MULTI_MISSILE, pre_click=False, post_click=False,
                   manual=False, recharge=900, start=-1, left=0, granted=True,
                   charged=True, on_hold=False, frame=5000, deferment=False, player=True,
                   dominator_active=False):
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
        return int(dominator_active)

    def psydom_message(e):
        e.events.append(['dominator_message'])

    emu.hook(STUB_LAUNCH, launch, 8)
    emu.hook(0x53A0E0, has_deferment, 0)
    emu.hook(0x53AE00, storm_message, 0)
    emu.hook(0x53B400, psydom_active, 0)
    emu.hook(0x53B410, psydom_message, 0)
    result = emu.invoke(0x6CB920, ecx=SUPER, args=[int(player), CELL_ARG]) & 0xFF
    return dict(kind=kind, pre_click=pre_click, post_click=post_click, manual=manual,
                recharge=recharge, start=start, left=left, granted=granted,
                charged=charged, on_hold=on_hold, frame=frame, deferment=deferment,
                player=player, dominator_active=dominator_active, result=result,
                events=emu.events,
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
    for active in (False, True):
        for player in (False, True):
            rows.append(click_fire_row(kind=TYPE_PSYCHIC_DOMINATOR, start=frame - 900,
                                       left=900, dominator_active=active, player=player))
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


# ---------------------------------------------------------------- AI use

RANDOM_SEED = 0x65C6D0
RANDOM_RANGED = 0x65C7E0
RNG_BYTES = 0x3F4
GAME_MODE = 0xA8B238
HOUSE_ITEMS = 0xA8022C
TECHNO_ITEMS, TECHNO_COUNT = 0xA8EC7C, 0xA8EC88
FACTORY_ITEMS, FACTORY_COUNT = 0xA83E34, 0xA83E40
INFANTRY_ITEMS, INFANTRY_COUNT = 0xA83DEC, 0xA83DF8
CELL_TABLE = 0x87F924
SCENARIO_ACTIVE = 0xA8E9A0
STORM_ACTIVE = 0xA9FAB4
MAP = 0x87F7E8
SUPERS_VTABLE = 0x7EA4E4

AI = BASE + 0x40000
ENEMY = AI
ALLY = AI + 0x6000
POINTERS = AI + 0xC000
TECHNO_POINTERS = POINTERS
FACTORY_POINTERS = POINTERS + 0x400
INFANTRY_POINTERS = POINTERS + 0x800
HOUSE_POINTERS = POINTERS + 0xC00
SUPER_POINTERS = POINTERS + 0xD00
BUILD_CONST = POINTERS + 0xE00
BUILD_TECH = POINTERS + 0xF00
OBJECTS = AI + 0x10000
OBJECT_STRIDE = 0x800
AI_TYPES = AI + 0x30000
AI_TYPE_STRIDE = 0x2000
FACTORY_BLOCKS = AI + 0x70000
OBJECT_VT = AI + 0x71000
CELL_BLOCKS = AI + 0x72000
CELL_STRIDE = 0x200
AI_SUPERS = AI + 0x80000
AI_SW_TYPES = AI + 0x81000
VALUE_LISTS = AI + 0x82000
CELL_OUT = AI + 0x83000
CELL_POINTERS = BASE + 0x200000

# The cell-offset table 0xABD490 (BSS) and its static initializer, the only
# writer (cdecl, no arguments).
CELL_OFFSETS_INIT = 0x561910

STUB_WHAT = STUBS + 0x200
STUB_LAYER = STUBS + 0x210
STUB_O_COORDS = STUBS + 0x220
STUB_OWNER = STUBS + 0x230
STUB_HIGH = STUBS + 0x240
STUB_GET_CELL = STUBS + 0x250

WHAT = {'unit': 1, 'aircraft': 2, 'building': 6, 'infantry': 0xF}
TYPE_OFFSET = {'unit': 0x6C4, 'infantry': 0x6C0, 'building': 0x520}
FACTORY_KINDS = {'BuildingType': 7, 'InfantryType': 0x10, 'UnitType': 0x28,
                 'AircraftType': 3}
# AI_FindBestRallyTarget's per-difficulty Rules vectors (the item pointers),
# read by RulesClass::ReadGeneral 0x670801..0x670AE6.
VALUE_OFFSETS = {'AIIonCannonConYardValue': 0x1198, 'AIIonCannonWarFactoryValue': 0x11B4,
                 'AIIonCannonPowerValue': 0x11D0, 'AIIonCannonTechCenterValue': 0x11EC,
                 'AIIonCannonEngineerValue': 0x1208, 'AIIonCannonThiefValue': 0x1224,
                 'AIIonCannonHarvesterValue': 0x1240, 'AIIonCannonMCVValue': 0x125C,
                 'AIIonCannonAPCValue': 0x1278, 'AIIonCannonBaseDefenseValue': 0x1294,
                 'AIIonCannonPlugValue': 0x12B0, 'AIIonCannonHelipadValue': 0x12CC,
                 'AIIonCannonTempleValue': 0x12E8}
# Distinct per difficulty so the row shows which entry was read.
VALUES = {'AIIonCannonConYardValue': (100, 90, 80), 'AIIonCannonWarFactoryValue': (70, 71, 72),
          'AIIonCannonPowerValue': (60, 61, 62), 'AIIonCannonTechCenterValue': (50, 51, 52),
          'AIIonCannonEngineerValue': (11, 12, 13), 'AIIonCannonThiefValue': (14, 15, 16),
          'AIIonCannonHarvesterValue': (17, 18, 19), 'AIIonCannonMCVValue': (20, 21, 22),
          'AIIonCannonAPCValue': (23, 24, 25), 'AIIonCannonBaseDefenseValue': (35, 36, 37),
          'AIIonCannonPlugValue': (40, 41, 42), 'AIIonCannonHelipadValue': (43, 44, 45),
          'AIIonCannonTempleValue': (46, 47, 48)}
RETAIL_VALUES = {'AIIonCannonConYardValue': (100, 100, 100),
                 'AIIonCannonWarFactoryValue': (100, 100, 100),
                 'AIIonCannonPowerValue': (60, 100, 100),
                 'AIIonCannonTechCenterValue': (100, 100, 100),
                 'AIIonCannonEngineerValue': (1, 1, 1), 'AIIonCannonThiefValue': (1, 1, 1),
                 'AIIonCannonHarvesterValue': (1, 1, 1), 'AIIonCannonMCVValue': (1, 1, 1),
                 'AIIonCannonAPCValue': (1, 1, 1), 'AIIonCannonBaseDefenseValue': (35, 35, 35),
                 'AIIonCannonPlugValue': (40, 40, 40), 'AIIonCannonHelipadValue': (20, 20, 20),
                 'AIIonCannonTempleValue': (40, 40, 40)}
# Object types by their INI keys; the Rust replay parses the same keys.
TYPE_CATALOG = [
    ('HARV', 'unit', {'Harvester': 'yes'}),
    ('MCV', 'unit', {'DeploysInto': 'CONYARD'}),
    ('APC', 'unit', {'Passengers': '5'}),
    ('TANK', 'unit', {}),
    ('DEPLOYER', 'unit', {'DeploysInto': 'PLAIN', 'Passengers': '-1'}),
    ('HARVMCV', 'unit', {'Harvester': 'yes', 'DeploysInto': 'CONYARD', 'Passengers': '3'}),
    ('CONYARD', 'building', {'Factory': 'BuildingType'}),
    ('WEAP', 'building', {'Factory': 'UnitType'}),
    ('NAVALYARD', 'building', {'Factory': 'UnitType', 'Naval': 'yes', 'Power': '-20'}),
    ('BARRACKS', 'building', {'Factory': 'InfantryType', 'Power': '-10'}),
    ('POWER', 'building', {'Power': '100'}),
    ('DRAIN', 'building', {'Power': '-50'}),
    ('DEFENSE', 'building', {'IsBaseDefense': 'yes', 'Power': '-5'}),
    ('POWERDEFENSE', 'building', {'IsBaseDefense': 'yes', 'Power': '10'}),
    ('PLUG', 'building', {'IsPlug': 'yes'}),
    ('TEMPLE', 'building', {'IsTemple': 'yes', 'IsPlug': 'yes'}),
    ('PAD', 'building', {'HoverPad': 'yes'}),
    ('TECH', 'building', {'Power': '-100'}),
    ('PLAIN', 'building', {}),
    ('ENGI', 'infantry', {'Engineer': 'yes'}),
    ('THIEF', 'infantry', {'VehicleThief': 'yes'}),
    ('ENGITHIEF', 'infantry', {'Engineer': 'yes', 'VehicleThief': 'yes'}),
    ('GI', 'infantry', {}),
    ('JET', 'aircraft', {}),
]
BUILD_CONST_TYPES = ['CONYARD']
BUILD_TECH_TYPES = ['TECH', 'POWER', 'PAD']
# MapClass +0xF4/+0xFC/+0x100/+0x104/+0x108: the playfield the native
# IsCellInPlayfield 0x578460 tests (every cell lookup misses, so the level
# and slope are the dummy's zeros).
PLAYFIELD = {'base': 40, 'off_fc': 2, 'off_100': 4, 'off_104': 36, 'off_108': 36}
LAYER_GROUND, LAYER_AIR = 2, 3


def seed_scenario_rng(emu, seed):
    emu.invoke(RANDOM_SEED, ecx=SCENARIO_RANDOM, args=[seed])


def record_draws(emu):
    """RandomRanged 0x65C7E0 runs natively; each call's stream and bounds are
    recorded."""
    def entered(uc, _address, _size, _data):
        sp = uc.reg_read(UC_X86_REG_ESP)
        emu.events.append(['draw', uc.reg_read(UC_X86_REG_ECX) - SCENARIO_RANDOM + 0x218,
                           i32(emu.read32(sp + 4)), i32(emu.read32(sp + 8))])
    emu.uc.hook_add(UC_HOOK_CODE, entered, begin=RANDOM_RANGED, end=RANDOM_RANGED)


def rng_state(emu):
    return bytes(emu.uc.mem_read(SCENARIO_RANDOM, RNG_BYTES)).hex()


def install_playfield(emu):
    emu.write32(CELL_TABLE, CELL_POINTERS)
    for name, offset in (('base', 0xF4), ('off_fc', 0xFC), ('off_100', 0x100),
                         ('off_104', 0x104), ('off_108', 0x108)):
        emu.write32(MAP + offset, PLAYFIELD[name])


def install_houses(emu, *, difficulty=0, enemy_index=1, allies=0b100):
    """HouseClass::Array: the computer house, its enemy, and a house it
    counts as an ally (`+0x5788` bit 2)."""
    for index, house in enumerate((HOUSE, ENEMY, ALLY)):
        emu.write32(HOUSE_POINTERS + 4 * index, house)
        emu.write32(house + 0x30, index)
    emu.write32(HOUSE_ITEMS, HOUSE_POINTERS)
    emu.write32(HOUSE + 0x184, difficulty)
    emu.write32(HOUSE + 0x5600, enemy_index)
    emu.write32(HOUSE + 0x5788, allies)
    # The constructor's preferred target (type 1, no cell; 0x4F5A77/0x4F5A81).
    emu.write32(HOUSE + 0x54EC, 1)


def install_types(emu):
    types = {}
    for index, (name, what, _keys) in enumerate(TYPE_CATALOG):
        types[name] = AI_TYPES + index * AI_TYPE_STRIDE
    for name, what, keys in TYPE_CATALOG:
        ty = types[name]
        for key, value in keys.items():
            if key == 'Harvester':
                write8(emu, ty + 0xE0E, 1)
            elif key == 'DeploysInto':
                emu.write32(ty + 0x404, types[value])
            elif key == 'Passengers':
                emu.write32(ty + 0x5E0, int(value))
            elif key == 'Factory':
                emu.write32(ty + 0xEB8, FACTORY_KINDS[value])
            elif key == 'Naval':
                write8(emu, ty + 0xCCE, 1)
            elif key == 'Power':
                power = int(value)
                emu.write32(ty + 0xEE0, max(power, 0))
                emu.write32(ty + 0xEE4, max(-power, 0))
            elif key == 'IsBaseDefense':
                write8(emu, ty + 0x1706, 1)
            elif key == 'IsPlug':
                write8(emu, ty + 0x154D, 1)
            elif key == 'IsTemple':
                write8(emu, ty + 0x154C, 1)
            elif key == 'HoverPad':
                write8(emu, ty + 0x154E, 1)
            elif key == 'Engineer':
                write8(emu, ty + 0xEC3, 1)
            elif key == 'VehicleThief':
                write8(emu, ty + 0xEC6, 1)
            else:
                raise OracleError(f'unknown catalog key {key}')
    for base, names, items in ((0x8B0, BUILD_CONST_TYPES, BUILD_CONST),
                               (0x920, BUILD_TECH_TYPES, BUILD_TECH)):
        for slot, name in enumerate(names):
            emu.write32(items + 4 * slot, types[name])
        emu.write32(RULES + base, items)
        emu.write32(RULES + base + 0xC, len(names))
    return types


def install_values(emu, values):
    for slot, (key, offset) in enumerate(sorted(VALUE_OFFSETS.items())):
        items = VALUE_LISTS + slot * 0x10
        for index, value in enumerate(values[key]):
            emu.write32(items + 4 * index, value)
        emu.write32(RULES + offset, items)


SCENARIO_RANDOM = FAKE + 0x22000 + 0x218


def techno(owner='enemy', type='TANK', *, layer=LAYER_GROUND, alive=True, limbo=False,
           coords=(30 * 256 + 128, 30 * 256 + 128, 0), cloak=0, stage=0, factory=None):
    return dict(owner=owner, type=type, layer=layer, alive=alive, limbo=limbo,
                coords=list(coords), cloak=cloak, stage=stage, factory=factory)


def at(x, y, z=0):
    return (x * 256 + 128, y * 256 + 128, z)


def best_rally_row(objects, *, difficulty=1, values=VALUES, seed=31):
    emu = Emu()
    install_houses(emu, difficulty=difficulty)
    install_playfield(emu)
    install_values(emu, values)
    types = install_types(emu)
    whats = {name: what for name, what, _keys in TYPE_CATALOG}
    owners = {'self': HOUSE, 'enemy': ENEMY, 'ally': ALLY}
    facts = {}
    factories = []
    emu.write32(OBJECT_VT + 0x2C, STUB_WHAT)
    emu.write32(OBJECT_VT + 0x48, STUB_O_COORDS)
    emu.write32(OBJECT_VT + 0x78, STUB_LAYER)
    for index, obj in enumerate(objects):
        this = OBJECTS + index * OBJECT_STRIDE
        what = whats[obj['type']]
        facts[this] = (WHAT[what], obj)
        emu.write32(TECHNO_POINTERS + 4 * index, this)
        emu.write32(this, OBJECT_VT)
        emu.write32(this + 0x21C, owners[obj['owner']])
        emu.write32(this + 0x220, obj['cloak'])
        write8(emu, this + 0x90, obj['alive'])
        write8(emu, this + 0x81, obj['limbo'])
        if what in TYPE_OFFSET:
            emu.write32(this + TYPE_OFFSET[what], types[obj['type']])
        if what == 'building':
            write8(emu, this + 0x6ED, obj['stage'])
        if obj['factory'] is not None:
            factory = FACTORY_BLOCKS + len(factories) * 0x100
            emu.write32(factory + 0x58, this)
            emu.write32(factory + 0x38, obj['factory']['rate'])
            write8(emu, factory + 0x70, obj['factory']['suspended'])
            factories.append(factory)
    # A factory building something else stands first.
    idle = FACTORY_BLOCKS + 0xF00
    emu.write32(idle + 0x58, OBJECTS + 0x1F * OBJECT_STRIDE)
    emu.write32(idle + 0x38, 9)
    factories.insert(0, idle)
    for slot, factory in enumerate(factories):
        emu.write32(FACTORY_POINTERS + 4 * slot, factory)
    emu.write32(TECHNO_ITEMS, TECHNO_POINTERS)
    emu.write32(TECHNO_COUNT, len(objects))
    emu.write32(FACTORY_ITEMS, FACTORY_POINTERS)
    emu.write32(FACTORY_COUNT, len(factories))

    def what_am_i(e):
        return facts[e.uc.reg_read(UC_X86_REG_ECX)][0]

    def layer(e):
        e.events.append(['layer', (e.uc.reg_read(UC_X86_REG_ECX) - OBJECTS) // OBJECT_STRIDE])
        return facts[e.uc.reg_read(UC_X86_REG_ECX)][1]['layer']

    def coords(e):
        out = e.arg(0)
        write_coord(e, out, facts[e.uc.reg_read(UC_X86_REG_ECX)][1]['coords'])
        return out

    emu.hook(STUB_WHAT, what_am_i, 0)
    emu.hook(STUB_LAYER, layer, 0)
    emu.hook(STUB_O_COORDS, coords, 4)
    seed_scenario_rng(emu, seed)
    record_draws(emu)
    emu.uc.mem_write(CELL_OUT, struct.pack('<hh', -7, -7))
    emu.invoke(0x50CBF0, ecx=HOUSE, args=[CELL_OUT])
    return dict(difficulty=difficulty, values={key: list(v) for key, v in values.items()},
                seed=seed, objects=objects, events=emu.events, cell=read_cell(emu, CELL_OUT),
                rng_after=rng_state(emu))


def best_rally_target():
    rows = []
    # Each kind alone, at each difficulty: its value decides nothing, the
    # one candidate is the target (RandomRanged(0, 0)).
    for name, what, _keys in TYPE_CATALOG:
        for difficulty in (0, 1, 2):
            rows.append(best_rally_row([techno(type=name, coords=at(31, 32))],
                                       difficulty=difficulty))
    # Two kinds: the larger value wins, in either array order.
    pairs = [('HARV', 'MCV'), ('APC', 'TANK'), ('CONYARD', 'WEAP'), ('WEAP', 'NAVALYARD'),
             ('NAVALYARD', 'BARRACKS'), ('POWER', 'DRAIN'), ('DEFENSE', 'POWERDEFENSE'),
             ('PLUG', 'TEMPLE'), ('PAD', 'TECH'), ('TECH', 'PLAIN'), ('ENGI', 'THIEF'),
             ('ENGITHIEF', 'GI'), ('JET', 'GI'), ('DEPLOYER', 'HARVMCV'), ('TANK', 'JET')]
    for first, second in pairs:
        for order in ((first, second), (second, first)):
            rows.append(best_rally_row([techno(type=order[0], coords=at(31, 32)),
                                        techno(type=order[1], coords=at(40, 25))]))
    # Ties: the final RandomRanged(0, n - 1) picks, with retail values.
    for seed in (31, 32, 33, 34, 7, 1234567):
        rows.append(best_rally_row([techno(type='CONYARD', coords=at(31, 32)),
                                    techno(type='WEAP', coords=at(40, 25)),
                                    techno(type='TECH', coords=at(35, 35)),
                                    techno(type='POWER', coords=at(36, 36))],
                                   values=RETAIL_VALUES, seed=seed, difficulty=1))
    # Who is a candidate: other houses' objects, limbo, dead, airborne.
    rows.append(best_rally_row([techno(owner='self', type='CONYARD', coords=at(31, 32)),
                                techno(owner='ally', type='CONYARD', coords=at(33, 32)),
                                techno(type='GI', coords=at(40, 25))]))
    for flags in (dict(limbo=True), dict(alive=False), dict(layer=LAYER_AIR), dict(layer=4),
                  dict(layer=0)):
        rows.append(best_rally_row([techno(type='CONYARD', coords=at(31, 32), **flags),
                                    techno(type='GI', coords=at(40, 25))]))
        rows.append(best_rally_row([techno(type='CONYARD', coords=at(31, 32), **flags)]))
    # Off the playfield the value is zero, but the object stays a candidate.
    rows.append(best_rally_row([techno(type='CONYARD', coords=at(10, 10))]))
    rows.append(best_rally_row([techno(type='CONYARD', coords=at(10, 10)),
                                techno(type='GI', coords=at(40, 25))]))
    rows.append(best_rally_row([techno(type='GI', coords=at(70, 60)),
                                techno(type='TANK', coords=at(10, 10))]))
    # Coordinates to cells toward zero, at the playfield's edges.
    for coords in ((-100, 300, 0), (-300, -300, 0), (24 * 256 + 255, 24 * 256 + 255, 0),
                   (25 * 256, 24 * 256, 0), (61 * 256 + 128, 61 * 256 + 128, 0),
                   (62 * 256 + 128, 61 * 256 + 128, 0), (60 * 256, 25 * 256, 0),
                   (31 * 256 + 128, 32 * 256 + 128, 2000)):
        rows.append(best_rally_row([techno(type='TANK', coords=coords),
                                    techno(type='HARV', coords=at(40, 41))]))
    # Cloaked objects of any house draw RandomRanged(0, best + 10).
    for seed in (31, 99):
        rows.append(best_rally_row([techno(type='GI', coords=at(40, 25)),
                                    techno(owner='self', type='TANK', coords=at(31, 32),
                                           cloak=2),
                                    techno(type='TANK', coords=at(33, 32), cloak=2),
                                    techno(type='POWER', coords=at(34, 34), cloak=1),
                                    techno(type='POWER', coords=at(35, 34), cloak=3),
                                    techno(type='CONYARD', coords=at(36, 32), cloak=2,
                                           limbo=True)],
                                   seed=seed))
    # A building's cloak stage 15 draws as well (BuildingClass +0x6ED).
    for stage in (14, 15):
        rows.append(best_rally_row([techno(type='WEAP', coords=at(33, 32), stage=stage),
                                    techno(type='GI', coords=at(40, 25))]))
    # A Hard house also takes what an enemy factory is building.
    for difficulty in (0, 1):
        for factory in (dict(rate=5, suspended=False), dict(rate=0, suspended=False),
                        dict(rate=5, suspended=True)):
            rows.append(best_rally_row([techno(type='CONYARD', limbo=True, coords=(0, 0, 0),
                                               factory=factory)],
                                       difficulty=difficulty))
            rows.append(best_rally_row([techno(type='WEAP', limbo=True,
                                               coords=at(33, 32), factory=factory),
                                        techno(type='GI', coords=at(40, 25))],
                                       difficulty=difficulty))
    rows.append(best_rally_row([techno(owner='self', type='WEAP', limbo=True,
                                       coords=at(33, 32), factory=dict(rate=5,
                                                                       suspended=False))],
                               difficulty=0))
    # No enemy object: no target.
    rows.append(best_rally_row([]))
    rows.append(best_rally_row([techno(owner='self', type='WEAP', coords=at(33, 32))]))
    # A larger mix, several seeds.
    mix = [techno(type='GI', coords=at(40, 25)), techno(type='HARV', coords=at(41, 26)),
           techno(owner='ally', type='TANK', coords=at(45, 30), cloak=2),
           techno(type='POWER', coords=at(30, 31)), techno(type='POWER', coords=at(31, 31)),
           techno(type='DEFENSE', coords=at(32, 30), cloak=2),
           techno(type='JET', coords=at(36, 36), layer=LAYER_AIR),
           techno(type='POWER', coords=at(29, 31)), techno(type='ENGI', coords=at(28, 30))]
    for seed in (31, 5, 77, 2024):
        for difficulty in (0, 1, 2):
            rows.append(best_rally_row(mix, values=RETAIL_VALUES, seed=seed,
                                       difficulty=difficulty))
    return rows


def install_supers(emu, kinds):
    """The house's Supers vector (`+0x254`, DynamicVectorClass 0x7EA4E4):
    Super i of `kinds[i] = (Type=, charged)`."""
    emu.write32(HOUSE + 0x254, SUPERS_VTABLE)
    emu.write32(HOUSE + 0x258, SUPER_POINTERS)
    emu.write32(HOUSE + 0x25C, len(kinds))
    emu.write32(HOUSE + 0x264, len(kinds))
    for index, (kind, charged) in enumerate(kinds):
        this = AI_SUPERS + index * 0x100
        ty = AI_SW_TYPES + index * 0x100
        emu.write32(SUPER_POINTERS + 4 * index, this)
        emu.write32(this + 0x28, ty)
        write8(emu, this + 0x6F, charged)
        emu.write32(ty + 0xB4, kind)


def super_index(this):
    return (this - AI_SUPERS) // 0x100


def record_fire(emu):
    """Fire_SW 0x4FAE50 (thiscall, RET 8) is a recorded stub."""
    def fire(e):
        if e.uc.reg_read(UC_X86_REG_ECX) != HOUSE:
            raise OracleError('Fire_SW called on another house')
        e.events.append(['fire', i32(e.arg(0)), read_cell(e, e.arg(1))])
        return 1
    emu.hook(0x4FAE50, fire, 8)


def try_fire_row(kinds, *, game_mode=1, human=False, control=False, enemy_index=1,
                 storm=False, rally=(30, 31), defense=(0, 0), defense_frame=-100,
                 defense_frames=50, frame=1000):
    emu = Emu()
    install_houses(emu, enemy_index=enemy_index)
    install_supers(emu, kinds)
    emu.write32(GAME_MODE, game_mode)
    write8(emu, HOUSE + 0x1EC, human)
    write8(emu, HOUSE + 0x1ED, control)
    write8(emu, STORM_ACTIVE, storm)
    emu.uc.mem_write(HOUSE + 0x54F4, struct.pack('<hh', *defense))
    emu.write32(HOUSE + 0x54FC, defense_frame)
    emu.write32(RULES + 0xEE0, defense_frames)
    emu.write32(FRAME, frame)
    record_fire(emu)

    def picker(name, pops):
        def answer(e):
            e.events.append([name, super_index(e.arg(0))])
        emu.hook({'ground': 0x509CD0, 'psychic_dominator': 0x50A150,
                  'genetic_mutator': 0x509F60}[name], answer, pops)

    def best(e):
        e.events.append(['best_rally_target'])
        e.uc.mem_write(e.arg(0), struct.pack('<hh', *rally))
        return e.arg(0)

    picker('ground', 4)
    picker('psychic_dominator', 4)
    picker('genetic_mutator', 4)
    emu.hook(0x50CBF0, best, 4)
    emu.invoke(0x5098F0, ecx=HOUSE)
    return dict(kinds=[[kind, charged] for kind, charged in kinds], game_mode=game_mode,
                human=human, control=control, enemy_index=enemy_index, storm=storm,
                rally=list(rally), defense=list(defense), defense_frame=defense_frame,
                defense_frames=defense_frames, frame=frame, events=emu.events)


def try_fire():
    rows = []
    every = [(kind, True) for kind in range(12)]
    rows.append(try_fire_row(every))
    rows.append(try_fire_row([(kind, False) for kind in range(12)]))
    rows.append(try_fire_row(list(reversed(every)), defense=(20, 21), defense_frame=990))
    for game_mode, human, control in ((1, True, False), (1, False, True), (0, False, True),
                                      (0, True, False), (0, False, False)):
        rows.append(try_fire_row(every, game_mode=game_mode, human=human, control=control))
    rows.append(try_fire_row(every, enemy_index=-1))
    rows.append(try_fire_row(every, storm=True))
    rows.append(try_fire_row(every, rally=(0, 0)))
    rows.append(try_fire_row(every, rally=(0, 5)))
    rows.append(try_fire_row([(0, True), (0, True), (2, True)], rally=(5, 0)))
    # Force Shield: the alert's cell while the alert is younger than
    # AISuperDefenseFrames (`0x00509A7F..0x00509A99`).
    for defense, defense_frame, defense_frames, frame in (
            ((20, 21), 990, 50, 1000), ((20, 21), 950, 50, 1000), ((20, 21), 951, 50, 1000),
            ((20, 21), 949, 50, 1000), ((20, 21), -100, 50, 0), ((20, 21), -100, 50, -51),
            ((20, 21), 1000, 0, 1000), ((20, 21), 1000, -1, 999), ((0, 0), 990, 50, 1000),
            ((0, 7), 990, 50, 1000), ((7, 0), 990, 50, 1000),
            ((20, 21), 0x7FFFFFF0, 0x20, 0x7FFFFFF0), ((20, 21), -100, 50, -100)):
        rows.append(try_fire_row([(10, True)], defense=defense, defense_frame=defense_frame,
                                 defense_frames=defense_frames, frame=frame))
    return rows


def ground_rally_row(*, enemy_index=1, enemy_base=(30, 31), enemy_alternate=(0, 0),
                     own_base=(10, 11), own_alternate=(0, 0), found=(32, 33), supers=3,
                     fired=1, frame=1000):
    emu = Emu()
    install_houses(emu, enemy_index=enemy_index)
    install_supers(emu, [(5, True)] * supers)
    emu.write32(FRAME, frame)
    for house, base, alternate in ((HOUSE, own_base, own_alternate),
                                   (ENEMY, enemy_base, enemy_alternate)):
        emu.uc.mem_write(house + 0x5490, struct.pack('<hh', *base))
        emu.uc.mem_write(house + 0x5494, struct.pack('<hh', *alternate))
    record_fire(emu)

    def nearby(e):
        args = [i32(e.arg(index)) for index in range(15)]
        e.events.append(['find_nearby', read_cell(e, e.arg(1)), args[2:12],
                         read_cell(e, e.arg(12)), args[13:]])
        e.uc.mem_write(e.arg(0), struct.pack('<hh', *found))
        return e.arg(0)

    emu.hook(0x56DC20, nearby, 0x3C)
    emu.invoke(0x509CD0, ecx=HOUSE, args=[AI_SUPERS + fired * 0x100])
    return dict(enemy_index=enemy_index, enemy_base=list(enemy_base),
                enemy_alternate=list(enemy_alternate), own_base=list(own_base),
                own_alternate=list(own_alternate), found=list(found), supers=supers,
                fired=fired, events=emu.events)


def ground_rally_point():
    return [ground_rally_row(), ground_rally_row(enemy_alternate=(40, 41)),
            ground_rally_row(enemy_base=(0, 0)), ground_rally_row(enemy_base=(0, 0),
                                                                 enemy_alternate=(0, 0)),
            ground_rally_row(enemy_index=-1), ground_rally_row(enemy_index=-1,
                                                               own_alternate=(12, 13)),
            ground_rally_row(found=(0, 0)), ground_rally_row(found=(-2, -2)),
            ground_rally_row(found=(32766, 5)), ground_rally_row(supers=1, fired=0),
            ground_rally_row(supers=5, fired=4)]


def gen_mutator_row(objects):
    """`objects`: (kind, owner, cell, extra) in spawn order; each infantry is
    InfantryClass::Array's next entry, each lists itself in its cell like
    Unlimbo: a non-building at the head, a building at the tail."""
    emu = Emu()
    install_houses(emu)
    install_playfield(emu)
    install_supers(emu, [(9, True), (9, True)])
    write8(emu, SCENARIO_ACTIVE, 1)
    record_fire(emu)
    owners = {'self': HOUSE, 'enemy': ENEMY, 'ally': ALLY}
    cells = {}
    facts = {}
    infantry = []

    def cell_block(cell):
        cell = tuple(cell)
        if cell not in cells:
            block = CELL_BLOCKS + len(cells) * CELL_STRIDE
            emu.uc.mem_write(block + 0x24, struct.pack('<hh', *cell))
            cells[cell] = block
        return cells[cell]

    emu.write32(OBJECT_VT + 0x2C, STUB_WHAT)
    emu.write32(OBJECT_VT + 0x3C, STUB_OWNER)
    emu.write32(OBJECT_VT + 0x54, STUB_HIGH)
    emu.write32(OBJECT_VT + 0x1BC, STUB_GET_CELL)
    for index, (kind, owner, cell, extra) in enumerate(objects):
        this = OBJECTS + index * OBJECT_STRIDE
        bridge = extra.get('bridge', False)
        facts[this] = dict(what=WHAT.get(kind, 0x24), owner=owners[owner],
                           high=extra.get('high', False), cell=tuple(cell))
        emu.write32(this, OBJECT_VT)
        write8(emu, this + 0x8C, bridge)
        write8(emu, this + 0x81, extra.get('limbo', False))
        block = cell_block(cell)
        head = block + (0xE8 if bridge else 0xE4)
        if not extra.get('limbo', False):
            if kind == 'building':
                tail = head - 0x30
                while emu.read32(tail + 0x30):
                    tail = emu.read32(tail + 0x30)
                emu.write32(tail + 0x30, this)
            else:
                emu.write32(this + 0x30, emu.read32(head))
                emu.write32(head, this)
        if kind == 'infantry':
            emu.write32(INFANTRY_POINTERS + 4 * len(infantry), this)
            infantry.append(this)
    emu.write32(INFANTRY_ITEMS, INFANTRY_POINTERS)
    emu.write32(INFANTRY_COUNT, len(infantry))

    def lookup(e):
        return cell_block(read_cell(e, e.arg(0)))

    emu.hook(0x5657A0, lookup, 4)
    emu.hook(STUB_WHAT, lambda e: facts[e.uc.reg_read(UC_X86_REG_ECX)]['what'], 0)
    emu.hook(STUB_OWNER, lambda e: facts[e.uc.reg_read(UC_X86_REG_ECX)]['owner'], 0)
    emu.hook(STUB_HIGH, lambda e: int(facts[e.uc.reg_read(UC_X86_REG_ECX)]['high']), 0)
    emu.hook(STUB_GET_CELL,
             lambda e: cell_block(facts[e.uc.reg_read(UC_X86_REG_ECX)]['cell']), 0)
    emu.invoke(CELL_OFFSETS_INIT)
    emu.invoke(0x509F60, ecx=HOUSE, args=[AI_SUPERS + 0x100])
    return dict(objects=[[kind, owner, list(cell), extra]
                         for kind, owner, cell, extra in objects],
                events=emu.events)


def gen_mutator():
    def inf(owner, cell, **extra):
        return ('infantry', owner, cell, extra)

    def unit(owner, cell, **extra):
        return ('unit', owner, cell, extra)

    rows = [
        gen_mutator_row([inf('enemy', (30, 31))]),
        gen_mutator_row([inf('self', (30, 31))]),
        gen_mutator_row([inf('ally', (30, 31))]),
        gen_mutator_row([inf('self', (30, 31)), inf('enemy', (31, 31))]),
        # The densest neighbourhood; ties keep the later infantry (the scan
        # runs last to first and needs a strictly larger count).
        gen_mutator_row([inf('enemy', (30, 31)), inf('enemy', (30, 31)),
                         inf('enemy', (40, 41)), inf('enemy', (41, 41)),
                         inf('enemy', (40, 42))]),
        gen_mutator_row([inf('enemy', (30, 31)), inf('enemy', (31, 31)),
                         inf('enemy', (40, 41)), inf('enemy', (41, 41))]),
        # The spread cells (table entries 0..=9): the later infantry wins a
        # tie, so (30, 30) wins only when it sees the other one.
        gen_mutator_row([inf('enemy', (30, 30)), inf('enemy', (29, 28))]),
        gen_mutator_row([inf('enemy', (30, 30)), inf('enemy', (30, 28))]),
        gen_mutator_row([inf('enemy', (30, 30)), inf('enemy', (31, 28))]),
        gen_mutator_row([inf('enemy', (30, 30)), inf('enemy', (31, 29))]),
        gen_mutator_row([inf('enemy', (30, 30)), inf('enemy', (29, 31))]),
        gen_mutator_row([inf('enemy', (30, 30)), inf('enemy', (32, 30))]),
        # A cell's list after GetInfantry stops at its first non-infantry.
        gen_mutator_row([inf('enemy', (30, 31)), unit('enemy', (30, 31)),
                         inf('enemy', (30, 31)), inf('self', (36, 36))]),
        gen_mutator_row([unit('enemy', (30, 31)), inf('enemy', (30, 31)),
                         inf('enemy', (30, 31)), inf('self', (36, 36))]),
        gen_mutator_row([inf('enemy', (30, 31)), ('building', 'enemy', (30, 31), {}),
                         inf('enemy', (30, 31)), inf('self', (36, 36))]),
        # High-flying infantry do not count; neither does an own or allied one.
        gen_mutator_row([inf('enemy', (30, 31), high=True), inf('self', (31, 31))]),
        gen_mutator_row([inf('enemy', (30, 31), high=True), inf('enemy', (30, 31)),
                         inf('ally', (31, 31))]),
        # The bridge list is read for an infantry on a bridge.
        gen_mutator_row([inf('enemy', (30, 31), bridge=True), inf('self', (31, 31))]),
        gen_mutator_row([inf('enemy', (30, 31), bridge=True),
                         inf('self', (31, 31), bridge=True)]),
        # A limbo infantry is no centre and lists nowhere.
        gen_mutator_row([inf('enemy', (30, 31), limbo=True), inf('self', (36, 36))]),
        # Off the playfield nothing fires.
        gen_mutator_row([inf('enemy', (10, 10))]),
        gen_mutator_row([inf('enemy', (10, 10)), inf('enemy', (11, 10)),
                         inf('enemy', (40, 41))]),
        gen_mutator_row([]),
    ]
    return rows


# ---------------------------------------------------------------- chrono_process

# A Chrono Warp's Teleport and its owner as Launch case 4 leaves them
# (0x6CC9F2..0x6CCB43): a fresh TeleportLocomotionClass (constructor
# 0x718000, Link_To_Object 0x55A710, Begin_Piggyback 0x719E90, all native)
# over the owner's locomotor; the owner latched (+0x27C) with its
# destination (+0x288).
CHRONO = BASE + 0x300000
TELEPORT = CHRONO
OWNER = CHRONO + 0x1000
OWNER_VT = CHRONO + 0x2000
OWNER_TYPE = CHRONO + 0x3000
STASH = CHRONO + 0x4000
STASH_VT = CHRONO + 0x5000
WARP_ANIM_TYPE = CHRONO + 0x6000
CHRONO_OUT_SOUND, CHRONO_IN_SOUND = 12, 13
SOURCE_COORD = (21 * 256 + 128, 21 * 256 + 128, 0)
DEST_COORD = (40 * 256 + 128, 40 * 256 + 128, 0)

STUB_TECHNO_TYPE = STUBS + 0x300
STUB_MARK = STUBS + 0x310
STUB_SET_LOCATION = STUBS + 0x320
STUB_SET_HEIGHT = STUBS + 0x330
STUB_MAP_COORDS = STUBS + 0x340
STUB_VT_18C = STUBS + 0x350
STUB_SET_DESTINATION = STUBS + 0x360
STUB_IDLE = STUBS + 0x370
STUB_ADDREF = STUBS + 0x380


def chrono_process_row(*, blocks=0, chrono_delay=60, stale_delay=0, start=1000,
                       frames=200):
    """Frames of a Chrono Warp from the first frame after Launch case 4.

    Each frame follows UnitClass::AI and FootClass::AI's control flow: the
    prologue's extra Process while WarpingIn (+0x271, vt+0x1D8 0x70C5C0) or
    BeingWarpedOut (+0x270, vt+0x1D4 0x70C5B0) with the latch
    (0x7362A7..0x7362F5); the frozen return while BeingWarpedOut
    (0x7362FB..0x73635A, no Temporal attacker); FootClass::AI's Process
    (0x4DA877) and its end of the piggyback when Is_Ok_To_End answers true
    (0x4DAE5F..0x4DAEC3). Process 0x7192F0, its TimerCheck 0x719BF0 and
    Is_Ok_To_End 0x719F30 run natively. Rows keep the frames whose state,
    owner bytes, timer or events changed."""
    emu = Emu()
    emu.write32(FRAME, start - 1)
    emu.write32(STASH, STASH_VT)
    emu.write32(STASH_VT + 4, STUB_ADDREF)
    emu.hook(STUB_ADDREF, lambda _e: 1, 4)
    emu.invoke(0x718000, ecx=TELEPORT)
    emu.invoke(0x55A710, args=[TELEPORT + 4, OWNER])
    emu.invoke(0x719E90, args=[TELEPORT + 0x18, STASH])
    emu.write32(OWNER, OWNER_VT)
    write8(emu, OWNER + 0x8C, 0)
    write8(emu, OWNER + 0x90, 1)
    write_coord(emu, OWNER + 0x9C, SOURCE_COORD)
    write8(emu, OWNER + 0x270, 0)
    write8(emu, OWNER + 0x271, 0)
    write8(emu, OWNER + 0x27C, 1)
    emu.write32(OWNER + 0x280, 0)
    emu.write32(OWNER + 0x284, stale_delay)
    write_coord(emu, OWNER + 0x288, DEST_COORD)
    emu.write32(OWNER + 0x2B4, 0)
    write8(emu, OWNER + 0x3D5, 1)
    emu.write32(OWNER + 0x428, 1)
    emu.write32(OWNER + 0x42C, HOUSE)
    write8(emu, OWNER + 0x6AD, 0)
    emu.write32(OWNER_TYPE + 0x574, -1)
    emu.write32(OWNER_TYPE + 0x578, -1)
    emu.write32(RULES + 0x218, CHRONO_IN_SOUND)
    emu.write32(RULES + 0x21C, CHRONO_OUT_SOUND)
    emu.write32(RULES + 0x33C, WARP_ANIM_TYPE)
    emu.write32(RULES + 0xBEC, chrono_delay)
    pending_blocks = [blocks]

    def owner_call(slot, stub, pops, answer):
        emu.write32(OWNER_VT + slot, stub)
        emu.hook(stub, answer, pops)

    def event(*fields):
        emu.events.append(list(fields))

    def set_location(e):
        coord = read_coord(e, e.arg(0))
        write_coord(e, OWNER + 0x9C, coord)
        event('set_location', coord)

    def map_coords(e):
        out = e.arg(0)
        x, y, _ = read_coord(e, OWNER + 0x9C)
        e.uc.mem_write(out, struct.pack('<hh', x // 256, y // 256))
        return out

    owner_call(0x84, STUB_TECHNO_TYPE, 0, lambda _e: OWNER_TYPE)
    owner_call(0x124, STUB_MARK, 4, lambda e: event('mark', e.arg(0)))
    owner_call(0x1B4, STUB_SET_LOCATION, 4, set_location)
    owner_call(0x1CC, STUB_SET_HEIGHT, 4, lambda e: event('set_height', i32(e.arg(0))))
    owner_call(0x1B8, STUB_MAP_COORDS, 4, map_coords)
    owner_call(0x18C, STUB_VT_18C, 4, lambda e: event('vt_18c', e.arg(0)))
    owner_call(0x480, STUB_SET_DESTINATION, 8,
               lambda e: event('set_destination', e.arg(0), e.arg(1) & 0xFF))
    owner_call(0x484, STUB_IDLE, 8, lambda e: event('idle', e.arg(0) & 0xFF, e.arg(1) & 0xFF))

    def anim(e):
        kind = 'warp' if e.arg(0) == WARP_ANIM_TYPE else hex(e.arg(0))
        event('anim', kind, read_coord(e, e.arg(1)), e.arg(2), e.arg(3), e.arg(4))
        return e.uc.reg_read(UC_X86_REG_ECX)

    def sound(e):
        event('sound', e.uc.reg_read(UC_X86_REG_ECX),
              read_coord(e, e.uc.reg_read(UC_X86_REG_EDX)))
        return 0

    def update_position(e):
        if e.uc.reg_read(UC_X86_REG_ECX) != TELEPORT:
            raise OracleError('Update_Position on an unexpected object')
        coord = [i32(e.arg(0)), i32(e.arg(1)), i32(e.arg(2))]
        place = e.arg(3) & 0xFF
        event('update_position', coord, place)
        if place:
            write_coord(e, TELEPORT + 0x28, coord)
            return 1
        if pending_blocks[0]:
            pending_blocks[0] -= 1
            write_coord(e, OWNER + 0x288, [coord[0] + 256, coord[1], coord[2]])
            return 0
        return 1

    def validation(e):
        event('post_warp_validation', [i32(e.arg(0)), i32(e.arg(1)), i32(e.arg(2))])

    emu.hook(0x421EA0, anim, 0x1C)
    emu.hook(0x7509E0, sound, 4)
    emu.hook(0x718260, update_position, 0x10)
    emu.hook(0x578460, lambda _e: 1, 8)
    emu.hook(0x7187A0, validation, 0xC)
    emu.hook(0x70C610, lambda e: event('archive_target', e.arg(0)), 4)
    emu.hook(0x70F770, lambda _e: event('shorten_scan'), 0)

    def acquire(_e):
        event('passive_acquire')
        return 0

    emu.hook(0x709480, acquire, 0)

    def snapshot():
        return [emu.read_i32(TELEPORT + 0x38), read8(emu, OWNER + 0x270),
                read8(emu, OWNER + 0x271), read8(emu, OWNER + 0x27C),
                emu.read_i32(TELEPORT + 0x3C), emu.read_i32(TELEPORT + 0x44),
                emu.read_i32(OWNER + 0x284)]

    def process(caller):
        before = emu.read_i32(TELEPORT + 0x38)
        emu.events = []
        emu.invoke(0x7192F0, args=[TELEPORT + 4])
        return [caller, before, emu.read_i32(TELEPORT + 0x38), emu.events]

    rows = []
    last = None
    ended = None
    for frame in range(start, start + frames):
        emu.write32(FRAME, frame)
        calls = []
        if read8(emu, OWNER + 0x271) or (read8(emu, OWNER + 0x270)
                                        and read8(emu, OWNER + 0x27C)):
            calls.append(process('prologue'))
        if not read8(emu, OWNER + 0x270):
            calls.append(process('foot'))
            if emu.invoke(0x719F30, args=[TELEPORT + 0x18]) & 0xFF:
                ended = frame - start
        state = snapshot()
        timer_start = state[4] - start if state[4] != -1 else -1
        state = state[:4] + [timer_start] + state[5:]
        calls = [call for call in calls if call[1] != call[2] or call[3]]
        if calls or state != last or ended is not None:
            rows.append(dict(frame=frame - start, calls=calls, state=state[0],
                             warped_out=state[1], warping_in=state[2], latched=state[3],
                             timer=[state[4], state[5]], delay=state[6]))
        last = state
        if ended is not None:
            break
    if ended is None:
        raise OracleError('the warp did not end')
    return dict(blocks=blocks, chrono_delay=chrono_delay, stale_delay=stale_delay,
                ended=ended, frames=rows)


def chrono_process():
    return [chrono_process_row(),
            chrono_process_row(blocks=1),
            chrono_process_row(blocks=2),
            chrono_process_row(stale_delay=60),
            chrono_process_row(blocks=1, chrono_delay=0)]


# ---------------------------------------------------------------- chrono_update_position

CHRONO_CELLS = CHRONO + 0x10000
CHRONO_CELL_VT = CHRONO + 0x8000
CHRONO_OBJECTS = CHRONO + 0x20000
CHRONO_OBJECT_VT = CHRONO + 0x9000
CHRONO_OBJECT_TYPES = CHRONO + 0x30000
C4_WARHEAD = CHRONO + 0xA000
STUB_CELL_COORDS = STUBS + 0x390
STUB_OBJ_CURTAIN = STUBS + 0x3A0
STUB_OBJ_WHAT = STUBS + 0x3B0
STUB_OBJ_COORDS = STUBS + 0x3C0
STUB_OBJ_TYPE = STUBS + 0x3D0
STUB_OBJ_DAMAGE = STUBS + 0x3E0
STUB_PUT = STUBS + 0x3F0
STUB_REMOVE = STUBS + 0x400
BRIDGE_HEIGHT = 416  # 0xB0EC2C as its initializer 0x717F60 leaves it: four 104-lepton levels


def cell_of(coord):
    return (coord[0] // 256, coord[1] // 256)



class ChronoMap:
    """MapClass::operator[] by coordinate 0x565730 and by cell 0x5657A0,
    answering one fixture CellClass per cell: MapCoords (+0x24), no tube
    (+0x44 = -1), the row's flags (+0x140), the ground and bridge object
    lists (+0xE4, +0xE8) and GetCoords vt+0x48 (the centre raised 104 leptons
    per level)."""

    def __init__(self, emu, cells, heads=None):
        self.emu = emu
        self.cells = cells
        self.heads = heads or {}
        self.addresses = {}
        emu.write32(CHRONO_CELL_VT + 0x48, STUB_CELL_COORDS)
        emu.hook(0x565730, lambda e: self.address(cell_of(read_coord(e, e.arg(0)))), 4)
        emu.hook(0x5657A0, lambda e: self.address(read_cell(e, e.arg(0))), 4)
        emu.hook(STUB_CELL_COORDS, self.coords, 4)

    def address(self, cell):
        cell = tuple(cell)
        if cell not in self.addresses:
            emu = self.emu
            this = CHRONO_CELLS + 0x200 * len(self.addresses)
            facts = self.cells.get(cell, {})
            emu.write32(this, CHRONO_CELL_VT)
            emu.uc.mem_write(this + 0x24, struct.pack('<hh', *cell))
            emu.write32(this + 0x44, -1)
            emu.write32(this + 0x140, facts.get('flags', 0))
            emu.write32(this + 0xE4, self.heads.get((cell, 'ground', 'first'), 0))
            emu.write32(this + 0xE8, self.heads.get((cell, 'bridge', 'first'), 0))
            self.addresses[cell] = this
        return self.addresses[cell]

    def coords(self, emu):
        this = emu.uc.reg_read(UC_X86_REG_ECX)
        cell = next(cell for cell, address in self.addresses.items() if address == this)
        level = self.cells.get(cell, {}).get('level', 0)
        out = emu.arg(0)
        write_coord(emu, out, [cell[0] * 256 + 128, cell[1] * 256 + 128,
                               level * LEVEL_LEPTONS])
        return out


def chrono_cells(cells):
    return [[x, y, facts.get('level', 0), facts.get('flags', 0)]
            for (x, y), facts in sorted(cells.items())]


def update_position_row(*, place, coord, owner='unit', owner_coord=SOURCE_COORD,
                        marked=None, on_bridge=False, mz=0, cells=None, objects=(),
                        found=(41, 39), floor=0):
    """Update_Position 0x718260 on a fixture map. `cells` maps a cell to its
    level and flags; `objects` lists (name, cell, list, what, foot, curtained,
    coords, strength) in each cell list's order; `floor` answers the cell
    floor height 0x578080."""
    cells = dict(cells or {})
    emu = Emu()
    emu.write32(FRAME, 4000)
    emu.write32(0xB0EC2C, BRIDGE_HEIGHT)
    emu.invoke(0x718000, ecx=TELEPORT)
    emu.invoke(0x55A710, args=[TELEPORT + 4, OWNER])
    if marked is not None:
        write_coord(emu, TELEPORT + 0x28, marked)
    emu.write32(OWNER, OWNER_VT)
    write8(emu, OWNER + 0x8C, on_bridge)
    write_coord(emu, OWNER + 0x9C, owner_coord)
    write_coord(emu, OWNER + 0x288, coord)
    emu.write32(OWNER_TYPE + 0x5B4, mz)
    emu.write32(OWNER_TYPE + 0xA0, 300)
    emu.write32(RULES + 0xFA8, C4_WARHEAD)
    names = {OWNER: 'owner'}
    facts = {OWNER: dict(what=0xF if owner == 'infantry' else 1, coords=list(owner_coord),
                         curtained=False, type=OWNER_TYPE)}
    heads = {}
    for index, (name, cell, layer, what, foot, curtained, at, strength) in enumerate(objects):
        this = CHRONO_OBJECTS + 0x400 * index
        kind = CHRONO_OBJECT_TYPES + 0x100 * index
        emu.write32(this, CHRONO_OBJECT_VT)
        write8(emu, this + 0x14, 0x4 if foot else 0)
        emu.write32(this + 0x30, 0)
        emu.write32(kind + 0xA0, strength)
        names[this] = name
        facts[this] = dict(what=what, coords=list(at), curtained=curtained, type=kind)
        key = (tuple(cell), layer)
        if key in heads:
            emu.write32(heads[key] + 0x30, this)
        else:
            heads[(tuple(cell), layer, 'first')] = this
        heads[key] = this
    ChronoMap(emu, cells, heads)

    def fact(e, key):
        return facts[e.uc.reg_read(UC_X86_REG_ECX)][key]

    def obj_coords(e):
        out = e.arg(0)
        write_coord(e, out, fact(e, 'coords'))
        return out

    def damage(e):
        damage_value = e.read_i32(e.arg(0))
        e.events.append(['damage', names[e.uc.reg_read(UC_X86_REG_ECX)], damage_value,
                         i32(e.arg(1)), 'C4' if e.arg(2) == C4_WARHEAD else hex(e.arg(2)),
                         e.arg(3), e.arg(4) & 0xFF, e.arg(5) & 0xFF, e.arg(6)])
        return 0

    for vtable in (OWNER_VT, CHRONO_OBJECT_VT):
        emu.write32(vtable + 0x160, STUB_OBJ_CURTAIN)
        emu.write32(vtable + 0x2C, STUB_OBJ_WHAT)
        emu.write32(vtable + 0x48, STUB_OBJ_COORDS)
        emu.write32(vtable + 0x84, STUB_OBJ_TYPE)
        emu.write32(vtable + 0x16C, STUB_OBJ_DAMAGE)
    emu.write32(OWNER_VT + 0xF0, STUB_PUT)
    emu.write32(OWNER_VT + 0xF4, STUB_REMOVE)
    emu.hook(STUB_OBJ_CURTAIN, lambda e: int(fact(e, 'curtained')), 0)
    emu.hook(STUB_OBJ_WHAT, lambda e: fact(e, 'what'), 0)
    emu.hook(STUB_OBJ_COORDS, obj_coords, 4)
    emu.hook(STUB_OBJ_TYPE, lambda e: fact(e, 'type'), 0)
    emu.hook(STUB_OBJ_DAMAGE, damage, 0x1C)
    emu.hook(STUB_PUT, lambda e: e.events.append(['put', read_coord(e, e.arg(0))]), 4)
    emu.hook(STUB_REMOVE, lambda e: e.events.append(['remove', read_coord(e, e.arg(0))]), 4)

    def floor_height(e):
        e.events.append(['floor', read_coord(e, e.arg(0))])
        return floor

    def zone(e):
        e.events.append(['zone', read_cell(e, e.arg(0)), e.arg(1), e.arg(2) & 0xFF])
        return 7

    def nearby(e):
        args = [e.arg(index) for index in range(15)]
        e.events.append(['nearby', read_cell(e, args[1]), args[2], args[3], args[4],
                         args[5] & 0xFF, args[6], args[7], args[8] & 0xFF, args[9] & 0xFF,
                         args[10] & 0xFF, args[11] & 0xFF, read_cell(e, args[12]),
                         args[13] & 0xFF, args[14] & 0xFF])
        e.uc.mem_write(args[0], struct.pack('<hh', *found))
        return args[0]

    emu.hook(0x578080, floor_height, 4)
    emu.hook(0x56D230, zone, 0xC)
    emu.hook(0x56DC20, nearby, 0x3C)
    result = emu.invoke(0x718260, ecx=TELEPORT, args=[*map(u32_value, coord), int(place)]) & 0xFF
    return dict(place=place, coord=list(coord), owner=owner, owner_coord=list(owner_coord),
                marked=None if marked is None else list(marked), on_bridge=on_bridge, mz=mz,
                cells=chrono_cells(cells),
                objects=[[name, list(cell), layer, what, foot, curtained, list(at), strength]
                         for name, cell, layer, what, foot, curtained, at, strength in objects],
                found=list(found), floor=floor, result=result, events=emu.events,
                marked_after=read_coord(emu, TELEPORT + 0x28),
                destination_after=read_coord(emu, OWNER + 0x288),
                on_bridge_after=read8(emu, OWNER + 0x8C))


def u32_value(value):
    return value & 0xFFFFFFFF


def update_position():
    """Rows over a flat map around DEST_COORD's cell (40, 40). Each blocked
    row's `found` is the cell VERA's Find_Nearby_Passable_Cell port picks in
    the same world (chronosphere_tests rebuilds it); the native search
    itself is a stub here. `spot` is where VERA's spawn puts an
    infantryman in the cell."""
    dest = list(DEST_COORD)
    sub = [dest[0] + 30, dest[1] - 20, 215]
    spot = [dest[0] + 64, dest[1] - 64, 0]
    south = [40 * 256 + 128, 42 * 256 + 128, 0]

    def unit(name, cell=(40, 40), at=None, curtained=False, layer='ground'):
        return (name, cell, layer, 1, True, curtained, at or dest, 400)

    def infantry(name, at, cell=(40, 40)):
        return (name, cell, 'ground', 0xF, True, False, at, 125)

    def building(cell, at):
        return ('GAPOWR', cell, 'ground', 6, False, False, at, 750)

    rows = [
        # Placing: Marked empty, then set; the bridge height enters only
        # when the owner was not already on the bridge.
        update_position_row(place=True, coord=sub, floor=208, cells={(40, 40): {'level': 2}}),
        update_position_row(place=True, coord=sub, marked=[100, 200, 0], floor=208,
                            cells={(40, 40): {'level': 2, 'flags': 0x100}}),
        update_position_row(place=True, coord=sub, marked=[100, 200, 0], floor=208,
                            on_bridge=True, cells={(40, 40): {'level': 2, 'flags': 0x100}}),
        update_position_row(place=True, coord=sub, floor=0, on_bridge=True),
        # Testing: an empty cell; Foot victims; the Iron Curtain; a building.
        update_position_row(place=False, coord=dest),
        update_position_row(place=False, coord=dest, marked=[100, 200, 0]),
        update_position_row(place=False, coord=dest, objects=[unit('HTNK')]),
        update_position_row(place=False, coord=dest, objects=[unit('HTNK', curtained=True)]),
        update_position_row(place=False, coord=dest,
                            objects=[unit('HTNK', curtained=True), unit('MTNK')]),
        update_position_row(place=False, coord=dest, objects=[building((40, 40), dest)],
                            found=(40, 39)),
        # Blocked away from the cell centre, onto a raised cell: the found
        # cell keeps the offset from the blocked cell's coordinate.
        update_position_row(place=False, coord=[south[0] + 30, south[1] - 20, 0],
                            objects=[building((40, 42), south)], found=(40, 41),
                            cells={(40, 41): {'level': 1}}),
        update_position_row(place=False, coord=[south[0] + 30, south[1] - 20, 215],
                            objects=[building((40, 42), south)], found=(41, 41),
                            cells={(40, 42): {'level': 2}, (41, 39): {'level': 1}}),
        # Infantry: killed only at the warping infantryman's exact coordinate.
        update_position_row(place=False, coord=spot, owner='infantry',
                            objects=[infantry('E1', spot)]),
        update_position_row(place=False, coord=dest, owner='infantry',
                            objects=[infantry('E1', spot)]),
        update_position_row(place=False, coord=dest, owner='infantry',
                            objects=[infantry('E1', spot), unit('HTNK')]),
        update_position_row(place=False, coord=dest, objects=[infantry('E1', spot)]),
        # Bridges: no deck flag blocks; the bridge list is walked instead.
        update_position_row(place=False, coord=dest, cells={(40, 40): {'flags': 0x100}}),
        update_position_row(place=False, coord=dest, cells={(40, 40): {'flags': 0x300}},
                            objects=[unit('HTNK', layer='bridge'), unit('MTNK')]),
    ]
    # The MovementZone handed to the nearby-cell search.
    for mz in range(1, 13):
        rows.append(update_position_row(place=False, coord=dest, mz=mz,
                                        objects=[building((40, 40), dest)]))
    return rows


# ---------------------------------------------------------------- chrono_destination

CASE4_OFFSET = CHRONO + 0xB000
CASE4_PCELL = CHRONO + 0xB010
CASE4_SUPER = CHRONO + 0xB100
CASE4_OBJECT = CHRONO + 0xC000
CASE4_OBJECT_VT = CHRONO + 0xD000
STUB_CASE4_WHAT = STUBS + 0x410
STUB_CASE4_COORDS = STUBS + 0x420


def chrono_destination_row(*, what, offset, coords, src=(21, 21), target=(40, 40),
                           cells=None):
    """Launch case 4's `+0x288` for one object of the source block: the
    Unit's (0x6CC9AF..0x6CCA4C) and then the others' (0x6CCB6A..0x6CCC2D),
    run as slices on Launch's frame (the offset entry at [esp+0x2C], the
    clicked cell at [esp+0x1E8], the Super at [esp+0x44]). The bridge height
    0xB0C07C holds 416, as its initializer 0x6CAD80 leaves it."""
    cells = dict(cells or {})
    emu = Emu()
    emu.write32(0xB0C07C, BRIDGE_HEIGHT)
    ChronoMap(emu, cells)
    emu.uc.mem_write(CASE4_OFFSET, struct.pack('<hh', *offset))
    emu.uc.mem_write(CASE4_PCELL, struct.pack('<hh', *target))
    emu.uc.mem_write(CASE4_SUPER + 0x62, struct.pack('<hh', *src))
    emu.write32(CASE4_OBJECT, CASE4_OBJECT_VT)
    emu.write32(CASE4_OBJECT_VT + 0x2C, STUB_CASE4_WHAT)
    emu.write32(CASE4_OBJECT_VT + 0x48, STUB_CASE4_COORDS)
    emu.hook(STUB_CASE4_WHAT, lambda _e: what, 0)
    emu.hook(STUB_CASE4_COORDS, coords_stub(coords), 4)
    uc = emu.uc
    sp = STACK_BASE + STACK_SIZE - 0x1000
    emu.write32(sp + 0x2C, CASE4_OFFSET)
    emu.write32(sp + 0x44, CASE4_SUPER)
    emu.write32(sp + 0x1E8, CASE4_PCELL)
    uc.reg_write(UC_X86_REG_ESP, sp)
    uc.reg_write(UC_X86_REG_ESI, CASE4_OBJECT)
    uc.reg_write(UC_X86_REG_EBP, 0)
    uc.reg_write(UC_X86_REG_FPCW, NATIVE_FPCW)
    run_checked(uc, 0x6CC9AF, 0x6CCA4C, count=10_000)
    run_checked(uc, 0x6CCB6A, 0x6CCC2D, count=10_000)
    return dict(what=what, offset=list(offset), coords=list(coords), src=list(src),
                target=list(target), cells=chrono_cells(cells),
                destination=read_coord(emu, sp + 0x10))


def chrono_destination():
    """Objects of a source block at (21, 21) warped to (40, 40); `spot` is
    where VERA's spawn puts an infantryman in a cell."""
    def centre(cell, z=0):
        return [cell[0] * 256 + 128, cell[1] * 256 + 128, z]

    def spot(cell, z=0):
        x, y, _ = centre(cell)
        return [x + 64, y - 64, z]

    unit, infantry = 1, 0xF
    return [
        chrono_destination_row(what=unit, offset=(0, 0), coords=centre((21, 21))),
        chrono_destination_row(what=unit, offset=(-1, -1), coords=centre((20, 20)),
                               cells={(39, 39): {'level': 1}}),
        chrono_destination_row(what=unit, offset=(1, 0), coords=centre((22, 21)),
                               cells={(41, 40): {'flags': 0x100}}),
        chrono_destination_row(what=unit, offset=(0, 1), coords=centre((21, 22), 104),
                               cells={(21, 22): {'level': 1}, (40, 41): {'level': 2}}),
        chrono_destination_row(what=infantry, offset=(0, 0), coords=spot((21, 21)),
                               cells={(40, 40): {'level': 2}}),
        chrono_destination_row(what=infantry, offset=(-1, 0), coords=spot((20, 21), 104),
                               cells={(20, 21): {'level': 1}}),
        chrono_destination_row(what=infantry, offset=(-1, 0), coords=spot((20, 21)),
                               cells={(21, 21): {'level': 3}, (40, 40): {'level': 4}}),
        chrono_destination_row(what=infantry, offset=(1, 1), coords=spot((22, 22)),
                               cells={(41, 41): {'flags': 0x100}}),
    ]

# ---------------------------------------------------------------- psychic dominator

# The Psychic Dominator's globals (SuperWeaponEffects, reset by 0x539740 and
# saved by 0x539890): its cell, status, anim and owner.
G_PSYDOM_COORDS = 0xA9FA48
G_PSYDOM_STATUS = 0xA9FAC0
G_PSYDOM_ANIM = 0xA9FAC4
G_PSYDOM_OWNER = 0xA9FAC8
G_NUKE_FLASH = 0xA9FABC
G_CHRONO_SCREEN = 0xA9FAB0
G_STORM_ACTIVE = 0xA9FAB4
PSYDOM = BASE + 0x380000
PSYDOM_ANIM = PSYDOM
PSYDOM_ANIM_TYPE = PSYDOM + 0x1000
PSYDOM_ANIM_TYPE_VT = PSYDOM + 0x2000
PSYDOM_IMAGE = PSYDOM + 0x3000
PSYDOM_FIRST_ANIM = PSYDOM + 0x4000
PSYDOM_SECOND_ANIM = PSYDOM + 0x5000
PSYDOM_HOUSE = PSYDOM + 0x6000
STUB_PSYDOM_IMAGE = STUBS + 0x500
# ScenarioClass fields: Timer_1248 (start, pad, duration), the ambient
# target +0x3530 and current +0x352C, the profiles and the two fade rates.
SCN_TIMER = 0x1248
SCN_TARGET = 0x3530
SCN_CURRENT = 0x352C
SCN_CELL_REDRAW = 0x34AB
SCN_PROFILES = {'ambient': 0x3528, 'ion': (0x3548, 0x354C, 0x3550, 0x3554),
                'nuke': (0x3560, 0x3564, 0x3568, 0x356C),
                'dominator': (0x357C, 0x3580, 0x3584, 0x3588)}
SCN_NUKE_RATE = 0x3578
SCN_DOMINATOR_RATE = 0x3594


def psydom_process_emu():
    """One emulator for many PsychicDominator::Process 0x53AF40 calls: its
    anim's type answers GetImage (vt+0x9C) with a fixture image whose frame
    count (+6) each step writes; MindControlArea 0x53B080 and UpdateLighting
    0x53C280 are recorded stubs."""
    emu = Emu()
    emu.write32(PSYDOM_ANIM + 0xC8, PSYDOM_ANIM_TYPE)
    emu.write32(PSYDOM_ANIM_TYPE, PSYDOM_ANIM_TYPE_VT)
    emu.write32(PSYDOM_ANIM_TYPE_VT + 0x9C, STUB_PSYDOM_IMAGE)

    def image(e):
        if e.uc.reg_read(UC_X86_REG_ECX) != PSYDOM_ANIM_TYPE:
            raise OracleError('GetImage on an unexpected type')
        return PSYDOM_IMAGE

    emu.hook(STUB_PSYDOM_IMAGE, image, 0)
    emu.hook(0x53B080, lambda e: e.events.append(['mind_control_area']), 0)
    emu.hook(0x53C280, lambda e: e.events.append(['update_lighting']), 0)
    return emu


def psydom_process_step(emu, *, status, stage=0, frames=0, percent=50, ambient=(100, 100)):
    emu.events = []
    emu.write32(G_PSYDOM_STATUS, status)
    emu.write32(G_PSYDOM_ANIM, PSYDOM_ANIM)
    emu.uc.mem_write(G_PSYDOM_COORDS, struct.pack('<hh', 33, 44))
    emu.write32(PSYDOM_ANIM + 0xAC, stage)
    emu.uc.mem_write(PSYDOM_IMAGE + 6, struct.pack('<h', frames))
    emu.write32(RULES + 0x304, percent)
    emu.write32(SCENARIO + SCN_TARGET, ambient[0])
    emu.write32(SCENARIO + SCN_CURRENT, ambient[1])
    emu.invoke(0x53AF40)
    return dict(status=status, stage=stage, frames=frames, percent=percent,
                ambient=list(ambient), status_after=emu.read_i32(G_PSYDOM_STATUS),
                anim_after=int(emu.read32(G_PSYDOM_ANIM) != 0),
                coords_after=read_cell(emu, G_PSYDOM_COORDS), events=list(emu.events))


PSYDOM_PERCENTS = list(range(0, 101)) + [-10, -1, 101, 150, 1000]
PSYDOM_FRAMES = list(range(1, 65))


def psydom_fire_stages():
    """Status 2's test (0x53AF64..0x53AFAA): for each DominatorFireAtPercentage
    (Rules+0x304) and anim frame count, the first stage (+0xAC) whose
    FILD/FIDIV ratio is at least FILD percent FMUL 0.01, searched in
    0..2*frames (the test is monotonic in the stage); None if none is."""
    emu = psydom_process_emu()
    rows = []
    for percent in PSYDOM_PERCENTS:
        firsts = []
        for frames in PSYDOM_FRAMES:
            def fires(stage):
                step = psydom_process_step(emu, status=2, stage=stage, frames=frames,
                                           percent=percent)
                fired = step['status_after'] == 3
                if fired != (step['events'] == [['mind_control_area']]):
                    raise OracleError('status 3 without MindControlArea')
                return fired
            low, high = 0, 2 * frames + 1
            while low < high:
                middle = (low + high) // 2
                if fires(middle):
                    high = middle
                else:
                    low = middle + 1
            firsts.append(low if low <= 2 * frames else None)
        rows.append([percent, firsts])
    return dict(frames=PSYDOM_FRAMES, first_stage=rows)


def psydom_process():
    """Single steps of every status, the status 2 rows at the retail
    PDFXCLD count (60 frames, 20 percent) and a zero frame count."""
    emu = psydom_process_emu()
    step = lambda **row: psydom_process_step(emu, **row)
    rows = [step(status=0), step(status=6), step(status=-1), step(status=1)]
    for stage in (11, 12, 13):
        rows.append(step(status=2, stage=stage, frames=60, percent=20))
    rows += [step(status=2, stage=0, frames=0, percent=20),
             step(status=2, stage=5, frames=0, percent=20)]
    for stage in (10, 11, 21, 30):
        rows.append(step(status=3, stage=stage, frames=21))
    for stage in (19, 20, 21, 30):
        rows.append(step(status=4, stage=stage, frames=21))
    rows += [step(status=5, ambient=(100, 100)), step(status=5, ambient=(100, 120)),
             step(status=5, ambient=(150, 140))]
    return dict(steps=rows, fire_stages=psydom_fire_stages())


def psydom_start_row(*, first=True, second=True, frame=4000, cell=(33, 44), level=0):
    """PsyDom::Start 0x53AE50 (ECX the house, the cell by value; RET 4) with
    Rules DominatorFirstAnim/SecondAnim (+0x2FC/+0x300) set or null: the
    globals, the anim constructor 0x421EA0's arguments (a recorded stub),
    Timer_1248 and UpdateLighting 0x53C280 (a recorded stub)."""
    emu = Emu()
    emu.write32(FRAME, frame)
    emu.write32(RULES + 0x2FC, PSYDOM_FIRST_ANIM if first else 0)
    emu.write32(RULES + 0x300, PSYDOM_SECOND_ANIM if second else 0)
    emu.write32(SCENARIO + SCN_TIMER, 77)
    emu.write32(SCENARIO + SCN_TIMER + 8, 55)
    Cells(emu, {tuple(cell): level})

    def anim(e):
        kind = {PSYDOM_FIRST_ANIM: 'first', PSYDOM_SECOND_ANIM: 'second'}.get(e.arg(0),
                                                                             hex(e.arg(0)))
        e.events.append(['anim', kind, read_coord(e, e.arg(1)), i32(e.arg(2)), i32(e.arg(3)),
                         e.arg(4), i32(e.arg(5)), e.arg(6) & 0xFF])
        return e.uc.reg_read(UC_X86_REG_ECX)

    emu.hook(0x421EA0, anim, 0x1C)
    emu.hook(0x53C280, lambda e: e.events.append(['update_lighting']), 0)
    packed = struct.unpack('<I', struct.pack('<hh', *cell))[0]
    emu.invoke(0x53AE50, ecx=PSYDOM_HOUSE, args=[packed])
    return dict(first=first, second=second, frame=frame, cell=list(cell), level=level,
                status=emu.read_i32(G_PSYDOM_STATUS),
                owner_set=int(emu.read32(G_PSYDOM_OWNER) == PSYDOM_HOUSE),
                anim_set=int(emu.read32(G_PSYDOM_ANIM) != 0),
                coords=read_cell(emu, G_PSYDOM_COORDS),
                timer=[emu.read_i32(SCENARIO + SCN_TIMER), emu.read_i32(SCENARIO + SCN_TIMER + 8)],
                events=emu.events)


def psydom_start():
    return [psydom_start_row(), psydom_start_row(level=2, cell=(70, 12), frame=9),
            psydom_start_row(first=False), psydom_start_row(second=False)]


def update_lighting_row(*, nuke=0, chrono=0, storm=False, psydom=0):
    """ScenarioClass::UpdateLighting 0x53C280: the ambient target it writes
    (+0x3530) and RecalcLighting 0x53AD00's arguments (ECX, EDX and two stack
    words; a recorded stub)."""
    emu = Emu()
    emu.write32(G_NUKE_FLASH, nuke)
    emu.write32(G_CHRONO_SCREEN, chrono)
    write8(emu, G_STORM_ACTIVE, storm)
    emu.write32(G_PSYDOM_STATUS, psydom)
    emu.write32(SCENARIO + SCN_PROFILES['ambient'], 101)
    for name, values in (('ion', (87, 30, 40, 75)), ('nuke', (200, 175, 150, 125)),
                         ('dominator', (150, 85, 20, 30))):
        for offset, value in zip(SCN_PROFILES[name], values):
            emu.write32(SCENARIO + offset, value)

    def recalc(e):
        e.events.append(['recalc', i32(e.uc.reg_read(UC_X86_REG_ECX)),
                         i32(e.uc.reg_read(UC_X86_REG_EDX)), i32(e.arg(0)), i32(e.arg(1))])

    emu.hook(0x53AD00, recalc, 8)
    emu.invoke(0x53C280)
    return dict(nuke=nuke, chrono=chrono, storm=storm, psydom=psydom,
                target=emu.read_i32(SCENARIO + SCN_TARGET), events=emu.events)


def update_lighting():
    return [update_lighting_row(nuke=nuke, chrono=chrono, storm=storm, psydom=psydom)
            for nuke in (0, 1, 2) for chrono in (0, 1) for storm in (False, True)
            for psydom in (0, 1, 2, 3, 4, 5, 6)]


def ambient_step_row(*, target=150, current=100, rate=0.2, step=0.2, frame=1000,
                     timer=(999, 1), nuke=0, chrono=0, psydom=0, nuke_rate=3,
                     dominator_rate=1):
    """LogicClass::PerTickUpdate's ambient fade (0x55B33D..0x55B4D7) as a
    slice (EBP the Scenario, EBX the Rules, EDI the frame): the gates, the
    interval each lighting state selects for Timer_1248, the target clamp and
    the clamped step. NukeFlash::IsFadingIn/Out 0x53A110/0x53A120,
    ChronoScreenEffect::Active 0x53BAD0 and PsyDom::Active 0x53B400 run
    natively on their globals; 0x4AE4C0 and 0x4F42F0 are recorded stubs."""
    emu = Emu()
    emu.write32(FRAME, frame)
    emu.write32(G_NUKE_FLASH, nuke)
    emu.write32(G_CHRONO_SCREEN, chrono)
    emu.write32(G_PSYDOM_STATUS, psydom)
    emu.uc.mem_write(RULES + 0x1668, struct.pack('<d', rate))
    emu.uc.mem_write(RULES + 0x1670, struct.pack('<d', step))
    emu.write32(SCENARIO + SCN_TIMER, timer[0])
    emu.write32(SCENARIO + SCN_TIMER + 8, timer[1])
    emu.write32(SCENARIO + SCN_TARGET, target)
    emu.write32(SCENARIO + SCN_CURRENT, current)
    emu.write32(SCENARIO + SCN_NUKE_RATE, nuke_rate)
    emu.write32(SCENARIO + SCN_DOMINATOR_RATE, dominator_rate)
    write8(emu, SCENARIO + SCN_CELL_REDRAW, 0)
    emu.hook(0x4AE4C0, lambda e: e.events.append(['cell_lighting']), 0)
    emu.hook(0x4F42F0, lambda e: e.events.append(['redraw', e.arg(0)]), 4)
    uc = emu.uc
    sp = STACK_BASE + STACK_SIZE - 0x1000
    uc.reg_write(UC_X86_REG_ESP, sp)
    uc.reg_write(UC_X86_REG_EBP, SCENARIO)
    uc.reg_write(UC_X86_REG_EBX, RULES)
    uc.reg_write(UC_X86_REG_EDI, frame)
    uc.reg_write(UC_X86_REG_FPCW, NATIVE_FPCW)
    run_checked(uc, 0x55B33D, 0x55B4D7, count=10_000)
    return dict(target=target, current=current, rate=rate, step=step, frame=frame,
                timer=list(timer), nuke=nuke, chrono=chrono, psydom=psydom,
                nuke_rate=nuke_rate, dominator_rate=dominator_rate,
                target_after=emu.read_i32(SCENARIO + SCN_TARGET),
                current_after=emu.read_i32(SCENARIO + SCN_CURRENT),
                timer_after=[emu.read_i32(SCENARIO + SCN_TIMER),
                             emu.read_i32(SCENARIO + SCN_TIMER + 8)],
                cell_redraw=read8(emu, SCENARIO + SCN_CELL_REDRAW), events=emu.events)


def ambient_step():
    row = ambient_step_row
    rows = [row(), row(psydom=1), row(psydom=4, dominator_rate=7), row(psydom=5, target=100,
                                                                       current=150),
            row(nuke=1), row(nuke=2), row(chrono=1), row(nuke=1, psydom=2),
            row(timer=(995, 6)), row(timer=(994, 6)), row(timer=(-1, 0)), row(timer=(-1, 3)),
            row(target=100), row(rate=0.0), row(target=-5, current=10), row(target=105),
            row(target=95, current=100), row(step=0.07), row(step=0.29), row(step=0.57),
            row(rate=0.01), row(rate=0.0011), row(psydom=3, dominator_rate=0),
            row(psydom=3, dominator_rate=-4)]
    return rows


# ScenarioClass::Read_INI_Basic 0x689E90's Dominator keys: (key, Scenario
# offset, the default-inverse slice, the conversion slice). Each default slice
# ends after FSTP double [ESP]; each conversion slice starts at the FMUL after
# ReadDouble returns and ends after Math__ftol.
DOMINATOR_LIGHTING_SITES = (
    ('DominatorAmbient', 0x357C, (0x68AAFD, 0x68AB17), (0x68AB26, 0x68AB37)),
    ('DominatorRed', 0x3580, (0x68AB37, 0x68AB51), (0x68AB60, 0x68AB71)),
    ('DominatorGreen', 0x3584, (0x68AB71, 0x68AB8B), (0x68AB9A, 0x68ABAB)),
    ('DominatorBlue', 0x3588, (0x68ABAB, 0x68ABC5), (0x68ABD4, 0x68ABE5)),
    ('DominatorGround', 0x358C, (0x68ABE5, 0x68ABFF), (0x68AC0E, 0x68AC1F)),
    ('DominatorLevel', 0x3590, (0x68AC1F, 0x68AC39), (0x68AC48, 0x68AC59)),
    ('DominatorAmbientChangeRate', 0x3594, (0x68AC59, 0x68AC73), (0x68AC82, 0x68AC93)),
)
DOMINATOR_PERCENT_TOKENS = ('0', '1.5', '.85', '.2', '.3', '1', '.01', '.009', '.0099',
                            '1.99999', '-.2', '.155', '2.5')
DOMINATOR_MILLI_TOKENS = ('0', '.001', '.0015', '.0009', '.002', '.05', '-.001', '.0319',
                          '1.99999', '.000989')
TRAMPOLINE = STUBS + 0x600
TRAMPOLINE_VALUE = STUBS + 0x680


def dominator_lighting_read():
    """The map's Dominator lighting. ScenarioClass::Set_Defaults 0x683610's
    block 0x683915..0x6839BD (EBP the Scenario, EBX zero as at 0x68365A, EAX
    100 as at 0x6838C2) writes the defaults. For each key, Read_INI_Basic's
    default slice turns the stored value into ReadDouble's default (its double
    at [ESP]), and its conversion slice turns ReadDouble's answer into the
    stored value: a trampoline loads the answer into ST0 (FLD qword) and jumps
    to the slice's FMUL. The answers are the defaults (a missing key) and
    each token's float scan widened to double."""
    emu = Emu()
    uc = emu.uc
    sp = STACK_BASE + STACK_SIZE - 0x1000
    uc.reg_write(UC_X86_REG_FPCW, NATIVE_FPCW)
    uc.reg_write(UC_X86_REG_ESP, sp)
    uc.reg_write(UC_X86_REG_EBP, SCENARIO)
    uc.reg_write(UC_X86_REG_EBX, 0)
    uc.reg_write(UC_X86_REG_EAX, 100)
    run_checked(uc, 0x683915, 0x6839BD, count=100)
    defaults = {name: emu.read_i32(SCENARIO + offset)
                for name, offset, _default, _convert in DOMINATOR_LIGHTING_SITES}

    def convert(value, start, end):
        uc.mem_write(TRAMPOLINE_VALUE, struct.pack('<d', value))
        jump = start - (TRAMPOLINE + 11)
        uc.mem_write(TRAMPOLINE, b'\xdd\x05' + u32(TRAMPOLINE_VALUE) + b'\xe9'
                     + struct.pack('<i', jump))
        uc.reg_write(UC_X86_REG_ESP, sp)
        uc.reg_write(UC_X86_REG_FPCW, NATIVE_FPCW)
        run_checked(uc, TRAMPOLINE, end, count=500)
        return i32(uc.reg_read(UC_X86_REG_EAX))

    rows = []
    for name, offset, (default_start, default_end), (start, end) in DOMINATOR_LIGHTING_SITES:
        uc.reg_write(UC_X86_REG_ESP, sp)
        uc.reg_write(UC_X86_REG_ESI, SCENARIO)
        uc.reg_write(UC_X86_REG_EDI, 0)
        uc.reg_write(UC_X86_REG_FPCW, NATIVE_FPCW)
        run_checked(uc, default_start, default_end, count=50)
        default = struct.unpack('<d', uc.mem_read(uc.reg_read(UC_X86_REG_ESP), 8))[0]
        tokens = DOMINATOR_MILLI_TOKENS if offset >= 0x358C else DOMINATOR_PERCENT_TOKENS
        authored = []
        for token in tokens:
            value = struct.unpack('<f', struct.pack('<f', float(token)))[0]
            authored.append([token, convert(value, start, end)])
        rows.append(dict(key=name, offset=offset, stored_default=defaults[name],
                         default_double=default,
                         default_units=convert(default, start, end), authored=authored))
    return rows


# Map-authored Ground/Level stand-ins (Scenario offset, value), distinct per
# profile so each arm's reads show; NukeGround/NukeLevel (+0x3570/+0x3574)
# keep the values Set_Defaults writes, which no INI key changes.
RELIGHT_GROUND_LEVEL = ((0x3540, 21), (0x3544, 13), (0x3558, 30), (0x355C, 40),
                        (0x358C, 3), (0x3590, 7))
RELIGHT_CELL = CELL
RELIGHT_SCALARS = CELL + 0x800


def relight_row(*, storm=False, psydom=0, nuke=0, level=0, ambient=1000):
    """CellClass::ProcessColourComponents 0x484180's profile arms as a slice
    (0x48445F..0x4845A2): the gathered additive ([ESP+0x44], zero here: no
    light reaches the cell) joins the ambient the top holds (written at
    0x4841DE), both scalars start from the sum, then LightningStorm::IsActive 0x53A100, PsyDom::Active 0x53B400 and
    NukeFlash::IsFadingIn 0x53A110 pick the Ground/Level each scalar adds for
    the cell's level (+0x11B). The Scenario holds Set_Defaults' block
    (0x683915..0x6839BD) with RELIGHT_GROUND_LEVEL over it."""
    emu = Emu()
    uc = emu.uc
    sp = STACK_BASE + STACK_SIZE - 0x1000
    uc.reg_write(UC_X86_REG_FPCW, NATIVE_FPCW)
    uc.reg_write(UC_X86_REG_ESP, sp)
    uc.reg_write(UC_X86_REG_EBP, SCENARIO)
    uc.reg_write(UC_X86_REG_EBX, 0)
    uc.reg_write(UC_X86_REG_EAX, 100)
    run_checked(uc, 0x683915, 0x6839BD, count=100)
    for offset, value in RELIGHT_GROUND_LEVEL:
        emu.write32(SCENARIO + offset, value)
    write8(emu, G_STORM_ACTIVE, storm)
    emu.write32(G_PSYDOM_STATUS, psydom)
    emu.write32(G_NUKE_FLASH, nuke)
    write8(emu, RELIGHT_CELL + 0x11B, level & 0xFF)
    top, bottom, additive = RELIGHT_SCALARS, RELIGHT_SCALARS + 4, RELIGHT_SCALARS + 8
    emu.write32(top, ambient)
    emu.write32(bottom, 0)
    emu.write32(additive, 0)
    emu.write32(sp + 0x44, additive)
    emu.write32(sp + 0x50, bottom)
    uc.reg_write(UC_X86_REG_ESP, sp)
    uc.reg_write(UC_X86_REG_EBX, top)
    uc.reg_write(UC_X86_REG_EDI, RELIGHT_CELL)
    run_checked(uc, 0x48445F, 0x4845A2, count=200)
    return dict(storm=storm, psydom=psydom, nuke=nuke, level=level, ambient=ambient,
                nuke_ground=emu.read_i32(SCENARIO + 0x3570),
                nuke_level=emu.read_i32(SCENARIO + 0x3574), top=emu.read_i32(top),
                bottom=emu.read_i32(bottom))


def relight():
    rows = [relight_row(psydom=psydom, level=level)
            for psydom in (0, 1, 2, 3, 4, 5) for level in (0, 2, 7)]
    rows += [relight_row(storm=True, psydom=3, level=2),
             relight_row(nuke=1, level=2), relight_row(nuke=1, psydom=3, level=2),
             relight_row(psydom=3, level=4, ambient=1500)]
    return rows


def generate():
    return {'source': 'unicorn/gamemd.exe', 'click_fire': click_fire(),
            'defense_alert': defense_alert(), 'mission_missile': mission_missile(),
            'nuke_maker': nuke_maker(), 'super_anim': super_anim(),
            'opening_super_anim': opening_super_anim(),
            'ai_try_fire': try_fire(), 'ai_best_rally_target': best_rally_target(),
            'ai_ground_rally_point': ground_rally_point(),
            'ai_genetic_mutator': gen_mutator(),
            'chrono_process': chrono_process(),
            'chrono_update_position': update_position(),
            'chrono_destination': chrono_destination(),
            'psydom_process': psydom_process(),
            'psydom_start': psydom_start(),
            'update_lighting': update_lighting(),
            'ambient_step': ambient_step(),
            'dominator_lighting_read': dominator_lighting_read(),
            'relight': relight(),
            'ai_catalog': {'types': [[name, what, keys] for name, what, keys in TYPE_CATALOG],
                           'build_const': BUILD_CONST_TYPES, 'build_tech': BUILD_TECH_TYPES,
                           'playfield': PLAYFIELD}}


if __name__ == '__main__':
    here = Path(__file__)
    finish_vectors(generate, here.with_suffix('.json'), source_paths={
        'superweapon_oracle': here, 'ai_base_building_oracle':
            here.with_name('ai_base_building_oracle.py')},
        provenance=lambda: provenance(
        scope=('nuclear missile launch chain: ClickFire admission/refusal/recharge writes, '
               'the computer launch alert (distance, draw, stores), Mission_Missile by '
               'status with its anims, bullet and velocity bits, NukeMaker\'s payload '
               'bullet and velocity bits, and both SuperAnim blocks; the computer\'s '
               'superweapon use: AI_TryFireSW\'s gates, arms and Force Shield timing, '
               'AI_FindBestRallyTarget\'s values, draws and pick, AI_GroundRallyPoint\'s '
               'cell and AI_Fire_GenMutator\'s count and pick; the Chrono Warp\'s Teleport '
               'states, owner bytes, timers and end frame, unblocked, blocked once or '
               'twice and with a stale ChronoDelay; Update_Position\'s placement, '
               'kills, blocks and blocked retarget; Launch case 4\'s destinations; the '
               'Psychic Dominator\'s ClickFire refusal, Process statuses and firing '
               'stages, Start\'s writes, UpdateLighting\'s targets and RecalcLighting '
               'arguments, the ambient fade\'s intervals, clamp and step, the map\'s '
               'Dominator lighting defaults and conversions, and the Ground/Level a '
               'full cell relight adds in each lighting state'),
        assumptions=['fresh emulator per case; fixture Super/House/Building/Bullet layouts '
                     'from live disassembly',
                     'x87 control word 0x0E7F (53-bit chop) at each entry',
                     'ClickFire: no charge drain, no one-time grant, CustomChargeTime -1',
                     'the opening block starts with EDI -1, as OnConstructionComplete '
                     'sets it at 0x445FCB'],
        substitutions=['Launch 0x6CC390, LightningStorm::HasDeferment 0x53A0E0 and '
                       'PrintMessage 0x53AE00, PsyDom::Active 0x53B400 (the row\'s answer) and '
                       'PrintMessage 0x53B410 are recorded stubs',
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
                       'count vt+0x408 are recorded/supplied stubs',
                       'AI: Fire_SW 0x4FAE50, AI_GroundRallyPoint 0x509CD0, AI_Fire_PsyDom '
                       '0x50A150 and AI_Fire_GenMutator 0x509F60 are recorded stubs in '
                       'ai_try_fire, where AI_FindBestRallyTarget 0x50CBF0 answers the row\'s '
                       'cell; Find_Nearby_Passable_Cell 0x56DC20 answers the row\'s cell',
                       'AI: object WhatAmI vt+0x2C, GetCoords vt+0x48, InWhichLayer vt+0x78, '
                       'GetOwningHouse vt+0x3C, IsHighFlying vt+0x54 and GetCell vt+0x1BC '
                       'answer supplied facts; MapClass::operator[] 0x5657A0 answers fixture '
                       'cells; every IsCellInPlayfield lookup misses (level and slope 0)',
                       'chrono_process: Update_Position 0x718260 is a recorded stub that '
                       'answers blocked for the row\'s first calls (moving +0x288 one cell '
                       'east) and otherwise true, setting Marked when placing; the owner\'s '
                       'vtable calls, the anim constructor 0x421EA0, VocClass::PlayAt '
                       '0x7509E0, PostWarpValidation 0x7187A0, Set_ArchiveTarget 0x70C610 and '
                       'ShortenPassiveScanTimer 0x70F770 are recorded stubs; '
                       'Passive_Target_Acquire 0x709480 answers false and IsCellInPlayfield '
                       '0x578460 true; the frame driver reads +0x270/+0x271/+0x27C for the '
                       'class AI gates (no Temporal attacker, the Unit reaches FootClass::AI)',
                       'chrono_update_position: MapClass::operator[] by coordinate 0x565730 '
                       'and by cell 0x5657A0 answer fixture cells (GetCoords: the centre raised '
                       '104 leptons per level); object IsIronCurtained vt+0x160, WhatAmI '
                       'vt+0x2C, GetCoords vt+0x48, GetTechnoType vt+0x84, ReceiveDamage '
                       'vt+0x16C and the owner\'s PUT/REMOVE vt+0xF0/vt+0xF4 are supplied or '
                       'recorded stubs; the floor height 0x578080, the zone 0x56D230 (7) and '
                       'Find_Nearby_Passable_Cell 0x56DC20 answer the row; the bridge height '
                       '0xB0EC2C holds 416, as its initializer 0x717F60 leaves it',
                       'chrono_destination: the slices run on a fixture Launch frame (the '
                       'offset entry, the clicked cell and the Super in their stack slots); '
                       'object WhatAmI vt+0x2C and GetCoords vt+0x48 answer the row; the '
                       'bridge height 0xB0C07C holds 416, as its initializer 0x6CAD80 '
                       'leaves it',
                       'psydom_process: the anim type\'s GetImage vt+0x9C answers a fixture '
                       'image whose frame count (+6) the row writes; MindControlArea '
                       '0x53B080 and UpdateLighting 0x53C280 are recorded stubs; the firing '
                       'stages come from a binary search over 0..2*frames, the test being '
                       'monotonic in the stage',
                       'psydom_start: MapClass::operator[] 0x5657A0 answers one fixture cell '
                       '(GetCoords: the centre raised 104 leptons per level); the anim '
                       'constructor 0x421EA0 and UpdateLighting 0x53C280 are recorded stubs',
                       'update_lighting: RecalcLighting 0x53AD00 is a recorded stub',
                       'ambient_step: the slice starts with EBP, EBX and EDI holding the '
                       'Scenario, the Rules and the frame, as PerTickUpdate leaves them; '
                       '0x4AE4C0 and 0x4F42F0 are recorded stubs',
                       'dominator_lighting_read: Set_Defaults runs from 0x683915 with EBX '
                       'zero and EAX 100, the values its earlier instructions leave; '
                       'CCINIClass::ReadDouble 0x5283D0 is not run: a trampoline loads its '
                       'answer (the default, or the token scanned as a float and widened) '
                       'into ST0 before each conversion slice',
                       'relight: the slice starts after the light gather with EBX and '
                       '[ESP+0x50] pointing at the top (holding the ambient) and bottom '
                       'scalars, [ESP+0x44] at the gathered additive and EDI at a fixture '
                       'cell holding only its level; the '
                       'Scenario comes from Set_Defaults\' block as in '
                       'dominator_lighting_read, with authored Ground/Level stand-ins'],
        entry_points={'ClickFire': 0x6CB920, 'defense_alert': 0x4FAF00,
                      'Mission_Missile': 0x44C980, 'NukeMaker': 0x46B310,
                      'UpdateAnimation_super_anim': 0x450F9E,
                      'OnConstructionComplete_super_anim': 0x4463F0,
                      'AI_TryFireSW': 0x5098F0, 'AI_FindBestRallyTarget': 0x50CBF0,
                      'AI_GroundRallyPoint': 0x509CD0, 'AI_Fire_GenMutator': 0x509F60,
                      'TeleportLocomotionClass::Process': 0x7192F0,
                      'TeleportLocomotionClass::Update_Position': 0x718260,
                      'SuperClass::Launch_case4_destination': 0x6CC9AF,
                      'PsychicDominator::Process': 0x53AF40, 'PsyDom::Start': 0x53AE50,
                      'ScenarioClass::UpdateLighting': 0x53C280,
                      'LogicClass::PerTickUpdate_ambient_fade': 0x55B33D,
                      'ScenarioClass::Set_Defaults_lighting': 0x683915,
                      'ScenarioClass::Read_INI_Basic_dominator': 0x68AAFD,
                      'CellClass::ProcessColourComponents_profile_arms': 0x48445F}))
