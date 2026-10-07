"""Original AircraftClass::Enter_Idle_Mode (0x004176F0) up to its airborne arm and
AircraftClass::Mission_Retreat (0x00415A50), one call per row.

Each row runs the ORIGINAL function on a supplied Aircraft whose vtable is a clone of 0x007E22A4
with only the slots below replaced by scratch INT3 stubs. A stub records its arguments and
returns the row's value with the native callee's stack cleanup; entry hooks at native callees do
the same without running their bodies. Every row asserts that the call returns to its caller
with the native stack cleanup and EBX/EBP/ESI/EDI preserved, writes nothing outside the stack and
the owner, and reads no scratch byte (owner, type, house, interface) the row did not supply.

Enter_Idle_Mode, called as Mission_Move (0x004166E0) and Mission_Attack state 10 (0x00418CFA)
call it: vt+0x484(0, 1), thiscall, RET 8.
    The body and Is_Suspended vt+0x1FC (0x005B3A10, +0xB0 != -1) run natively; the type's
    MissileSpawn (+0xD68) and AirportBound (+0xE0D) bytes are the row's.
    Stubs: Restore vt+0x1F8 (0x004D8F80; the stub moves the suspended selector +0xB0 to the
    current +0xAC and empties +0xB0, the selector half of the restore), Commence vt+0x1EC
    (0x0041B870), Queue_Mission vt+0x1E8 (0x0041BA90), Ready_To_Commence vt+0x200 (0x0041B5E0),
    Is_Armed vt+0x2AC (0x00701120), Assign_Target vt+0x3C8 (0x006FCDB0), Assign_Destination
    vt+0x480 (0x0041AA80), the tail's dock search vt+0x528 (0x0041BBD0; recorded, answers no
    dock), Crash vt+0x3DC (0x004DEBB0), the layer vt+0x78 (0x0041ADC0), the height vt+0x1C8
    (0x005F5F40) and IFlyControl+0xC, the landing altitude (0x0041B6A0), through a scratch
    interface vtable at +0x6C0.
    Entry hooks: FootClass::Enter_Idle_Mode 0x004D82B0 (RET 8), HouseClass::IsControlledByHuman
    0x0050B730 and RadioClass::In_Radio_Contact 0x0065AE30. The airborne arm's entry 0x00417802
    is recorded and the row leaves through the function's own epilogue 0x00417B8E (which
    restores the four registers and returns 0), so its callees (GetWeapon vt+0x3F8, Get_Mission
    vt+0x184, vt+0x54, Transmit_Message vt+0x278, 0x006EC370, 0x006EA870 and
    Find_Nearest_Friendly_Airfield 0x0041A160) fail the row if reached.

Mission_Retreat (vt+0x230, thiscall, no arguments, RET):
    The body, HouseClass 0x0050DA80 (the waypoint edge +0x577C, 0 outside 0..3) and the static
    initializer 0x00413C60 of the empty cell 0x00889E68 run natively. Stubs: GetCell vt+0x1BC
    (0x005F6960) answers the owner's cell and Assign_Destination vt+0x480. Entry hooks:
    MapClass::PickCellOnEdge 0x004AA440 records its arguments and answers the row's cell;
    MapClass::operator[] 0x005657A0 records the cell and answers a distinct scratch pointer per
    cell. PickCellOnEdge itself, with its Scenario RNG draws, is aircraft_states.py's evidence:
    Mission_Attack state 10 calls it with the same arguments.
"""
from pathlib import Path
import struct

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE, UC_HOOK_MEM_READ, UC_HOOK_MEM_WRITE
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX,
                               UC_X86_REG_EDI, UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP)

from tools.native_oracle import (RET_MAGIC, SCRATCH, SCRATCH_SIZE, STACK_BASE, STACK_SIZE,
                                 OracleError, finish_vectors, load_image, provenance, run_checked)

