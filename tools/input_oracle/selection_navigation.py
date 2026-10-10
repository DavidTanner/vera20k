"""Original N/M/P, retained T/Health selection and VoiceSelect RNG controls.

Initialized actors/layers are fixture inputs, not a constructor or scenario-load
claim. Search and selection predicates execute unchanged. See the adjacent md.
"""

from copy import deepcopy
from functools import lru_cache
from pathlib import Path
from types import SimpleNamespace
import hashlib
import os
import struct

from unicorn import UC_HOOK_CODE, UC_HOOK_MEM_WRITE
from unicorn.x86_const import (
    UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX,
    UC_X86_REG_EDX, UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP, UC_X86_REG_FPCW,
)

from tools.native_oracle import (
    RET_MAGIC, SCRATCH, NativeCallTrace, finish_vectors, provenance, run_checked,
)
from tools.input_oracle.fast_scroll import (
    DISPLAY, byte, i32, put8, return_from_sink,
)
from tools.rmg_oracle.gen_rng_vectors import STRUCT_LEN, draws, seeded_struct
from tools.storage_oracle.keyboard_bindings import KeyboardFixture, stock_csf
from tools.projectile_oracle.bridge_render_inputs import BulletReader, lexical
from tools.spatial_oracle.building_body_rules import RULES, SP, dwords

HERE = Path(__file__).resolve().parent
SEARCH = {"next": 0x4AA2B0, "previous": 0x4AA380}
COMMAND = {"next": "NextObject", "previous": "PreviousObject"}
VTABLES = {"infantry": 0x7EB058, "unit": 0x7F5C70, "building": 0x7E3EBC}
MODE_FIELDS = {"repair": 0x11B0, "sell": 0x11B1,
               "power": 0x11B2, "planning": 0x11B3}
ACTOR_BYTES = {"limbo": 0x81, "alive": 0x90, "in_playfield": 0x3D5,
               "discovered": 0x41B, "robot_offline": 0x1C8,
               "docked": 0x418, "locomotor_swap": 0x6AD,
               "virtual_1d4": 0x270, "mission_only": 0x3D4}
SPECS = {
    0x4AA2B0: ("next", 1, 4), 0x4AA380: ("previous", 1, 4),
    0x6F32D0: ("local_gate", 0, 0), 0x6FC030: ("dynamic_gate", 0, 0),
    0x48DC90: ("unselect_all", 0, 0), 0x5F44A0: ("deselect", 0, 0),
    0x6FBFA0: ("select", 0, 0), 0x5F4520: ("object_select", 0, 0),
    0x4AEB30: ("set_follow", 1, 4), 0x4AE290: ("center_selection", 0, 0),
    0x4AC820: ("power_mode", 1, 4), 0x4AC700: ("planning_mode", 2, 8),
    0x4AC8C0: ("repair_mode", 1, 4), 0x4AC660: ("sell_mode", 1, 4),
    0x708EB0: ("voice_select", 0, 0), 0x708D90: ("queue_voice", 1, 4),
    0x65C780: ("rng_raw", 0, 0),
}
SELECTION_SPECS = {
    0x732280: ("combatant_command", 0, 0),
    0x732580: ("alive_owner_gate", 0, 0),
    0x7342C0: ("screen_techno_gate", 0, 0),
    0x7325C0: ("combatant_final_gate", 0, 0),
    0x6DA770: ("unselect_if_not_owned", 0, 0),
    0x6DA740: ("tactical_unselect_all", 0, 0),
    0x731D00: ("reset_selection_mode", 0, 0),
    0x732CA0: ("type_press", 0, 0),
    0x732CC0: ("type_release", 0, 0),
    0x732950: ("type_tap", 0, 0),
    0x733380: ("health_command", 0, 0),
    0x70D150: ("action_timer_start", 0, 0),
    0x70D4A0: ("world_detach", 2, 8),
    0x733160: ("navigation_pointer_expired", 0, 0),
    0x7258D0: ("pointer_expired_dispatch", 0, 0),
    0x5F5280: ("object_detach_all", 1, 4),
    0x5F4D30: ("object_conceal", 0, 0),
    0x4D9720: ("foot_detach_all", 1, 4),
    0x4A9770: ("display_remove", 1, 4),
    0x55BAE0: ("logic_remove", 1, 4),
    0x439150: ("bomb_pointer_expiry", 1, 4),
    0x54E590: ("temporal_pointer_expiry", 1, 4),
    0x6DA560: ("tactical_pointer_expiry", 2, 8),
    0x55B880: ("logic_pointer_expiry", 2, 8),
    0x65ACB0: ("radio_transmit_first", 1, 4),
}
ACTION_TIMER = 0xB0EA80


@lru_cache(maxsize=None)
def seed_bytes(seed):
    return seeded_struct(seed)


def actor(identity, kind="unit", **overrides):
    result = dict(id=identity, kind=kind, health=100, limbo=False, alive=True,
                  in_playfield=True, discovered=True, selectable=True,
                  owner="local", slave=False, robot_offline=False, bunker=False,
                  docked=False, cell_building=False, locomotor_swap=False, virtual_1d4=False,
                  mission_only=False, techno_cast=True,
                  coord=[identity * 257, identity * 513, identity * 7],
                  voice_list=[101], queued_voice=-1)
    result.update(overrides)
    return result


