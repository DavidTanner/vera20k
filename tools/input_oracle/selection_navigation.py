"""Original N/M search, selection lifecycle and VoiceSelect RNG controls.

Initialized actors/layers are fixture inputs, not a constructor or scenario-load
claim. Search and selection predicates execute unchanged. See the adjacent md.
"""

from copy import deepcopy
from functools import lru_cache
from pathlib import Path
import hashlib
import struct

from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_ESP

from tools.native_oracle import (
    RET_MAGIC, SCRATCH, NativeCallTrace, finish_vectors, provenance, run_checked,
)
from tools.input_oracle.fast_scroll import (
    DISPLAY, byte, i32, put8, return_from_sink,
)
from tools.rmg_oracle.gen_rng_vectors import STRUCT_LEN, draws, seeded_struct
from tools.storage_oracle.keyboard_bindings import KeyboardFixture

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
                 other_control=False, seed=1, voice_enabled=True):
        super().__init__()
        self.uc.mem_map(RET_MAGIC, 0x1000)
        self.next_free = SCRATCH + 0x10000
        self.actors, self.types, self.identities = {}, {}, {0: None}
        self.input_actors = deepcopy(actors)
        self.houses = {name: self.alloc(0x6000) for name in ("local", "other")}
        for house in self.houses.values():
            self.put(house + 0x34, self.alloc(0x200))
        self.put(0xA83D4C, self.houses["local"])
        self.put(0xA8B238, int(not campaign))
        put8(self.uc, self.houses["local"] + 0x1EC, True)
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
            self.uc.mem_write(pointer + 0x9C, struct.pack("<3i", *value["coord"]))
            self.put(pointer + 0x4F0, value["queued_voice"] & 0xFFFFFFFF)
            put8(self.uc, typ + 0x230, value["selectable"])
            put8(self.uc, typ + 0xC9C, True)  # Positive Primary damage input.
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

    def observe(self, uc, pc, _size, _data):
        sp = uc.reg_read(UC_X86_REG_ESP)
        self.trace.returned(pc, sp)
        if pc in SPECS:
            self.trace.entered(pc, sp, SPECS[pc])
            self.call_order.append(SPECS[pc][0])
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
    initial = fixture.rng_bytes()
    steps = []
    for _ in range(spec.get("calls", 4)):
        before = fixture.rng_bytes()
        fixture.reset_observations()
        fixture.invoke(0x708EB0, fixture.actors[1])
        steps.append(fixture.observations(before))
    after = fixture.rng_bytes()
    continuation, _ = draws(after["main"], 4)
    return {**spec, "initial_queued_voice": 777, "steps": steps,
            "rng_before_hex": {name: raw.hex() for name, raw in initial.items()},
            "rng_after_hex": {name: raw.hex() for name, raw in after.items()},
            "main_next_four": continuation}


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


def generate():
    return dict(schema=1,
                search_cases=[search_case(spec) for spec in search_inputs()],
                command_histories=[command_history(spec) for spec in command_inputs()],
                voice_histories=[voice_history(spec) for spec in voice_inputs()])


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
        "Cursor-mode controls execute original power/planning/repair/sell bodies. Planning's saved cursor pointer11BC is null, so its nonempty cursor restoration callback and active planning-group propagation are outside these controls.",
        "Camera4AE290 executes through its singleton/empty reduction; final rendering and full camera bounds/projection are outside the declared6D6070 boundary. Original.text integrity is checked after each invocation.",
    ], substitutions=[
        "Inherited KeyboardFixture allocator7C8E17 returns bounded scratch storage during original registration and the empty planning-slot allocation. Registration stops before533D20 INI file loading. No allocator failure/CRT exit registration claims.",
        "Mouse cursor presentation5BDA80/5BDAA0, Tactical camera application6D6070 and redraw4F42F0 record calls then return.",
    ], entry_points={"next": SEARCH["next"], "previous": SEARCH["previous"],
                     "execute_next": 0x536610, "execute_previous": 0x536A80,
                     "candidate": 0x6F32D0, "dynamic_candidate": 0x6FC030,
                     "select": 0x6FBFA0, "deselect": 0x5F44A0,
                     "voice_select": 0x708EB0, "queue_voice": 0x708D90,
                     "rng_raw": 0x65C780, "rng_seed": 0x65C6D0})
    result["command"] = "python -m tools.input_oracle.selection_navigation --check"
    return result


if __name__ == "__main__":
    finish_vectors(generate, HERE / "selection_navigation.json", provenance=metadata)