ENTER_IDLE, IDLE_BODY = 0x4176F0, (0x4176F0, 0x417BAA)
MISSION_RETREAT, RETREAT_BODY = 0x415A50, (0x415A50, 0x415B09)
AIRCRAFT_VT, FLY_VT = 0x7E22A4, 0x7E2250
MAP, EMPTY_CELL, EMPTY_CELL_INITIALIZER = 0x87F7E8, 0x889E68, 0x413C60

# Scratch layout (SCRATCH .. SCRATCH + 0x10000).
OWNER = SCRATCH + 0x1000            # AircraftClass
TYPE = SCRATCH + 0x2000             # AircraftTypeClass (+0xD68 MissileSpawn)
VTABLE = SCRATCH + 0x3000           # clone of 0x007E22A4 (0x600 bytes)
IFLY_VT = SCRATCH + 0x3800          # IFlyControl vtable whose +0xC is a stub
STUBS = SCRATCH + 0x3C00            # INT3 stubs, 0x10 apart
TEAM, AIRSTRIKE, PASSENGER, NAV_OBJECT = (SCRATCH + 0x4000 + 0x100 * n for n in range(4))
CELL_POINTERS = SCRATCH + 0x5000    # opaque CellClass pointers handed out by operator[]
HOUSE = SCRATCH + 0x8000            # HouseClass (+0x1E0 Edge=, +0x577C waypoint edge)
SP = STACK_BASE + STACK_SIZE - 0x1000
SENTINELS = {UC_X86_REG_EBX: 0x0B0B0B0B, UC_X86_REG_EBP: 0x0E0E0E0E,
             UC_X86_REG_ESI: 0x05050505, UC_X86_REG_EDI: 0x0D0D0D0D}

# Cloned vtable slots: offset -> (stub name, native target asserted against the image, pops).
SLOTS = {
    0x1F8: ("restore", 0x4D8F80, 0), 0x1EC: ("commence", 0x41B870, 0),
    0x1E8: ("queue_mission", 0x41BA90, 8), 0x200: ("ready", 0x41B5E0, 0),
    0x2AC: ("is_armed", 0x701120, 0), 0x78: ("layer", 0x41ADC0, 0),
    0x1C8: ("height", 0x5F5F40, 0), 0x3C8: ("assign_target", 0x6FCDB0, 4),
    0x480: ("assign_destination", 0x41AA80, 8), 0x1BC: ("get_cell", 0x5F6960, 0),
    0x3F8: ("get_weapon", 0x70E140, None), 0x184: ("get_mission", 0x5B3040, None),
    0x54: ("vt54", 0x41B920, None), 0x528: ("dock_search", 0x41BBD0, 0xC),
    0x278: ("transmit_message", 0x65AAA0, None), 0x3DC: ("crash", 0x4DEBB0, 4),
}
# Slots the two functions reach that stay native (asserted only).
NATIVE_SLOTS = {0x1FC: 0x5B3A10, 0x484: ENTER_IDLE, 0x230: MISSION_RETREAT}
# The airborne arm's entry and the epilogue a row leaves it through.
AIRBORNE_ARM, IDLE_RETURN_ZERO = 0x417802, 0x417B8E
# Entry hooks: address -> (name, pops); None pops fails the row.
HOOKS = {
    0x4D82B0: ("foot_enter_idle", 8), 0x50B730: ("house_human", 0),
    0x65AE30: ("in_radio_contact", 0), 0x4AA440: ("pick_cell_on_edge", 0x1C),
    0x5657A0: ("map_cell", 4), 0x6EC370: ("team_6ec370", None),
    0x6EA870: ("team_remove_6ea870", None), 0x41A160: ("nearest_airfield", None),
    AIRBORNE_ARM: ("airborne_arm", 0),
}
STUB_AT = {name: STUBS + 0x10 * n for n, (name, _, _) in enumerate(SLOTS.values())}
STUB_AT["landing_altitude"] = STUBS + 0x10 * len(SLOTS)
STUB_NAME = {address: name for name, address in STUB_AT.items()}
STUB_POPS = {name: pops for name, _, pops in SLOTS.values()} | {"landing_altitude": 4}