class SelectionFixture(KeyboardFixture):
    """Reuse original registered commands; supply bounded ordinary object state."""

    def __init__(self, actors, layers, *, campaign=False, other_human=False,
                 other_control=False, seed=1, voice_enabled=True,
                 selection_commands=False):
        super().__init__()
        self.selection_commands = selection_commands
        self.uc.mem_map(RET_MAGIC, 0x1000)
        self.next_free = SCRATCH + 0x10000
        self.actors, self.types, self.identities = {}, {}, {0: None}
        self.input_actors = deepcopy(actors)
        self.houses = {name: self.alloc(0x17000 if selection_commands else 0x6000)
                       for name in ("local", "other")}
        for house in self.houses.values():
            self.put(house + 0x34, self.alloc(0x200))
        self.put(0xA83D4C, self.houses["local"])
        self.put(0xA8B238, int(not campaign))
        put8(self.uc, self.houses["local"] + 0x1EC, True)
        put8(self.uc, self.houses["local"] + 0x1ED, True)
        put8(self.uc, self.houses["other"] + 0x1EC, other_human)
        put8(self.uc, self.houses["other"] + 0x1ED, other_control)
        self.put(DISPLAY, 0x7E1964)  # Actual Mouse/Display vtable.
        put8(self.uc, 0xA8ED6B, False)  # Ordinary game, not map editor.
        put8(self.uc, 0xAC4CF4, False)  # No planning-group propagation.
        put8(self.uc, 0xAC4CF6, False)
        put8(self.uc, 0x822CF2, voice_enabled)
        self.put(0xB73550, 1)
        self.put(0xA8ED84, 1000)
        for value in actors:
            pointer, typ = self.alloc(0x1000), self.alloc(0x1000)
            self.actors[value["id"]] = pointer
            self.types[value["id"]] = typ
            self.identities[pointer] = value["id"]
            self.put(pointer, VTABLES[value["kind"]])
            if selection_commands:
                # Actual full710AF0 constructor vtable, supplied as fixture data.
                # These actors/types remain initialized state, not constructed objects.
                self.put(typ, 0x7F4ED8)
            self.put(pointer + (0x6C0 if value["kind"] == "infantry" else 0x6C4), typ)
            if value["kind"] == "unit":
                # Unit virtual+37C746C90 regards any +6D8 != -1 as an active
                # control blocker. Ordinary initialized Unit uses -1; zeroed
                # storage would hide ObjectSelect's mission-only refusal.
                self.put(pointer + 0x6D8, 0xFFFFFFFF)
            self.put(pointer + 0x14, 7 if value["techno_cast"] else 0)
            self.put(pointer + 0x6C, value["health"] & 0xFFFFFFFF)
            self.put(pointer + 0x21C, self.houses[value["owner"]])
            self.put(pointer + 0x2DC, self.houses["other"] if value["slave"] else 0)
            self.put(pointer + 0x2E4, self.houses["other"] if value["bunker"] else 0)
            for field, offset in ACTOR_BYTES.items():
                put8(self.uc, pointer + offset, value[field])
            put8(self.uc, pointer + 0x74, value.get("marked", False))
            self.put(pointer + 0x94, value.get("native_layer", 0))
            self.uc.mem_write(pointer + 0x9C, struct.pack("<3i", *value["coord"]))
            self.put(pointer + 0x4F0, value["queued_voice"] & 0xFFFFFFFF)
            put8(self.uc, typ + 0x230, value["selectable"])
            put8(self.uc, typ + 0xDBC, value.get("is_selectable_combatant", False))
            self.put(typ + 0xA0, value.get("strength", 100))
            self.uc.mem_write(typ + 0x24, value.get("type_id", f"TYPE{value['id']}").encode("ascii") + b"\0")
            put8(self.uc, typ + 0xC9C, value.get("positive_primary_damage", True))
            self.set_voice_list(value["id"], value["voice_list"])
        # The dock gate uses original ObjectGetCell5F6960/Map565730 and
        # CellGetBuilding47C520. Only coordinate->cell storage/occupiers are
        # supplied; no building query is replaced by a fixture answer.
        self.put(DISPLAY + 0x13C, 0xC00000)
        self.put(DISPLAY + 0x140, 0x40000)
        put8(self.uc, 0xA8E9A0, True)
        for value in actors:
            if not value["docked"]:
                continue
            cell = self.alloc(0x200)
            self.put(cell, 0x7E4EEC)
            x, y = (coord >> 8 for coord in value["coord"][:2])
            if not (0 <= x < 512 and 0 <= y < 512):
                raise ValueError("Dock control coordinate outside supplied native cell table")
            self.put(0xC00000 + (y * 512 + x) * 4, cell)
            self.uc.mem_write(cell + 0x24, struct.pack("<hh", x, y))
            if value["cell_building"]:
                building = self.alloc(0x1000)
                self.put(building, VTABLES["building"])
                self.put(cell + 0xE4, building)
        self.set_layers(layers)
        # Original static initializer4E7D40 sets this vtable. The vector's
        # capacity/storage are supplied, as in naval_lifetime_controls.py.
        self.selection_items = self.alloc(64 * 4)
        for offset, value in ((0, 0x7E4F64), (4, self.selection_items),
                              (8, 64), (12, 1), (16, 0), (20, 10)):
            self.put(0xA8ECB8 + offset, value)
        self.scenario = self.alloc(0x7000)
        self.put(0xA8B230, self.scenario)
        self.rngs = {"main": 0x886B88, "scenario": self.scenario + 0x218,
                     "mapgen": 0xABE890}
        for pointer in self.rngs.values():
            self.uc.mem_write(pointer, seed_bytes(seed))
        self.seed = seed
        if selection_commands:
            self.csf, self.csf_sha256 = stock_csf()
            self.csf_strings = {}
            self.time_ms = 1000
            self.clock_sink = SCRATCH + 0xFF000
            self.put(0x7E1530, self.clock_sink)  # timeGetTime import transport.
            self.tactical = self.alloc(0x1000)
            self.put(0x887324, self.tactical)
            self.put(self.houses["local"] + 0x16054, 7)
            rules = self.alloc(0x2000)
            self.put(0x8871E0, rules)
            self.uc.mem_write(rules + 0x1700, struct.pack("<dd", 0.5, 0.25))
            self.uc.reg_write(UC_X86_REG_FPCW, 0x0E7F)
            self.uc.mem_write(ACTION_TIMER, struct.pack("<3i", 995, 0, 25))
            self.uc.hook_add(UC_HOOK_MEM_WRITE, self.observe_timer,
                             begin=ACTION_TIMER, end=ACTION_TIMER + 11)
        self.code_hash = self.text_hash()
        self.reset_observations()
        self.uc.hook_add(UC_HOOK_CODE, self.observe)

    def alloc(self, size):
        result = self.next_free
        self.next_free += (size + 15) & ~15
        if self.next_free >= SCRATCH + 0x100000:
            raise RuntimeError("Selection fixture storage exhausted")
        return result

    def text_hash(self):
        return hashlib.sha256(bytes(self.uc.mem_read(0x401000, 0x3E0000))).hexdigest()

    def set_voice_list(self, identity, values, offset=0x414):
        vector = self.types[identity] + offset
        items = self.alloc(max(4, 4 * len(values)))
        for index, value in enumerate(values):
            self.put(items + index * 4, value & 0xFFFFFFFF)
        self.put(vector + 4, items)
        self.put(vector + 8, len(values))
        self.put(vector + 16, len(values))

    def set_layers(self, layers):
        if len(layers) != 5:
            raise ValueError("Exactly five native layer vectors are required")
        for layer, ids in enumerate(layers):
            vector = 0x8A0360 + layer * 24
            items = self.alloc(max(4, len(ids) * 4))
            for index, identity in enumerate(ids):
                if identity is not None and identity not in self.actors:
                    raise ValueError(f"Layer refers to missing actor {identity}")
                self.put(items + index * 4, self.actors.get(identity, 0))
            self.put(vector + 4, items)
            self.put(vector + 8, len(ids))
            self.put(vector + 16, len(ids))

    def set_selection(self, identities):
        for identity, pointer in self.actors.items():
            put8(self.uc, pointer + 0x83, identity in identities)
        for index, identity in enumerate(identities):
            self.put(self.selection_items + index * 4, self.actors[identity])
        self.put(0xA8ECC8, len(identities))

    def set_display(self, *, follow=None, modes=None, placement=False):
        self.put(DISPLAY + 0x11A0, self.actors.get(follow, 0))
        put8(self.uc, DISPLAY + 0x119C, follow is not None)
        self.put(DISPLAY + 0x11A8, self.alloc(4) if placement else 0)
        for mode, offset in MODE_FIELDS.items():
            put8(self.uc, DISPLAY + offset, (modes or {}).get(mode, False))

    def reset_observations(self):
        self.trace = NativeCallTrace(self.uc, self.get)
        self.call_order, self.center_coords, self.redraws, self.cursor_calls = [], [], [], []
        self.messages, self.message_keys, self.timer_writes = [], [], []

    def observe_timer(self, uc, _access, address, size, value, _data):
        self.timer_writes.append(dict(offset=address - ACTION_TIMER, bytes=size, value=value))

    def observe(self, uc, pc, _size, _data):
        sp = uc.reg_read(UC_X86_REG_ESP)
        self.trace.returned(pc, sp)
        spec = SPECS.get(pc)
        if self.selection_commands:
            spec = SELECTION_SPECS.get(pc, spec)
        if spec:
            self.trace.entered(pc, sp, spec)
            self.call_order.append(spec[0])
        if self.selection_commands:
            if pc == 0x7C8B3D:  # Original vector destructor's CRT free boundary.
                return_from_sink(uc, 0)
                return
            if pc == self.clock_sink:
                return_from_sink(uc, 0, self.time_ms)
                return
            if pc == 0x734E60:  # StringTable transport; physical CSF strings.
                key = bytes(uc.mem_read(uc.reg_read(UC_X86_REG_ECX), 128)).split(b"\0")[0].decode("ascii")
                self.message_keys.append(key)
                if key not in self.csf_strings:
                    raw = (self.csf[key.upper()] + "\0").encode("utf-16-le")
                    pointer = self.alloc(len(raw))
                    uc.mem_write(pointer, raw)
                    self.csf_strings[key] = pointer
                return_from_sink(uc, 8, self.csf_strings[key])
                return
            if pc == 0x5D3BA0:  # Message list UI boundary, after original copying/formatting.
                args = [self.get(sp + 4 + index * 4) for index in range(7)]
                pointer = args[2]
                words = []
                while word := bytes(uc.mem_read(pointer + len(words) * 2, 2)):
                    if word == b"\0\0":
                        break
                    words.append(word)
                    if len(words) > 256:
                        raise RuntimeError("Unterminated native message")
                self.messages.append(dict(text=b"".join(words).decode("utf-16-le"),
                                          prefix=args[0], color=args[1], scheme=args[3],
                                          style=args[4], timeout=args[5], silent=args[6]))
                return_from_sink(uc, 28)
                return
        if pc == 0x6D6070:
            coord = self.get(sp + 4)
            self.center_coords.append(list(struct.unpack("<3i", uc.mem_read(coord, 12))))
            self.call_order.append("camera_coord")
            return_from_sink(uc, 4)
        elif pc == 0x4F42F0:
            self.redraws.append(self.get(sp + 4))
            self.call_order.append("redraw")
            return_from_sink(uc, 4)
        elif pc in (0x5BDA80, 0x5BDAA0):
            # Real Mouse virtual+48/+50 entries, declared cursor presentation.
            count = 2 if pc == 0x5BDA80 else 0
            self.cursor_calls.append(dict(pc=hex(pc), args=[self.get(sp + 4 + i * 4) for i in range(count)]))
            return_from_sink(uc, count * 4)

    def invoke(self, entry, receiver, arguments=()):
        self.uc.mem_write(self.stack - 0x1000, bytes(0x1800))
        self.put(self.stack, RET_MAGIC)
        for index, value in enumerate(arguments):
            self.put(self.stack + 4 + index * 4, value & 0xFFFFFFFF)
        self.uc.reg_write(UC_X86_REG_ESP, self.stack)
        self.uc.reg_write(UC_X86_REG_ECX, receiver)
        run_checked(self.uc, entry, RET_MAGIC, count=100000,
                    required_addresses=[entry])
        sp = self.uc.reg_read(UC_X86_REG_ESP)
        self.trace.returned(RET_MAGIC, sp)
        if sp != self.stack + 4 * (len(arguments) + 1):
            raise RuntimeError(f"Unbalanced return at {entry:#x}")
        if self.trace.pending:
            raise RuntimeError(f"Unreturned observed call at {entry:#x}")
        if self.text_hash() != self.code_hash:
            raise RuntimeError("Original .text changed during selection execution")
        return self.uc.reg_read(UC_X86_REG_EAX)

    def visits(self):
        return [dict(id=self.identities[int(row["ecx"], 16)],
                     gate=row["name"], accepted=bool(row["return_eax"] & 255))
                for row in self.trace.calls if row["name"] in ("local_gate", "dynamic_gate")]

    def rng_bytes(self):
        return {name: bytes(self.uc.mem_read(pointer, STRUCT_LEN))
                for name, pointer in self.rngs.items()}

    def snapshot(self):
        selected = [self.identities[self.get(self.selection_items + 4 * i)]
                    for i in range(self.get(0xA8ECC8))]
        return dict(selected=selected,
                    selected_flags={str(i): byte(self.uc, p + 0x83) for i, p in self.actors.items()},
                    follow=self.identities[self.get(DISPLAY + 0x11A0)],
                    follow_enabled=byte(self.uc, DISPLAY + 0x119C),
                    modes={name: byte(self.uc, DISPLAY + offset) for name, offset in MODE_FIELDS.items()},
                    placement=bool(self.get(DISPLAY + 0x11A8)),
                    selection_mode=self.get(0xB0FE54),
                    selection_across_map=byte(self.uc, 0xB0FE58))

    def observations(self, before):
        after = self.rng_bytes()
        raw_draws = [row["return_eax"] for row in self.trace.calls if row["name"] == "rng_raw"]
        reference, reference_after = draws(before["main"], len(raw_draws))
        if reference != raw_draws or reference_after != after["main"]:
            raise RuntimeError("Voice draws differ from original raw RNG continuation")
        if any(before[key] != after[key] for key in ("scenario", "mapgen")):
            raise RuntimeError("Selection unexpectedly changed deterministic RNG state")
        return dict(visits=self.visits(), call_order=self.call_order,
                    center_coord=self.center_coords[-1] if self.center_coords else None,
                    redraw=self.redraws, cursor_calls=self.cursor_calls,
                    voice_requests=[dict(id=self.identities[int(row["ecx"], 16)],
                                         sound_id=struct.unpack("<i", struct.pack("<I", row["args"][0]))[0])
                                    for row in self.trace.calls if row["name"] == "queue_voice"],
                    main_draws=raw_draws,
                    queued_voices={str(i): i32(self.uc, p + 0x4F0) for i, p in self.actors.items()},
                    rng_unchanged={name: before[name] == state for name, state in after.items()})

    def set_selection_sources(self, screen, map_order):
        if not self.selection_commands:
            raise RuntimeError("Selection sources require command observations")
        self.put(self.tactical + 0xDB0, len(screen))
        for index, identity in enumerate(screen):
            self.put(0xB0CEC8 + 12 * index, self.actors.get(identity, 0))
        items = self.alloc(max(4, len(map_order) * 4))
        for index, identity in enumerate(map_order):
            self.put(items + 4 * index, self.actors.get(identity, 0))
        self.put(0xA8EC7C, items)
        self.put(0xA8EC88, len(map_order))

    def initialize_cleanup_inputs(self, layers, world_size):
        """Run original startup/empty-Radio regions over supplied actor state.

        The shared effect-world owner executes40B540..40B5AB,
        725850..725886 and4E6D60..4E6D96, stopping before CRT atexit.
        This is initialization, not a full constructor or fatal-world witness.
        """
        from tools.projectile_oracle.ifv_impact import initialize_effect_world
        initialize_effect_world(SimpleNamespace(u=self.uc, read32=self.get), {},
                                seed=self.seed, map_size=world_size, clear_terrain=False)
        self.uc.reg_write(UC_X86_REG_ESP, self.stack)
        run_checked(self.uc, 0x4A8630, 0x4A8672, required_addresses=[0x4A8630])
        self.set_layers(layers)
        # Original Tactical ctor6D1D54 supplies this vptr; actual expiry+28
        # 6DA560 executes on the initialized screen records, without a sink.
        self.put(self.tactical, 0x7F4348)
        for pointer in self.actors.values():
            # Only the original Radio65A750 vector allocation/empty-slot
            # region is run. Calling its whole base ctor over an already
            # supplied Unit would incorrectly overwrite the actor's prestate.
            self.uc.reg_write(UC_X86_REG_ESI, pointer)
            self.uc.reg_write(UC_X86_REG_ESP, self.stack)
            run_checked(self.uc, 0x65A758, 0x65A798, required_addresses=[0x65A758])
        if self.text_hash() != self.code_hash:
            raise RuntimeError("Original .text changed during cleanup initialization")

    def cleanup_snapshot(self):
        pointer = self.get(0xB0FE6C)
        retained = [] if not pointer else [
            self.identities[self.get(self.get(pointer + 4) + 4 * index)]
            for index in range(self.get(pointer + 16))]
        return dict(**self.selection_snapshot(), retained_navigation=retained,
                    actor_state={str(identity): dict(
                        alive=bool(byte(self.uc, actor_pointer + 0x90)),
                        selected=bool(byte(self.uc, actor_pointer + 0x83)),
                        limbo=bool(byte(self.uc, actor_pointer + 0x81)),
                        marked=bool(byte(self.uc, actor_pointer + 0x74)),
                        native_layer=i32(self.uc, actor_pointer + 0x94))
                        for identity, actor_pointer in self.actors.items()},
                    layer_order=[[
                        self.identities[self.get(self.get(vector + 4) + 4 * index)]
                        for index in range(self.get(vector + 16))]
                        for vector in range(0x8A0360, 0x8A03D8, 24)],
                    screen_order=[self.identities[self.get(0xB0CEC8 + 12 * index)]
                                  for index in range(self.get(self.tactical + 0xDB0))],
                    map_order=[self.identities[self.get(self.get(0xA8EC7C) + 4 * index)]
                               for index in range(self.get(0xA8EC88))])

    def selection_snapshot(self):
        return dict(**self.snapshot(), across_map=bool(byte(self.uc, 0xB0FE64)),
                    submode=byte(self.uc, 0xB0FE58),
                    action_timer=list(struct.unpack("<3i", self.uc.mem_read(ACTION_TIMER, 12))))

    def selection_observations(self, before):
        names = ("screen_techno_gate", "alive_owner_gate", "dynamic_gate",
                 "combatant_final_gate", "select", "object_select", "deselect")
        calls = [dict(name=row["name"], id=self.identities.get(int(row["ecx"], 16)),
                      accepted=bool(row["return_eax"] & 255))
                 for row in self.trace.calls if row["name"] in names]
        return dict(**self.observations(before), selection_calls=calls,
                    message_keys=self.message_keys, messages=self.messages,
                    timer_writes=self.timer_writes,
                    detach_calls=[name for name in self.call_order
                                  if name in ("world_detach", "abstract_detach",
                                              "object_detach_all", "foot_detach_all")])