def u32(value):
    return struct.pack("<I", int(value) & 0xFFFFFFFF)


def s32(value):
    return struct.unpack("<i", u32(value))[0]


class Fixture:
    def __init__(self):
        u = self.u = Uc(UC_ARCH_X86, UC_MODE_32)
        load_image(u)
        u.mem_map(STACK_BASE, STACK_SIZE)
        u.mem_map(SCRATCH, SCRATCH_SIZE)
        u.mem_map(RET_MAGIC, 0x1000)
        for offset, (_, native, _) in SLOTS.items():
            if self.word(AIRCRAFT_VT + offset) != native:
                raise OracleError(f"Aircraft vt+0x{offset:X} is not 0x{native:08X}")
        for offset, native in NATIVE_SLOTS.items():
            if self.word(AIRCRAFT_VT + offset) != native:
                raise OracleError(f"Aircraft vt+0x{offset:X} is not 0x{native:08X}")
        if self.word(FLY_VT + 0xC) != 0x41B6A0:
            raise OracleError("IFlyControl+0xC is not 0x0041B6A0")
        u.mem_write(SP, u32(RET_MAGIC))
        u.reg_write(UC_X86_REG_ESP, SP)
        run_checked(u, EMPTY_CELL_INITIALIZER, RET_MAGIC, count=100)
        self.auditing = False
        u.hook_add(UC_HOOK_CODE, self.on_stub, begin=STUBS, end=STUBS + 0x3FF)
        for address in HOOKS:
            u.hook_add(UC_HOOK_CODE, self.on_hook, begin=address, end=address)
        u.hook_add(UC_HOOK_MEM_WRITE, self.on_write)
        u.hook_add(UC_HOOK_MEM_READ, self.on_read, begin=SCRATCH, end=SCRATCH + SCRATCH_SIZE - 1)

    def word(self, address):
        return struct.unpack("<I", self.u.mem_read(address, 4))[0]

    def put(self, address, blob):
        self.u.mem_write(address, blob)
        self.declared.append((address, address + len(blob)))

    def put32(self, address, *values):
        self.put(address, b"".join(u32(v) for v in values))

    def put8(self, address, value):
        self.put(address, bytes([int(value) & 0xFF]))

    def cell_pointer(self, cell):
        cell = tuple(cell)
        if cell not in self.cells:
            self.cells[cell] = CELL_POINTERS + 4 * len(self.cells)
        return self.cells[cell]

    def name(self, pointer):
        if pointer == 0:
            return None
        named = {NAV_OBJECT: "object", EMPTY_CELL: "empty_cell", MAP: "map"}
        if pointer in named:
            return named[pointer]
        for cell, address in self.cells.items():
            if pointer == address:
                return ["cell", *cell]
        raise OracleError(f"unexpected pointer 0x{pointer:08X}")

    # -- fixture
    def build(self, row):
        u = self.u
        self.row, self.calls, self.violations, self.cells = row, [], [], {}
        self.declared, self.undeclared_reads = [], set()
        u.mem_write(SCRATCH, bytes(SCRATCH_SIZE))
        u.mem_write(SP - 0x1000, bytes(0x1100))
        u.mem_write(STUBS, b"\xCC" * 0x400)
        vt = bytearray(u.mem_read(AIRCRAFT_VT, 0x600))
        for offset, (name, _, _) in SLOTS.items():
            vt[offset:offset + 4] = u32(STUB_AT[name])
        self.put(VTABLE, bytes(vt))
        ifly = bytearray(u.mem_read(FLY_VT, 0x40))
        ifly[0xC:0x10] = u32(STUB_AT["landing_altitude"])
        self.put(IFLY_VT, bytes(ifly))
        self.put32(OWNER, VTABLE)
        self.put32(OWNER + 0x21C, HOUSE)
        self.put32(OWNER + 0x6C0, IFLY_VT)
        self.put32(OWNER + 0x6C4, TYPE)

    def build_idle(self, row):
        self.build(row)
        self.put32(OWNER + 0xAC, row["current"])
        self.put32(OWNER + 0xB0, row["suspended"])
        self.put32(OWNER + 0xB4, row["queued"])
        self.put32(OWNER + 0xBC, row["state"])
        self.put32(OWNER + 0x118, PASSENGER if row["passengers"] else 0)
        self.put32(OWNER + 0x294, AIRSTRIKE if row["airstrike"] else 0)
        self.put32(OWNER + 0x2FC, row["ammo"])
        self.put8(OWNER + 0x3D4, row["mission_only"])
        self.put32(OWNER + 0x5D4, TEAM if row["team"] else 0)
        self.put8(OWNER + 0x6D2, row["latch"])
        self.put8(TYPE + 0xD68, row["missile_spawn"])
        self.put8(TYPE + 0xE0D, row["airport_bound"])

    def build_retreat(self, row):
        self.build(row)
        nav = row["nav"]
        if nav is None:
            pointer = 0
        elif nav == "object":
            pointer = NAV_OBJECT
        else:
            pointer = self.cell_pointer(row["own"] if nav == "own" else nav)
        self.put32(OWNER + 0x5A4, pointer)
        self.put32(HOUSE + 0x1E0, row["house_edge"])
        self.put32(HOUSE + 0x577C, row["waypoint_edge"])

    # -- hooks
    def arg(self, n):
        return self.word(self.u.reg_read(UC_X86_REG_ESP) + 4 * (n + 1))

    def returns(self, value, pops):
        u = self.u
        sp = u.reg_read(UC_X86_REG_ESP)
        u.reg_write(UC_X86_REG_EAX, int(value) & 0xFFFFFFFF)
        u.reg_write(UC_X86_REG_EIP, self.word(sp))
        u.reg_write(UC_X86_REG_ESP, sp + 4 + pops)

    def fail(self, message):
        self.violations.append(message)
        self.u.emu_stop()

    def from_body(self, name):
        caller = self.word(self.u.reg_read(UC_X86_REG_ESP))
        if not self.body[0] <= caller < self.body[1]:
            self.fail(f"{name} called from 0x{caller:08X}, outside the function under test")
            return False
        return True

    def on_stub(self, u, address, _size, _data):
        name = STUB_NAME.get(address)
        if name is None:
            return self.fail(f"unknown stub 0x{address:08X}")
        if not self.from_body(name):
            return None
        pops = STUB_POPS[name]
        if pops is None:
            return self.fail(f"{name} reached")
        this = u.reg_read(UC_X86_REG_ECX)
        if name == "landing_altitude":
            this = self.arg(0)
            if this != OWNER + 0x6C0:
                return self.fail(f"landing altitude on 0x{this:08X}")
        elif this != OWNER:
            return self.fail(f"{name} called on 0x{this:08X}")
        row, value = self.row, 0
        if name == "restore":
            self.calls.append(["restore"])
            u.mem_write(OWNER + 0xAC, bytes(u.mem_read(OWNER + 0xB0, 4)))
            u.mem_write(OWNER + 0xB0, u32(-1))
        elif name == "commence":
            self.calls.append(["commence"])
        elif name == "queue_mission":
            self.calls.append(["queue_mission", s32(self.arg(0)), s32(self.arg(1))])
        elif name == "ready":
            self.calls.append(["ready"])
            value = row["ready"]
        elif name == "assign_target":
            self.calls.append(["assign_target", self.name(self.arg(0))])
        elif name == "assign_destination":
            self.calls.append(["assign_destination", self.name(self.arg(0)), s32(self.arg(1))])
        elif name == "get_cell":
            self.calls.append(["get_cell"])
            value = self.cell_pointer(row["own"])
        elif name == "dock_search":
            self.calls.append(["dock_search", s32(self.arg(1)), s32(self.arg(2))])
        elif name == "crash":
            self.calls.append(["crash", self.name(self.arg(0))])
            value = 1
        else:                                              # is_armed, layer, height, landing
            value = {"is_armed": row.get("armed"), "layer": row.get("layer"),
                     "height": row.get("height"), "landing_altitude": row.get("landing")}[name]
        return self.returns(value, pops)

    def on_hook(self, u, address, _size, _data):
        name, pops = HOOKS[address]
        if name == "airborne_arm":
            self.calls.append(["airborne_arm"])
            u.reg_write(UC_X86_REG_EIP, IDLE_RETURN_ZERO)
            return None
        if not self.from_body(name):
            return None
        if pops is None:
            return self.fail(f"{name} reached")
        row, value = self.row, 0
        if name == "foot_enter_idle":
            if u.reg_read(UC_X86_REG_ECX) != OWNER:
                return self.fail("Foot Enter_Idle_Mode on another object")
            self.calls.append(["foot_enter_idle", s32(self.arg(0)), s32(self.arg(1))])
            value = row["foot"]
        elif name == "house_human":
            if u.reg_read(UC_X86_REG_ECX) != HOUSE:
                return self.fail("IsControlledByHuman on another house")
            value = row["human"]
        elif name == "in_radio_contact":
            if u.reg_read(UC_X86_REG_ECX) != OWNER:
                return self.fail("In_Radio_Contact on another object")
            value = row["radio"]
        elif name == "pick_cell_on_edge":
            if u.reg_read(UC_X86_REG_ECX) != MAP:
                return self.fail("PickCellOnEdge on another map")
            out = self.arg(0)
            self.calls.append(["pick_cell_on_edge", s32(self.arg(1)), self.name(self.arg(2)),
                               self.name(self.arg(3)), s32(self.arg(4)), s32(self.arg(5)),
                               s32(self.arg(6))])
            u.mem_write(out, struct.pack("<hh", *row["pick"]))
            value = out
        elif name == "map_cell":
            if u.reg_read(UC_X86_REG_ECX) != MAP:
                return self.fail("operator[] on another map")
            cell = list(struct.unpack("<hh", u.mem_read(self.arg(0), 4)))
            self.calls.append(["map_cell", cell])
            value = self.cell_pointer(cell)
        return self.returns(value, pops)

    def on_write(self, u, _access, address, size, _value, _data):
        if not self.auditing:
            return
        end = address + size
        if STACK_BASE <= address and end <= STACK_BASE + STACK_SIZE:
            return
        if OWNER <= address and end <= OWNER + 0x1000:
            return
        self.fail(f"write outside the stack and owner at 0x{address:08X}+{size} "
                  f"from 0x{u.reg_read(UC_X86_REG_EIP):08X}")

    def on_read(self, u, _access, address, size, _value, _data):
        if self.auditing and not any(low <= address and address + size <= high
                                     for low, high in self.declared):
            self.undeclared_reads.add((address, size, u.reg_read(UC_X86_REG_EIP)))

    # -- one call
    def execute(self, entry, body, args):
        u, row = self.u, self.row
        self.body = body
        sp = SP - 4 * len(args)
        u.mem_write(sp, u32(RET_MAGIC) + b"".join(u32(a) for a in args))
        for register, value in SENTINELS.items():
            u.reg_write(register, value)
        u.reg_write(UC_X86_REG_ECX, OWNER)
        u.reg_write(UC_X86_REG_ESP, sp)
        self.auditing = True
        try:
            run_checked(u, entry, RET_MAGIC, count=100_000, context={"row": row["name"]})
        except OracleError as error:
            raise OracleError(f"{row['name']}: {self.violations or error}") from error
        finally:
            self.auditing = False
        if self.violations:
            raise OracleError(f"{row['name']}: {self.violations}")
        if u.reg_read(UC_X86_REG_ESP) != sp + 4 + 4 * len(args):
            raise OracleError(f"{row['name']}: unbalanced stack at return")
        if any(u.reg_read(register) != value for register, value in SENTINELS.items()):
            raise OracleError(f"{row['name']}: callee-saved register not restored")
        if self.undeclared_reads:
            reads = ", ".join(f"0x{a:08X}+{s}@0x{pc:08X}" for a, s, pc in sorted(self.undeclared_reads))
            raise OracleError(f"{row['name']}: read of unsupplied scratch bytes: {reads}")
        return u.reg_read(UC_X86_REG_EAX)

    def enter_idle(self, row):
        row = {**IDLE_DEFAULTS, **row}
        self.build_idle(row)
        eax = self.execute(ENTER_IDLE, IDLE_BODY, [0, 1])
        return {"input": row, "ret": eax & 0xFF, "calls": self.calls,
                "state": s32(self.word(OWNER + 0xBC)), "latch": self.u.mem_read(OWNER + 0x6D2, 1)[0]}

    def retreat(self, row):
        row = {**RETREAT_DEFAULTS, **row}
        self.build_retreat(row)
        eax = self.execute(MISSION_RETREAT, RETREAT_BODY, [])
        return {"input": row, "ret": s32(eax), "calls": self.calls}


IDLE_DEFAULTS = {
    "current": 2, "queued": -1, "suspended": -1, "airstrike": False, "human": False,
    "team": False, "passengers": False, "mission_only": True, "ammo": 1, "radio": False,
    "ready": True, "foot": False, "layer": 3, "height": 768, "landing": 0, "state": 7, "latch": 1,
    "missile_spawn": True, "armed": False, "airport_bound": False,
}
RETREAT_DEFAULTS = {"nav": None, "own": [60, 64], "house_edge": -1, "waypoint_edge": 0,
                    "pick": [5, 6]}


def idle_rows():
    rows = []
    # The missile arm: every combination of what it reads, from Move with nothing queued.
    for mission_only in (True, False):
        for passengers in (False, True):
            for team in (False, True):
                for human in (False, True):
                    for radio in (False, True):
                        for ready in (True, False):
                            for foot in (False, True):
                                rows.append(dict(
                                    name=f"arm.m{int(mission_only)}.p{int(passengers)}"
                                         f".t{int(team)}.h{int(human)}.r{int(radio)}"
                                         f".ready{int(ready)}.f{int(foot)}",
                                    mission_only=mission_only, passengers=passengers, team=team,
                                    human=human, radio=radio, ready=ready, foot=foot))
    # The head: a suspended selector, the protected currents and the queued Commences.
    for suspended in (1, 0x19, 5):
        rows.append(dict(name=f"head.suspended{suspended}", suspended=suspended))
    for current in (4, 0x1A, 0x1B, 0x1E, 0x1F, 0, 1, 5, 0x19):
        for airstrike in (False, True):
            rows.append(dict(name=f"head.current{current}.as{int(airstrike)}", current=current,
                             airstrike=airstrike))
    for queued in (0x1A, 0x1E, 0x1B, 0x1F, 1, 2, 4, 5):
        rows.append(dict(name=f"head.queued{queued}", queued=queued))
    rows.append(dict(name="head.suspended_over_protected", suspended=1, current=4))
    rows.append(dict(name="head.protected_over_queued", current=4, queued=0x1A))
    # What the missile arm does not read: Ammo, the layer and the height.
    for ammo in (0, -1, 2):
        for mission_only in (True, False):
            rows.append(dict(name=f"ammo{ammo}.m{int(mission_only)}", ammo=ammo,
                             mission_only=mission_only))
    for layer, height, landing in ((2, 768, 0), (3, 0, 0), (3, 100, 100), (3, 768, 100), (1, -5, 0)):
        rows.append(dict(name=f"layer{layer}.z{height}.landing{landing}", layer=layer,
                         height=height, landing=landing))
    # Any other aircraft takes the same arm on the ground (the layer) or at or below its
    # landing altitude (the height); armed, a computer house's unteamed one picks Area Guard.
    entries = {"ground": dict(layer=2), "low": dict(layer=3, height=100, landing=100)}
    for entry, place in entries.items():
        for mission_only in (True, False):
            for passengers in (False, True):
                for team in (False, True):
                    for human in (False, True):
                        for armed in (False, True):
                            for radio in (False, True):
                                rows.append(dict(
                                    name=f"landed.{entry}.m{int(mission_only)}"
                                         f".p{int(passengers)}.t{int(team)}.h{int(human)}"
                                         f".a{int(armed)}.r{int(radio)}",
                                    missile_spawn=False, mission_only=mission_only,
                                    passengers=passengers, team=team, human=human, armed=armed,
                                    radio=radio, **place))
    # An armed MissileSpawn aircraft (no retail type).
    for mission_only in (True, False):
        for passengers in (False, True):
            for team in (False, True):
                for human in (False, True):
                    rows.append(dict(name=f"armed_missile.m{int(mission_only)}.p{int(passengers)}"
                                          f".t{int(team)}.h{int(human)}",
                                     armed=True, mission_only=mission_only,
                                     passengers=passengers, team=team, human=human))
    # The tail's dock hunt: armed, no Ammo and no radio contact.
    for entry, place in {"missile": {}, "ground": dict(missile_spawn=False, layer=2)}.items():
        for mission_only in (True, False):
            for radio in (False, True):
                for airport_bound in (False, True):
                    for ammo in (0, -1):
                        rows.append(dict(
                            name=f"dock.{entry}.m{int(mission_only)}.r{int(radio)}"
                                 f".ab{int(airport_bound)}.ammo{ammo}",
                            armed=True, ammo=ammo, mission_only=mission_only, radio=radio,
                            airport_bound=airport_bound, **place))
    # The airborne arm, after the head and the Foot base.
    for name, extra in (("plain", {}), ("armed", dict(armed=True)), ("foot", dict(foot=True)),
                        ("suspended", dict(suspended=1)), ("protected", dict(current=4)),
                        ("queued", dict(queued=0x1A)), ("low_ammo", dict(armed=True, ammo=0))):
        rows.append(dict(name=f"airborne.{name}", missile_spawn=False, **extra))
    return rows