def search_case(spec):
    fixture = SelectionFixture(spec["actors"], spec["layers"], **spec.get("house", {}))
    before = fixture.rng_bytes()
    pointer = fixture.invoke(SEARCH[spec["direction"]], DISPLAY,
                             [fixture.actors.get(spec["anchor"], 0)])
    return {**spec, "candidate": fixture.identities[pointer],
            "visits": fixture.visits(),
            "rng_unchanged": before == fixture.rng_bytes()}


def command_history(spec):
    fixture = SelectionFixture(spec["actors"], spec["layers"],
                               voice_enabled=spec.get("voice_enabled", True))
    fixture.set_selection(spec["selected"])
    fixture.set_display(follow=spec.get("follow"), modes=spec.get("modes"),
                        placement=spec.get("placement", False))
    fixture.put(0xB0FE54, 3)
    put8(fixture.uc, 0xB0FE58, True)
    steps = []
    for direction in spec["directions"]:
        fixture.reset_observations()
        before = fixture.rng_bytes()
        obj = fixture.objects[COMMAND[direction]]
        entry = fixture.get(fixture.get(obj) + 0x20)
        fixture.invoke(entry, obj, [0])
        steps.append(dict(direction=direction, **fixture.snapshot(),
                          **fixture.observations(before)))
    return {**spec, "steps": steps}


def voice_history(spec):
    value = actor(1, spec["kind"], voice_list=spec["voice_list"],
                  queued_voice=777, owner=spec.get("owner", "local"),
                  slave=spec.get("slave", False),
                  robot_offline=spec.get("robot_offline", False))
    fixture = SelectionFixture([value], [[], [], [1], [], []],
                               seed=spec["seed"],
                               voice_enabled=spec.get("voice_enabled", True))
    if value["slave"]:
        fixture.set_voice_list(1, spec["voice_list"], 0x430)
    if value["robot_offline"]:
        fixture.set_voice_list(1, spec["voice_list"], 0x44C)
    prefix = {}
    if advance := spec.get("advance_main_raw", 0):
        before_prefix = fixture.rng_bytes()
        fixture.reset_observations()
        for _ in range(advance):
            fixture.invoke(0x65C780, fixture.rngs["main"])
        observed = fixture.observations(before_prefix)
        prefix["main_prefix"] = dict(
            draws=observed["main_draws"],
            before_state_hex=before_prefix["main"].hex(),
            after_state_hex=fixture.rng_bytes()["main"].hex(),
        )
        assert len(observed["main_draws"]) == advance
    initial = fixture.rng_bytes()
    steps = []
    for _ in range(spec.get("calls", 4)):
        before = fixture.rng_bytes()
        fixture.reset_observations()
        fixture.invoke(0x708EB0, fixture.actors[1])
        steps.append(fixture.observations(before))
    after = fixture.rng_bytes()
    continuation, _ = draws(after["main"], 4)
    return {**spec, **prefix, "initial_queued_voice": 777, "steps": steps,
            "rng_before_hex": {name: raw.hex() for name, raw in initial.items()},
            "rng_after_hex": {name: raw.hex() for name, raw in after.items()},
            "main_next_four": continuation}


def combatant_history(spec):
    fixture = SelectionFixture(spec["actors"], spec["layers"],
                               selection_commands=True,
                               voice_enabled=spec.get("voice_enabled", True),
                               seed=spec.get("seed", 1), **spec.get("house", {}))
    cleanup = "retained_navigation" in spec
    if spec.get("setup_cleanup", False):
        fixture.initialize_cleanup_inputs(spec["layers"], spec["cleanup_world_size"])
    fixture.set_selection_sources(spec["screen_order"], spec["map_order"])
    fixture.set_selection(spec["selected"])
    fixture.set_display(follow=spec.get("follow"), modes=spec.get("modes"),
                        placement=spec.get("placement", False))
    fixture.put(0xB0FE54, spec.get("selection_mode", 0))
    put8(fixture.uc, 0xB0FE64, spec.get("across_map", False))
    put8(fixture.uc, 0xB0FE58, spec.get("submode", True))
    fixture.put(0xA8B538, spec.get("command_guard", 0))
    if "action_timer" in spec:
        fixture.uc.mem_write(ACTION_TIMER, struct.pack("<3i", *spec["action_timer"]))
    scope_writes = []
    if cleanup:
        # Actual732050 constructs the selected snapshot. AssignmentB0FE6C
        # supplies an earlier Health/Y snapshot surviving while P owns mode1.
        fixture.put(0xB0FE6C, fixture.invoke(0x732050, 0))
        if fixture.cleanup_snapshot()["retained_navigation"] != spec["retained_navigation"]:
            raise RuntimeError("Native selected snapshot differs from supplied cleanup prestate")

        def observe_scope(u, _access, address, size, value, _data):
            scope_writes.append(dict(pc=hex(u.reg_read(UC_X86_REG_EIP)),
                                     address=hex(address), bytes=size, value=value))
        fixture.uc.hook_add(UC_HOOK_MEM_WRITE, observe_scope,
                            begin=0xB0FE54, end=0xB0FE67)
    snapshot = fixture.cleanup_snapshot if cleanup else fixture.selection_snapshot
    initial = fixture.rng_bytes()
    steps = []
    for command in spec["commands"]:
        fixture.reset_observations()
        scope_writes.clear()
        before = fixture.rng_bytes()
        prior = snapshot()
        name, key_word = command["command"], command.get("key_word", 0)
        if name == "ordinary_select":
            fixture.invoke(0x6FBFA0, fixture.actors[command["id"]])
        elif name == "ordinary_deselect":
            fixture.invoke(0x5F44A0, fixture.actors[command["id"]])
        elif name == "unselect_all":
            fixture.invoke(0x6DA740, fixture.tactical)
        elif name == "pointer_expiry":
            fixture.invoke(0x733160, fixture.actors[command["id"]])
        elif name == "object_detach_all":
            fixture.invoke(0x5F5280, fixture.actors[command["id"]], [int(command["all"])])
        elif name == "object_conceal":
            fixture.invoke(0x5F4D30, fixture.actors[command["id"]])
        elif name == "supply_selection_sources":
            # Post-retirement registry membership is an explicit input seam;
            # mapped storage remains. No destructor/free is claimed here.
            fixture.set_selection_sources(command["screen_order"], command["map_order"])
            fixture.set_layers(command["layers"])
        else:
            obj = fixture.objects[dict(combatant="CombatantSelect", type="TypeSelect",
                                       health="HealthNav", next="NextObject",
                                       previous="PreviousObject")[name]]
            entry = fixture.get(fixture.get(obj) + 0x20)
            if name == "type":
                fixture.invoke(entry, obj, [key_word & ~0x800])
                fixture.time_ms += 100
                fixture.invoke(entry, obj, [key_word | 0x800])
            else:
                fixture.invoke(entry, obj, [key_word])
        step = {**command, "key_word": key_word, "before": prior,
                **snapshot(), **fixture.selection_observations(before)}
        if cleanup:
            after = fixture.rng_bytes()
            continuation, _ = draws(after["main"], 4)
            step.update(native_calls=[dict(**row, id=fixture.identities.get(int(row["ecx"], 16)))
                                      for row in fixture.trace.calls],
                        scope_writes=list(scope_writes),
                        rng_before_hex={name: raw.hex() for name, raw in before.items()},
                        rng_after_hex={name: raw.hex() for name, raw in after.items()},
                        main_next_four=continuation)
        steps.append(step)
    after = fixture.rng_bytes()
    continuation, _ = draws(after["main"], 4)
    return {**spec, "steps": steps, "csf_sha256": fixture.csf_sha256,
            "rng_before_hex": {name: raw.hex() for name, raw in initial.items()},
            "rng_after_hex": {name: raw.hex() for name, raw in after.items()},
            "main_next_four": continuation}


def combatant_cleanup_inputs():
    """Six completed cleanup-to-P controls; initialized memory, not fatal AI."""
    for name, alive, setup, commands in (
        ("direct_deselect_then_P", True, False,
         [dict(command="ordinary_deselect", id=20), dict(command="combatant")]),
        ("alivefalse_deselect_expiry_then_P", False, False,
         [dict(command="ordinary_deselect", id=20), dict(command="pointer_expiry", id=20),
          dict(command="combatant")]),
        ("absent_registry_after_deselect_expiry_then_P", False, False,
         [dict(command="ordinary_deselect", id=20), dict(command="pointer_expiry", id=20),
          dict(command="supply_selection_sources", screen_order=[40], map_order=[50, 40],
               layers=[[], [], [40, 50], [], []]), dict(command="combatant")]),
        ("initialized_object_detach_all_true_then_P", True, True,
         [dict(command="object_detach_all", id=20, all=True), dict(command="combatant")]),
        ("initialized_alivefalse_object_detach_all_then_P", False, True,
         [dict(command="object_detach_all", id=20, all=True), dict(command="combatant")]),
        ("initialized_object_conceal_then_P", True, True,
         [dict(command="object_conceal", id=20), dict(command="combatant")]),
    ):
        actors = [actor(20, alive=alive, is_selectable_combatant=True,
                        positive_primary_damage=True, marked=False, native_layer=2),
                  actor(40, "infantry", is_selectable_combatant=True,
                        positive_primary_damage=True, marked=False, native_layer=2),
                  actor(50, is_selectable_combatant=True,
                        positive_primary_damage=True, marked=False, native_layer=2)]
        yield dict(id=name, actors=actors, layers=[[], [], [20, 40, 50], [], []],
                   house=dict(campaign=False, other_human=False, other_control=False),
                   seed=1, screen_order=[40], map_order=[50, 40, 20], selected=[20],
                   follow=20, modes={name: False for name in MODE_FIELDS}, placement=False,
                   selection_mode=1, across_map=True, submode=1, retained_navigation=[20],
                   action_timer=[995, 0, 25], voice_enabled=True, setup_cleanup=setup,
                   cleanup_world_size=[64, 64], commands=commands)