def retreat_rows():
    rows = []
    for house_edge in (-1, 0, 1, 2, 3, 4, -5):
        for waypoint_edge in (0, 1, 2, 3, -1, 4):
            rows.append(dict(name=f"null.h{house_edge}.w{waypoint_edge}", house_edge=house_edge,
                             waypoint_edge=waypoint_edge))
    for pick in ([0, 0], [0, 7], [7, 0], [-1, 0], [0, -1]):
        rows.append(dict(name=f"null.pick{pick[0]}_{pick[1]}", pick=pick, waypoint_edge=2))
    for nav in ("own", [61, 64], [60, 65], "object"):
        label = nav if isinstance(nav, str) else f"cell{nav[0]}_{nav[1]}"
        rows.append(dict(name=f"nav.{label}", nav=nav, house_edge=1, waypoint_edge=3))
    return rows


def generate():
    fixture = Fixture()
    return {"enter_idle": [fixture.enter_idle(row) for row in idle_rows()],
            "mission_retreat": [fixture.retreat(row) for row in retreat_rows()]}


def metadata():
    return provenance(
        scope="AircraftClass::Enter_Idle_Mode 0x004176F0 as vt+0x484(0, 1) up to its airborne "
              "arm. On an unarmed MissileSpawn aircraft: every combination of +0x3D4, passengers "
              "(+0x118), team (+0x5D4), IsControlledByHuman, In_Radio_Contact, Ready_To_Commence "
              "and the Foot base's return, from Move with nothing queued; a suspended selector; "
              "each protected current with and without an Airstrike (+0x294); queued missions; "
              "Ammo, layer and height values. Without MissileSpawn, the landed arm entered on the "
              "ground layer and at the landing altitude over every combination of +0x3D4, "
              "passengers, team, IsControlledByHuman, Is_Armed and In_Radio_Contact; armed "
              "MissileSpawn rows; the tail's dock hunt (Ammo 0 or -1, radio contact, AirportBound) "
              "up to its dock search, which answers no dock; the airborne arm's entry. AircraftClass::Mission_Retreat 0x00415A50 with a NULL "
              "NavCom over House+0x1E0 {-1, 0..4, -5} x +0x577C {0..3, -1, 4}, picked cells "
              "equal to and differing from the empty cell in either word, and a NavCom that is "
              "the owner's cell, another cell or an object. Returned value, ordered call log "
              "with arguments, and for Enter_Idle_Mode Mission+0xBC and +0x6D2 afterwards. Not "
              "the airborne arm's body, a found dock, the restore's target and destination "
              "setters, or PickCellOnEdge's own search and draws.",
        assumptions=[
            "One emulator; per row the scratch region (0x10000 bytes) and the stack frame are "
            "rewritten. The owner holds a clone of the Aircraft vtable 0x007E22A4 with only the "
            "stubbed slots replaced, House +0x21C, IFlyControl +0x6C0 on a scratch copy of "
            "0x007E2250 with +0xC replaced, and type +0x6C4 with the row's MissileSpawn "
            "(+0xD68) and AirportBound (+0xE0D).",
            "A row reaching the airborne arm (0x00417802) records it and leaves through the "
            "function's epilogue 0x00417B8E, which the arm's own returns share: the frame holds "
            "only the four pushed registers there.",
            "Enter_Idle_Mode rows preset Mission+0xBC = 7 and +0x6D2 = 1 to show which arm "
            "writes them; the arguments are (0, 1), as both Mission_Move and Mission_Attack "
            "state 10 push them.",
            "The empty cell 0x00889E68 is set by its original static initializer 0x00413C60 "
            "(both words 0).",
            "Per row: RET to the caller with the native cleanup (RET 8 / RET) and "
            "EBX/EBP/ESI/EDI preserved; no write outside the stack and the owner; every read of "
            "scratch bytes falls on bytes the row supplied.",
        ],
        substitutions=[
            "Enter_Idle_Mode: Restore vt+0x1F8 moves +0xB0 to +0xAC and empties +0xB0; Commence "
            "vt+0x1EC, Queue_Mission vt+0x1E8, Assign_Target vt+0x3C8 and Assign_Destination "
            "vt+0x480 record their arguments; Ready vt+0x200 and Is_Armed vt+0x2AC answer the "
            "row's values; the dock search vt+0x528 records its two flags and answers no dock; "
            "Crash vt+0x3DC records its attacker; the layer vt+0x78, height vt+0x1C8 and landing altitude "
            "IFlyControl+0xC answer the row's values; FootClass::Enter_Idle_Mode 0x004D82B0, "
            "IsControlledByHuman 0x0050B730 and In_Radio_Contact 0x0065AE30 are entry hooks "
            "answering the row's values.",
            "Mission_Retreat: GetCell vt+0x1BC answers the owner's cell; Assign_Destination "
            "vt+0x480 records; PickCellOnEdge 0x004AA440 records its arguments and writes the "
            "row's cell; operator[] 0x005657A0 answers one opaque pointer per cell.",
        ],
        entry_points={"enter_idle_mode": ENTER_IDLE, "mission_retreat": MISSION_RETREAT,
                      "is_suspended": 0x5B3A10, "house_edge": 0x50DA80,
                      "empty_cell_initializer": EMPTY_CELL_INITIALIZER})


if __name__ == "__main__":
    finish_vectors(generate, Path(__file__).with_suffix(".json"), provenance=metadata)