def combatant_inputs():
    def value(identity, kind="unit", **overrides):
        return actor(identity, kind, is_selectable_combatant=overrides.pop("is_selectable_combatant", True),
                     positive_primary_damage=overrides.pop("positive_primary_damage", True),
                     **overrides)

    def row(name, actors=None, screen=(20, 10), map_order=(40, 10, 20),
            selected=(), commands=None, **fields):
        actors = actors if actors is not None else [value(20, "infantry"), value(10), value(40)]
        return dict(id=name, actors=actors, layers=[[], [], [a["id"] for a in actors], [], []],
                    screen_order=list(screen), map_order=list(map_order),
                    selected=list(selected), commands=commands or [dict(command="combatant")],
                    **fields)

    yield row("screen_then_map_then_repeat", commands=[dict(command="combatant") for _ in range(3)])
    yield row("screen_reverse_map_stored_order", screen=(10, 20), map_order=(20, 40, 10),
              commands=[dict(command="combatant") for _ in range(2)])
    yield row("already_all_screen_selected_expands_same_call", selected=(10, 20), follow=20)
    yield row("empty_screen_expands_same_call", screen=())
    yield row("empty_everywhere", actors=[], screen=(), map_order=())
    yield row("noncombatant_screen_expands_to_map", actors=[value(20, is_selectable_combatant=False), value(40)],
              screen=(20,), map_order=(40, 20))
    yield row("screen_null_and_nontechno", actors=[value(20), value(10, techno_cast=False), value(40)],
              screen=(None, 10, 20))
    yield row("map_alive_gate_accepts_nontechno", actors=[value(20, techno_cast=False), value(40)],
              screen=(), map_order=(20, 40))
    for shift in (False, True):
        yield row(f"shift{int(shift)}_retains_noncombatant", actors=[value(20, "infantry"), value(10),
                  value(40, is_selectable_combatant=False)], selected=(40,), follow=40,
                  commands=[dict(command="combatant", key_word=0x100 if shift else 0)])
        yield row(f"shift{int(shift)}_all_screen_selected_expands", selected=(10, 20), follow=20,
                  commands=[dict(command="combatant", key_word=0x100 if shift else 0)])
        yield row(f"placement_refusal_shift{int(shift)}", selected=(40,), follow=40, placement=True,
                  modes={"sell": True},
                  commands=[dict(command="combatant", key_word=0x100 if shift else 0)])
    for name, fields in (
        ("building", dict(kind="building")),
        ("noncombatant", dict(is_selectable_combatant=False)),
        ("alive_false", dict(alive=False)), ("other_owner", dict(owner="other")),
        ("health_zero_alive_true", dict(health=0)),
        ("limbo", dict(limbo=True)), ("not_in_playfield", dict(in_playfield=False)),
        ("undiscovered", dict(discovered=False)), ("not_selectable", dict(selectable=False)),
        ("slave", dict(slave=True)), ("robot_offline", dict(robot_offline=True)),
        ("bunker", dict(bunker=True)), ("locomotor_swap", dict(locomotor_swap=True)),
        ("docked_empty_cell", dict(docked=True)),
        ("docked_building", dict(docked=True, cell_building=True)),
        ("virtual_1d4", dict(virtual_1d4=True)), ("mission_only", dict(mission_only=True)),
    ):
        for kind in ("infantry", "unit") if "kind" not in fields else (fields["kind"],):
            yield row(f"{kind}_{name}_distinguishes_escalation_and_final_select",
                      actors=[value(20, kind, **{k: v for k, v in fields.items() if k != "kind"}), value(40)],
                      screen=(20,), map_order=(40, 20), selected=(40,), follow=40)
    yield row("native_prepend_and_append", actors=[value(20, "infantry"), value(10, positive_primary_damage=False),
              value(40), value(30, positive_primary_damage=False)], screen=(20, 10, 40, 30),
              map_order=(30, 10, 20, 40), selected=(10,))
    yield row("ordinary_selection_resets_mode_then_P_screen", screen=(20,),
              actors=[value(20, "infantry"), value(10), value(40), value(30, is_selectable_combatant=False)],
              commands=[dict(command="combatant"), dict(command="combatant"),
                        dict(command="ordinary_select", id=30), dict(command="unselect_all"),
                        dict(command="combatant")])
    yield row("P_T_P_health_P", actors=[value(20, "infantry", type_id="E1"), value(10, type_id="MTNK"),
              value(40, type_id="MTNK")], screen=(20, 10),
              commands=[dict(command="combatant"), dict(command="combatant"),
                        dict(command="type"), dict(command="combatant"),
                        dict(command="health"), dict(command="combatant")])
    yield row("health_P_T_P", commands=[dict(command="health"), dict(command="combatant"),
              dict(command="type"), dict(command="combatant")])
    yield row("T_map_P_restarts_screen", actors=[value(20, "infantry", type_id="E1"),
              value(10, type_id="MTNK"), value(40, type_id="MTNK")], selected=(20,),
              commands=[dict(command="type"), dict(command="combatant"),
                        dict(command="type"), dict(command="combatant")])
    yield row("health_empty_snapshot_preserves_map_byte", actors=[], screen=(), map_order=(),
              selection_mode=1, across_map=True,
              commands=[dict(command="health"), dict(command="combatant")])
    for campaign, human, control in ((False, False, False), (False, True, False),
                                     (True, False, True), (True, True, False)):
        yield row(f"owner_mode{int(campaign)}_human{int(human)}_control{int(control)}",
                  actors=[value(20, "infantry", owner="other"), value(40)],
                  screen=(20,), map_order=(40, 20), selected=(20,), follow=20,
                  selection_mode=1, across_map=True,
                  house=dict(campaign=campaign, other_human=human, other_control=control))
    for campaign, human in ((False, False), (True, True)):
        yield row(f"shift_P_lone_nonlocal_campaign{int(campaign)}_human{int(human)}",
                  actors=[value(20, "infantry", owner="other"), value(40)],
                  screen=(20,), map_order=(40, 20), selected=(20,), follow=20,
                  selection_mode=1, across_map=True,
                  house=dict(campaign=campaign, other_human=human, other_control=False),
                  commands=[dict(command="combatant", key_word=0x100)])
    yield row("ordinary_deselect_retains_P_map_scope", selected=(20,), follow=20,
              selection_mode=1, across_map=True,
              commands=[dict(command="ordinary_deselect", id=20), dict(command="combatant")])
    yield row("voice_disabled", voice_enabled=False)
    for seed in (0, 1234):
        yield row(f"voice_seed{seed}", seed=seed)
    yield row("inactive_action_timer_preserved", action_timer=[-1, 0, 0],
              commands=[dict(command="combatant"), dict(command="combatant")])
    yield row("voice_empty_and_multiple", actors=[value(20, "infantry", voice_list=[]),
              value(10, voice_list=[101, 202, 303]), value(40)])
    yield row("voice_minus_one_already_spent_draw", actors=[value(20, voice_list=[-1]), value(40)],
              screen=(20,), map_order=(40, 20))
    yield row("guarded_P_preserves_all", selected=(20,), follow=20, selection_mode=3,
              across_map=True, command_guard=1, modes={name: True for name in MODE_FIELDS})
    for mode in MODE_FIELDS:
        yield row(f"P_preserves_{mode}_mode", modes={mode: True}, selected=(40,), follow=40)


def combatant_type_histories():
    """Compose the existing type/INI fixture; do not substitute field results."""
    root = Path(os.environ.get("VERA20K_COMBATANT_INPUTS", "ini"))
    files = ("RULESMD.INI", "LANGRULE.INI", "MPBATTLEMD.INI", "XMP03T4.MAP")
    # Filename case reflects the native source identities while allowing the
    # existing extraction owner to preserve either ordinary filesystem spelling.
    paths = {path.name.upper(): path for path in root.iterdir() if path.is_file()}
    for name in files:
        if name != "LANGRULE.INI" and name not in paths:
            raise RuntimeError(f"Missing physical combatant type layer {name}; set VERA20K_COMBATANT_INPUTS")
    m = BulletReader({}, root)
    m.u.mem_write(0xA8E348, dwords(0x7EB6D4, m.alloc(4096), 1024, 1, 0, 10))
    text_hash = hashlib.sha256(bytes(m.u.mem_read(0x401000, 0x3E0000))).hexdigest()
    constructors = []

    def construct(name, entry):
        typ = m.alloc(0x2000)
        m.u.mem_write(typ, b"\xa5" * 0x2000)
        writes = []
        def observe(u, _access, address, size, value, _data):
            if address <= typ + 0xDBC < address + size:
                writes.append(dict(pc=hex(u.reg_read(UC_X86_REG_EIP)),
                                   offset=address-typ, bytes=size, value=value))
        hook = m.u.hook_add(UC_HOOK_MEM_WRITE, observe)
        try:
            m.invoke(entry, typ, (m.cstring(name),))
        finally:
            m.u.hook_del(hook)
        constructors.append(dict(type_id=name, entry=hex(entry), poison_byte=165,
                                 value_before=165, value_after=byte(m.u, typ+0xDBC),
                                 vtable=hex(m.read32(typ)), field_writes=writes))
        return typ

    def read(typ, sections):
        m.rules_cache(sections)
        before = bool(byte(m.u, typ+0xDBC))
        m.u.mem_write(SP, dwords(RET_MAGIC, RULES))
        m.u.reg_write(UC_X86_REG_ESP, SP)
        m.u.reg_write(UC_X86_REG_ECX, typ)
        stop = run_checked(m.u, 0x410A60, (0x410A8C, 0x410B7D),
                           count=200000, required_addresses=[0x526810])
        admitted = stop == 0x410A8C
        m.reads = []
        if admitted:
            for register, value in ((UC_X86_REG_ESP, SP), (UC_X86_REG_EBP, typ),
                                    (UC_X86_REG_EBX, typ+0x24), (UC_X86_REG_ESI, RULES)):
                m.u.reg_write(register, value)
            run_checked(m.u, 0x71574E, 0x71576F,
                        count=200000, required_addresses=[0x524EC0, 0x5295F0])
            if m.u.reg_read(UC_X86_REG_ESP) != SP:
                raise RuntimeError("Unbalanced native combatant bool-reader block")
        return dict(before=before, admitted=admitted, after=bool(byte(m.u, typ+0xDBC)),
                    reads=[dict(reader=row["reader"], section=row["section"], key=row["key"],
                                ini="rules") for row in m.reads if row["reader"] == "0x5295f0"])

    retail = []
    for name, entry in (("E1", 0x5236A0), ("E2", 0x5236A0), ("ENGINEER", 0x5236A0),
                        ("MTNK", 0x7470D0), ("HARV", 0x7470D0), ("CMIN", 0x7470D0),
                        ("ROBO", 0x7470D0), ("GACNST", 0x45DD90)):
        typ = construct(name, entry)
        layers = []
        for filename in files:
            if filename not in paths:
                layers.append(dict(file=filename, absent=True))
                continue
            raw = paths[filename].read_bytes()
            physical, _ = lexical(raw, {name})
            sections = {name: {key: value for key, value in physical[name].items()
                               if key == "IsSelectableCombatant"}} if name in physical else {}
            layers.append(dict(file=filename, sha256=hashlib.sha256(raw).hexdigest(),
                               raw_value=sections.get(name, {}).get("IsSelectableCombatant"),
                               sections=sections, **read(typ, sections)))
        retail.append(dict(type_id=name, constructor=False, layers=layers,
                           final=bool(byte(m.u, typ+0xDBC))))
    authored = []
    typ = construct("AUTHORED", 0x710AF0)
    for label, sections in (
        ("yes", {"AUTHORED": {"IsSelectableCombatant": "yes"}}),
        ("missing_key_retains_true", {"AUTHORED": {}}),
        ("missing_section_retains_true", {}),
        ("empty_retains_true", {"AUTHORED": {"IsSelectableCombatant": ""}}),
        ("wrong_key_case_retains_true", {"AUTHORED": {"isselectablecombatant": "no"}}),
        ("wrong_section_case_retains_true", {"authored": {"IsSelectableCombatant": "no"}}),
        ("numeric2_retains_true", {"AUTHORED": {"IsSelectableCombatant": "2"}}),
        ("malformed_retains_true", {"AUTHORED": {"IsSelectableCombatant": "garbage"}}),
        ("no", {"AUTHORED": {"IsSelectableCombatant": "no"}}),
        ("missing_key_retains_false", {"AUTHORED": {}}),
        ("empty_retains_false", {"AUTHORED": {"IsSelectableCombatant": ""}}),
        ("numeric2_retains_false", {"AUTHORED": {"IsSelectableCombatant": "2"}}),
        ("mixed_case_true", {"AUTHORED": {"IsSelectableCombatant": "TrUe"}}),
        ("numeric0_false", {"AUTHORED": {"IsSelectableCombatant": "0"}}),
        ("numeric1_true", {"AUTHORED": {"IsSelectableCombatant": "1"}}),
        ("false", {"AUTHORED": {"IsSelectableCombatant": "false"}}),
    ):
        authored.append(dict(id=label, sections=sections, **read(typ, sections)))
    if text_hash != hashlib.sha256(bytes(m.u.mem_read(0x401000, 0x3E0000))).hexdigest():
        raise RuntimeError("Original .text changed during combatant type execution")
    return dict(key=m.string(0x843414), field_offset=0xDBC, constructors=constructors,
                retail_histories=retail, authored_history=authored)


def search_inputs():
    actors = [actor(i, "infantry" if i in (10, 20, 70) else "unit")
              for i in (90, 80, 40, 10, 30, 20, 60, 70)]
    layers = [[90], [80], [40, None, 10], [30, 20], [60]]
    for direction in SEARCH:
        for anchor in (None, 40, 10, 30, 20, 60, 70):
            yield dict(id=f"layer_order_{direction}_anchor_{anchor}", actors=actors,
                       layers=layers, anchor=anchor, direction=direction)
        for name, overrides in (
            ("health_zero", {"health": 0}), ("health_negative", {"health": -1}),
            ("limbo", {"limbo": True}), ("not_in_playfield", {"in_playfield": False}),
            ("undiscovered", {"discovered": False}), ("not_selectable", {"selectable": False}),
            ("other_owner", {"owner": "other"}), ("slave", {"slave": True}),
            ("robot_offline", {"robot_offline": True}), ("bunker", {"bunker": True}),
            ("locomotor_swap", {"locomotor_swap": True}), ("dead_bit_only", {"alive": False}),
            ("no_techno_cast", {"techno_cast": False}),
            ("docked_empty_cell", {"docked": True}),
            ("docked_building", {"docked": True, "cell_building": True}),
            ("building", {"kind": "building"}),
        ):
            for kind in ("infantry", "unit") if name != "building" else ("building",):
                changed = [actor(40), actor(10, kind, **{k: v for k, v in overrides.items() if k != "kind"}), actor(20)]
                yield dict(id=f"{kind}_{name}_{direction}", actors=changed,
                           layers=[[], [], [40, 10, 20], [], []],
                           anchor=10, direction=direction)
        for name, supplied_layers, supplied_actors in (
            ("empty", [[], [], [], [], []], []),
            ("ignored_layers_only", [[90], [80], [], [], []], actors),
            ("all_ineligible", [[], [], [40, None, 10], [], []],
             [actor(40, health=0), actor(10, "infantry", locomotor_swap=True)]),
            ("singleton", [[], [], [40], [], []], [actor(40)]),
            ("empty_ground", [[], [], [], [30], [60]], actors),
        ):
            yield dict(id=f"{name}_{direction}", actors=supplied_actors,
                       layers=supplied_layers, anchor=40 if name == "singleton" else None,
                       direction=direction)
        for campaign, human, controlled in ((False, True, True), (True, False, False),
                                            (True, True, False), (True, False, True)):
            yield dict(id=f"owner_mode_{int(campaign)}_human{int(human)}_control{int(controlled)}_{direction}",
                       actors=[actor(1, "infantry", owner="other")],
                       layers=[[], [], [1], [], []], anchor=None, direction=direction,
                       house=dict(campaign=campaign, other_human=human, other_control=controlled))


def command_inputs():
    actors = [actor(20, "infantry"), actor(10), actor(30, "infantry")]
    layers = [[], [], [20, 10, 30], [], []]
    yield dict(id="consecutive_next_previous", actors=actors, layers=layers,
               selected=[20], follow=20,
               directions=["next", "next", "next", "previous", "previous", "previous"])
    yield dict(id="multiple_selection_first_anchor", actors=actors, layers=layers,
               selected=[10, 20], follow=20, directions=["next", "previous"])
    yield dict(id="empty_selection_then_repeat", actors=actors, layers=layers,
               selected=[], directions=["previous", "next", "next"])
    yield dict(id="singleton_follow_reselect", actors=[actor(20, "infantry")],
               layers=[[], [], [20], [], []], selected=[20], follow=20,
               directions=["next", "previous"])
    for mode in MODE_FIELDS:
        yield dict(id=f"cancel_{mode}", actors=actors, layers=layers, selected=[20],
                   modes={mode: True}, directions=["next"])
        yield dict(id=f"placement_preserves_{mode}_refuses_selection", actors=actors,
                   layers=layers, selected=[20], follow=20, modes={mode: True},
                   placement=True, directions=["next"])
    yield dict(id="placement_refuses_selection", actors=actors, layers=layers,
               selected=[20], follow=20, placement=True, directions=["next", "previous"])
    yield dict(id="no_candidate_preserves_selection_and_follow", actors=[actor(20, "infantry", health=0)],
               layers=[[], [], [20], [], []], selected=[20], follow=20,
               modes={"repair": True}, directions=["next", "previous"])
    for field in ("mission_only", "virtual_1d4"):
        for kind in ("unit", "infantry"):
            yield dict(id=f"{kind}_search_accepts_select_refuses_{field}",
                       actors=[actor(20, "infantry"), actor(10, kind, **{field: True})],
                       layers=[[], [], [20, 10], [], []], selected=[20], follow=20,
                       directions=["next"])
    yield dict(id="voice_disabled", actors=actors, layers=layers, selected=[20],
               voice_enabled=False, directions=["next"])
    yield dict(id="voice_empty", actors=[actor(20, "infantry", voice_list=[])],
               layers=[[], [], [20], [], []], selected=[], directions=["next"])


def voice_inputs():
    for kind in ("infantry", "unit"):
        for seed in (0, 1, 1234):
            for name, values in (("empty", []), ("single", [101]),
                                 ("multiple", [101, 202, 303])):
                yield dict(id=f"{kind}_{name}_seed{seed}", kind=kind, seed=seed,
                           voice_list=values, calls=4)
        for name, fields in (("disabled_queue", {"voice_enabled": False}),
                             ("other_owner_queue", {"owner": "other"}),
                             ("enslaved_list", {"slave": True}),
                             ("robot_offline_list", {"robot_offline": True})):
            yield dict(id=f"{kind}_{name}", kind=kind, seed=1,
                       voice_list=[101, 202, 303], calls=4, **fields)
        yield dict(id=f"{kind}_minus_one_sound", kind=kind, seed=1,
                   voice_list=[-1], calls=2)
    yield dict(id="unit_multiple_seed1_after248_main_raw", kind="unit", seed=1,
               voice_list=[101, 202, 303], calls=4, advance_main_raw=248)


def generate():
    return dict(schema=1,
                search_cases=[search_case(spec) for spec in search_inputs()],
                command_histories=[command_history(spec) for spec in command_inputs()],
                voice_histories=[voice_history(spec) for spec in voice_inputs()],
                combatant_histories=[combatant_history(spec) for spec in combatant_inputs()],
                combatant_cleanup_histories=[combatant_history(spec)
                                            for spec in combatant_cleanup_inputs()],
                combatant_type_histories=combatant_type_histories())


def metadata():
    result = provenance(scope=__doc__, assumptions=[
        "Actors, types, owners, layers and selection storage are initialized fixture inputs, not original constructor/scenario-load proof. No retail E1/MTNK INI inputs or actual airborne/layer producer are claimed.",
        "Real Unit/Infantry vtables and candidate/selection bodies execute; a Building vtable supplies the RTTI6 exclusion and dock occupier controls only. Tag is NULL, disguise byte1D8 is zero, planning propagationAC4CF4 is disabled and selected-control-group metadata is empty.",
        "Unit6D8 is supplied -1 for the ordinary control branch. Object270 is virtual1D4 warp refusal; Foot6AD is the separate locomotor-swap control. These initialized bits do not prove their lifecycle writers.",
        "Layer vectors are supplied at8A0360; original820030 direction table is read by both search bodies. Layer0/1 exclusion, nulls, absent/ineligible anchors and retained sequence outputs are native observations, not a Python expected scan.",
        "Selection vectorA8ECB8 uses actual static-initializer4E7D40 vtable7E4F64 with supplied capacity64. Consecutive command calls retain the original immediate selection array; no Rust command-delay model is supplied.",
        "Dock controls supply native map-table cells and occupiers; original5F6960/565730/47C520 execute. Building lifecycle, collision and aircraft motion are excluded.",
        "Voice lists contain fixture-relative numeric sound IDs. Native sound lookup, original TechnoType INI readers, OS audio playback and device samples are excluded. Direct VoiceSelect disabled/other-owner controls establish receiver ordering, not reachability through the Select wrapper.",
        "Main886B88, Scenario+218 and MapGenABE890 are initialized with original65C6D0 via sharedgen_rng_vectors.seeded_struct. Complete3F4-byte states bracket voice histories; actual raw draw outputs/continuation are independently replayed with the same original65C780 helper. Scenario and MapGen must remain byte-identical.",
        "The appended unit_multiple_seed1_after248_main_raw history executes 248 original65C780 calls on the same Main886B88 before four708EB0 calls. Prefix draws and full before/after state are observed; no reseeding or cursor replacement occurs between prefix and voices. This supplies an earlier-consumer boundary, not execution of terrain loading, MoveSound, Gattling or death consumers.",
        "Cursor-mode controls execute original power/planning/repair/sell bodies. Planning's saved cursor pointer11BC is null, so its nonempty cursor restoration callback and active planning-group propagation are outside these controls.",
        "Camera4AE290 executes through its singleton/empty reduction; final rendering and full camera bounds/projection are outside the declared6D6070 boundary. Original.text integrity is checked after each invocation.",
        "The appended CombatantSelect histories execute registered5367F0 over original732280, supplied Tactical screen recordsB0CEC8/count+DB0 and retained native Techno vectorA8EC7C/countA8EC88. Screen7342C0, Alive/owner732580, dynamic13C and final7325C0/Select14C execute unchanged. Map/display membership producers, selected Tag actions, disguise and robot power/voice lifecycle are not claimed.",
        "P mode1 and T mode2 share the single across-map byteB0FE64; Health mode3 and the empty-snapshot mode0 retain that byte. The next differing command performs its own native mode-based scope reset. The prior selection_across_map output keeps its historical B0FE58 submode meaning; appended across_map and submode expose both actual bytes separately.",
        "Real ObjectSelect5F4520 prepends positive-PrimaryC9C objects and appends others. Scan/Select/voice order and final CurrentObjects order are separately observed. InvertedShift is passed by the registered P/Health wrappers; direct ordinary_select invokes the existing TechnoSelect body, not a mouse gesture.",
        "The two Shift+P lone-nonlocal histories start mode1/maptrue, selected/follow20. Original6DA770 removes20 even with Shift, leaves map scope, clears follow and selects40 only; campaign human1EC/control1EDfalse and noncampaign controls both require the same final CurrentObjects membership.",
        "The direct ordinary_deselect step invokes original5F44A0 on local20 from selected/follow20, mode1/maptrue. Deselect clears selected membership/follow but retains mode1 and B0FE64; following P therefore starts across-map. OrdinarySelect and UnselectAll remain distinct mode-reset witnesses.",
        "P success invokes VoiceSelect for every eligible newly selected object while822CF2 remains enabled. Shift's already-selected refusals consume no draw. Empty and-minus1 voice lists and global disabled controls distinguish gate/admission. Every step checks original Main draw continuation and byte-identical Scenario/MapGen; full RNG bytes and next4 original Main draws bracket each new history.",
        "P armed power/planning/repair/sell modes, placement, follow and all12B0EA80 action-timer bytes are observed. Timer-write hooks and known action-timer/Detach entries stay empty in this untagged P path. OrdinarySelect and UnselectAll histories are reached original selection-reset witnesses, not a full native scenario replacement or save/load execution.",
        "Full original InfantryType5236A0, UnitType7470D0, BuildingType45DD90 and TechnoType710AF0 constructors overwrite A5-poisoned+DBC with0 at71164B. Physical RULESMD, optional LANGRULE, MPBATTLEMD and XMP03T4 layers are supplied through existing BulletReader lexical/cache owners; original410A60 section admission and71574E..71576F current-default Bool read/store execute. Full unrelated type reader/Rules scenario chronology and physical file IO are not claimed.",
        "Appended type fields use original524EC0 string accessor, exact IsSelectableCombatant843414 and ReadBool5295F0. Physical layer hashes and authored missing/empty/wrongcase/malformed/numeric retained-default histories are native outputs. All original.text bytes stay unchanged.",
        "The six combatant_cleanup_histories retain mode1/B0FE64true across actual Deselect5F44A0, direct pointer-expiry733160 and bounded whole ObjectDetach5F5280(true)/ObjectConceal5F4D30, then execute registered5367F0. Present Alivefalse/selectedfalse and supplied registry omission are separate explicit inputs. Subsequent P includes offscreen50 despite available unselected screen40. Rust pending membership has no native flag counterpart.",
        "Cleanup retainedB0FE6C input is constructed by original732050 from selected20 then assigned as supplied prior Health/Y state while P owns mode1. Full per-step RNG bytes/next4, actor state, actual layer/screen/map membership, scope/action-timer writes and original native nested calls are recorded; no Python mutation/comparator/RNG algorithm supplies reference outputs.",
        "Whole cleanup controls reuse ifv_impact.initialize_effect_world's original startup regions40B540..40B5AB,725850..725886,4E6D60..4E6D96 with supplied64x64 dimensions and seed1. Original4A8630..4A8672 initializes display vectors and65A758..65A798 allocates empty Radio slots over supplied actors. Tactical ctor6D1D54 vptr7F4348 is supplied; actual6DA560 executes. CRT atexit registrations are outside setup regions.",
        "Cleanup services/listeners, Bomb/Temporal/Team, Tags/spawner and radio contacts are empty. Units are unmarked, Type234false excludes Conceal LogicRemove, actual DisplayRemove executes. Alivefalse is prestate, not an executed death writer; absent membership is supplied, mapped object storage remains. Full fatal AI, UnInit/Limbo/Mark, destructor/free/deferred-drain and nonempty ancillary cleanup are not demonstrated.",
        "Original MainTick55D360 calls GScreenInput4F4320 at55D8AB and ProcessCommand55DEE0 at55D8B4 before Logic55AFB0 at55DC9E. Registered Execute+20 at55E015 reaches P5367F0/732280 and synchronous Select+14C at7324A4. Infantry vtable7EB058+DC/+14C/+150 original bytes are4D9720/6FBFA0/5F44A0; registeredPslot7EB9AC is5367F0. This is original body/caller/data evidence, not a whole fatal-frame execution or an EventClass phase claim.",
    ], substitutions=[
        "Inherited KeyboardFixture allocator7C8E17 returns bounded scratch storage during original registration and the empty planning-slot allocation. Registration stops before533D20 INI file loading. No allocator failure/CRT exit registration claims.",
        "Mouse cursor presentation5BDA80/5BDAA0, Tactical camera application6D6070 and redraw4F42F0 record calls then return.",
        "Appended command vectors reuse bounded allocator transport7C8E17; destructor CRT free7C8B3D returns without reclaiming fixture storage. Gameplay/vector/destructor/selection bodies execute original instructions.",
        "Appended T edges use supplied timeGetTime via original import7E1530 and100ms tap duration. StringTable734E60 returns actual UTF16 strings from physical langmd.mix/ra2md.csf via existing stock_csf; native wcscpy/swprintf execute. Final MessageList5D3BA0 records seven original arguments/text then returns; no GUI paint/timer scheduling is claimed.",
        "Appended type constructor/reader setup inherits existing BulletReader bounded heap, CRT TLS and asset-platform seams. Selected IsSelectableCombatant field readers invoke original INI/string/Bool code; no scalar/gate/field result is substituted.",
    ], entry_points={"next": SEARCH["next"], "previous": SEARCH["previous"],
                     "execute_next": 0x536610, "execute_previous": 0x536A80,
                     "candidate": 0x6F32D0, "dynamic_candidate": 0x6FC030,
                     "select": 0x6FBFA0, "deselect": 0x5F44A0,
                     "voice_select": 0x708EB0, "queue_voice": 0x708D90,
                     "rng_raw": 0x65C780, "rng_seed": 0x65C6D0,
                     "execute_combatant": 0x5367F0, "combatant": 0x732280,
                     "alive_owner": 0x732580, "screen_techno": 0x7342C0,
                     "combatant_final": 0x7325C0, "unselect_if_not_owned": 0x6DA770,
                     "execute_type": 0x5368B0, "type_tap": 0x732950,
                     "execute_health": 0x536950, "health": 0x733380,
                     "reset_mode": 0x731D00, "techno_type_constructor": 0x710AF0,
                     "infantry_type_constructor": 0x5236A0,
                     "unit_type_constructor": 0x7470D0,
                     "building_type_constructor": 0x45DD90,
                     "section_admission": 0x410A60,
                     "combatant_type_reader": 0x71574E, "bool_reader": 0x5295F0,
                     "navigation_pointer_expired": 0x733160,
                     "pointer_expired_dispatch": 0x7258D0,
                     "object_detach_all": 0x5F5280, "object_conceal": 0x5F4D30,
                     "foot_detach_all": 0x4D9720, "display_remove": 0x4A9770,
                     "main_tick": 0x55D360, "screen_input": 0x4F4320,
                     "process_command": 0x55DEE0, "logic": 0x55AFB0})
    result["command"] = "python -m tools.input_oracle.selection_navigation --check"
    result["coverage_counts"] = dict(search_cases=94, command_histories=20,
                                    command_calls=31, voice_histories=29,
                                    voice_calls=112, prefix_raw_calls=248)
    inputs = list(combatant_inputs())
    result["coverage_counts"].update(
        combatant_histories=len(inputs),
        combatant_steps=sum(len(spec["commands"]) for spec in inputs),
        combatant_registered_execute_calls=sum(
            2 if command["command"] == "type" else command["command"] not in ("ordinary_select", "ordinary_deselect", "unselect_all")
            for spec in inputs for command in spec["commands"]),
        combatant_type_constructors=9, combatant_retail_type_histories=8,
        combatant_authored_reader_steps=16)
    cleanup_inputs = list(combatant_cleanup_inputs())
    result["coverage_counts"].update(
        combatant_cleanup_histories=len(cleanup_inputs),
        combatant_cleanup_steps=sum(len(spec["commands"]) for spec in cleanup_inputs),
        combatant_cleanup_registered_execute_calls=sum(
            command["command"] == "combatant"
            for spec in cleanup_inputs for command in spec["commands"]),
        combatant_cleanup_direct_boundaries=sum(
            command["command"] in ("ordinary_deselect", "pointer_expiry",
                                   "object_detach_all", "object_conceal")
            for spec in cleanup_inputs for command in spec["commands"]))
    result["command"] = ("VERA20K_COMBATANT_INPUTS=/path/to/extracted/selection/layers "
                         "python -m tools.input_oracle.selection_navigation --check")
    result["csf_sha256"] = stock_csf()[1]
    return result


if __name__ == "__main__":
    finish_vectors(generate, HERE / "selection_navigation.json", provenance=metadata,
                   source_paths={
                       "selection_navigation": Path(__file__),
                       "native_oracle": HERE.parent / "native_oracle.py",
                       "keyboard_bindings": HERE.parent / "storage_oracle/keyboard_bindings.py",
                       "fast_scroll": HERE / "fast_scroll.py",
                       "rng": HERE.parent / "rmg_oracle/gen_rng_vectors.py",
                       "stock": HERE.parent / "sidebar_oracle/stock.py",
                       "type_fixture": HERE.parent / "projectile_oracle/bridge_render_inputs.py",
                       "ini_cache": HERE.parent / "rules_oracle/bridge_anim_inputs.py",
                       "heap": HERE.parent / "rules_oracle/bridge_anim_lists.py",
                       "ini_fixture": HERE.parent / "spatial_oracle/building_body_rules.py",
                       "key_crc": HERE.parent / "projectile_oracle/flat_art.py",
                       "cleanup_startup": HERE.parent / "projectile_oracle/ifv_impact.py",
                   })
