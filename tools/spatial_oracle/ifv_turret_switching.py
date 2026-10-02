"""Original IFV turret readers, selected-weapon lifecycle, and draw decisions.

VERA20K_IFV_ASSETS selects a flat physical asset extract. No Rust scalar result
initializes these native comparisons. --write publishes; --check reproduces.
Native executable IO, rasterization and the complete temporal rearm chain are
outside this component corpus; the sidecar declares every supplied boundary.
"""
import hashlib
import os
from pathlib import Path
import struct

from unicorn import UC_HOOK_MEM_WRITE
from unicorn.x86_const import (
    UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX,
    UC_X86_REG_EDI, UC_X86_REG_EDX, UC_X86_REG_EIP, UC_X86_REG_ESI,
    UC_X86_REG_ESP,
)
from tools.native_oracle import (
    RET_MAGIC, finish_vectors, provenance, run_checked, file_span, image_bytes,
)
from tools.projectile_oracle.bridge_render_inputs import lexical
from tools.rules_oracle.bridge_anim_inputs import Reader
from tools.spatial_oracle.building_body_rules import INI, SP, TYPE, dwords

ACTOR, PASSENGER, PASSENGER_TYPE, TEMPORAL = (
    TYPE + 0x6000, TYPE + 0x7000, TYPE + 0x8000, TYPE + 0x9000,
)
SET_GUNNER, RECEIVE_GUNNER, REMOVE_GUNNER = 0x70DC70, 0x746420, 0x7464E0
PAIR_NAMES = [
    'Normal', 'Repair', 'MachineGun', 'Flak', 'Pistol', 'Sniper', 'Shock',
    'Explode', 'BrainBlast', 'RadCannon', 'Chrono', 'TerroristExplode', 'Cow',
    'Initiate', 'Virus', 'YuriPrime', 'Guardian',
]
SPANS = [
    (0x6F2B99, 0x6F2BCA), (0x7110F2, 0x7110F8),
    (0x71136F, 0x71137B), (0x711781, 0x711790),
    (0x71284A, 0x712898), (0x714780, 0x71479A),
    (0x747BBD, 0x747E90), (0x717890, 0x7178A2),
    (0x7178B0, 0x7178BE), (0x70DC70, 0x70DCDE),
    (0x735678, 0x735691), (0x74689B, 0x7468B4),
    (0x746420, 0x7464DC), (0x7464E0, 0x7465A6),
    (0x4DE710, 0x4DE74E), (0x473430, 0x473453), (0x5F8640, 0x5F8696),
    (0x5F8844, 0x5F88AC), (0x73B7A3, 0x73B7CB),
    (0x73B8D9, 0x73B94F), (0x73BC26, 0x73BD79),
    (0x6FA4FB, 0x6FA5BE), (0x6F2E98, 0x6F2E9E),
    (0x6FE4A4, 0x6FE4C8), (0x6FF28F, 0x6FF2BE),
]


def signed(value):
    return struct.unpack('<i', dwords(value))[0]


class Machine(Reader):
    def __init__(self, name='FV'):
        self.phase = 'setup'
        self.reads, self.events, self.writes = [], [], []
        self.supply_rof = False
        self.loader_failure = None
        # The inherited owner installs CRT/TLS and original INI lookup support.
        # A nonexistent directory intentionally supplies no external file IO.
        super().__init__(Path(__file__).with_suffix('.no-assets'), {})
        self.u.hook_add(UC_HOOK_MEM_WRITE, self.on_write)
        self.u.mem_write(TYPE, bytes(0x1000))
        self.u.mem_write(TYPE, dwords(0x7F6218))
        self.u.mem_write(TYPE + 0x24, name.encode('ascii') + b'\0')
        self.u.mem_write(TYPE + 0x1F8, name.encode('ascii') + b'\0')
        self.regs(esi=TYPE, ebx=0, ebp=-1)
        for begin, end in [(0x7110F2, 0x7110F8), (0x71136F, 0x71137B),
                           (0x711781, 0x711790)]:
            run_checked(self.u, begin, end)
        self.constructor = self.type_state()
        self.u.mem_write(ACTOR, dwords(0x7F5C70))
        self.u.mem_write(ACTOR + 0x6C4, dwords(TYPE))
        self.u.mem_write(PASSENGER, dwords(0x7EB058))
        self.u.mem_write(PASSENGER + 0x6C0, dwords(PASSENGER_TYPE))
        self.rof_entries = {
            self.read32(0x7F5C70 + 0x318), self.read32(0x7EB058 + 0x318),
        }
        self.code_before = [bytes(self.u.mem_read(a, b-a)) for a, b in SPANS]

    def regs(self, **values):
        registers = dict(eax=UC_X86_REG_EAX, ebp=UC_X86_REG_EBP,
                         ebx=UC_X86_REG_EBX, ecx=UC_X86_REG_ECX,
                         edi=UC_X86_REG_EDI, edx=UC_X86_REG_EDX,
                         esi=UC_X86_REG_ESI, esp=UC_X86_REG_ESP)
        for key, value in values.items():
            self.u.reg_write(registers[key], value & 0xFFFFFFFF)

    def hook(self, u, pc, size, user):
        if self.phase != 'setup':
            sp = u.reg_read(UC_X86_REG_ESP)
            if pc in (0x5276D0, 0x5295F0):
                self.reads.append(dict(reader=hex(pc),
                    section=self.string(self.read32(sp+4)),
                    key=self.string(self.read32(sp+8)),
                    default=signed(self.read32(sp+12))))
            elif pc == 0x717890:
                self.events.append(dict(event='map_write', index=signed(self.read32(sp+4)),
                                        weapon=signed(self.read32(sp+8))))
            elif pc == SET_GUNNER:
                self.events.append(dict(event='set_gunner', mode=signed(self.read32(sp+4))))
            elif pc == 0x70DCF0:
                self.events.append(dict(event='get_current_turret'))
            elif pc in (0x65C780, 0x65C7E0):
                self.events.append(dict(event='scenario_random', entry=hex(pc)))
                raise AssertionError('unexpected RNG in bounded IFV component')
            elif self.supply_rof and pc in self.rof_entries:
                self.events.append(dict(event='supplied_get_rof',
                    receiver='transport' if u.reg_read(UC_X86_REG_ECX) == ACTOR else 'passenger',
                    slot=signed(self.read32(sp+4)), result=19))
                self.ret(19, 4)
                return
            elif self.phase == 'loader' and pc in (0x5F7A90, 0x5F7DB0):
                part = 'turret' if pc == 0x5F7A90 else 'barrel'
                index = signed(self.read32(sp+8))
                accepted = self.loader_failure != (part, index)
                self.events.append(dict(event='supplied_part_load', part=part, index=index,
                    image=self.string(self.read32(sp+4)), accepted=accepted))
                self.ret(int(accepted), 8)
                return
        super().hook(u, pc, size, user)

    def on_write(self, u, access, address, size, value, data):
        if self.phase == 'setup':
            return
        for owner, base, fields in [
            ('type', TYPE, [0x688, 0x808, 0x80C, 0x810, *range(0x814, 0x85C, 4)]),
            ('transport', ACTOR, [0x124, 0x138, 0x274, 0x2EC, 0x2F4, 0x2F8, 0x3B8]),
            ('passenger', PASSENGER, [0x274, 0x2EC, 0x2F4, 0x3B8]),
            ('temporal', TEMPORAL, [0x24]),
        ]:
            if any(address <= base+field < address+size for field in fields):
                self.writes.append(dict(owner=owner, offset=hex(address-base),
                    pc=hex(u.reg_read(UC_X86_REG_EIP)), size=size, value=signed(value)))

    def reset_trace(self, phase):
        self.phase = phase
        self.reads, self.events, self.writes = [], [], []

    def type_state(self):
        return dict(turrets=list(struct.unpack('<18i', self.u.mem_read(TYPE+0x814, 72))),
                    charge_raw_dword=self.read32(TYPE+0x810),
                    is_charge_turret=bool(self.u.mem_read(TYPE+0x810, 1)[0]),
                    turret_count=signed(self.read32(TYPE+0x808)),
                    weapon_count=signed(self.read32(TYPE+0x80C)),
                    ifv_mode=signed(self.read32(TYPE+0x688)))

    def object_state(self):
        return dict(turret=signed(self.read32(ACTOR+0x124)),
                    weapon=signed(self.read32(ACTOR+0x138)))

    def read_layer(self, sections, label):
        self.make_ini(sections)
        self.reset_trace('reader')
        before = self.type_state()
        self.u.mem_write(SP, dwords(RET_MAGIC, INI))
        self.regs(esp=SP, ecx=TYPE)
        end = run_checked(self.u, 0x410A60, (0x410A8C, 0x410B7D),
                          required_addresses=(0x526810,))
        admitted = end == 0x410A8C
        if admitted:
            self.regs(esp=SP, ebp=TYPE, esi=INI, ebx=TYPE+0x24)
            run_checked(self.u, 0x71284A, 0x712898)
            self.regs(esp=SP, ebp=TYPE, edi=INI, ebx=TYPE+0x24)
            run_checked(self.u, 0x714780, 0x71479A)
            self.regs(esp=SP, esi=TYPE, ebx=INI, ebp=TYPE+0x24, eax=0)
            run_checked(self.u, 0x747BBD, 0x747E90)
            assert self.u.reg_read(UC_X86_REG_ESP) == SP
        assert self.code_before == [bytes(self.u.mem_read(a,b-a)) for a,b in SPANS]
        return dict(name=label, sections=sections, admitted=admitted, before=before,
                    after=self.type_state(), reads=self.reads.copy(),
                    writes=self.writes.copy(), events=self.events.copy())

    def initialize(self, mapping, *, gunner=True, charge=False, count=4):
        self.phase = 'setup'
        self.u.mem_write(TYPE+0x814, dwords(*mapping))
        self.u.mem_write(TYPE+0x805, bytes([gunner]))
        self.u.mem_write(TYPE+0x808, dwords(count))
        self.u.mem_write(TYPE+0x810, dwords(charge))
        self.u.mem_write(ACTOR+0x124, dwords(987))
        self.u.mem_write(ACTOR+0x138, dwords(654))
        self.regs(esp=SP, esi=ACTOR, ebx=0)
        run_checked(self.u, 0x6F2B99, 0x6F2BCA)
        base = self.object_state()
        self.reset_trace('initialize')
        self.regs(esp=SP, esi=ACTOR)
        run_checked(self.u, 0x735678, 0x735691)
        return dict(base_constructor=base, after=self.object_state(),
                    writes=self.writes.copy(), events=self.events.copy())

    def operation(self, entry, args=()):
        self.reset_trace('operation')
        before = self.object_state()
        self.invoke(entry, ACTOR, args)
        return dict(entry=hex(entry), before=before, after=self.object_state(),
                    writes=self.writes.copy(), events=self.events.copy())


def reader_controls():
    pairs = {name+'TurretWeapon': str(i) for i, name in enumerate(PAIR_NAMES)}
    cases = [
        ('missing_section', 'FV', {}), ('empty_section', 'FV', {'FV': {}}),
        ('all_fixed_index_defaults', 'FV', {'FV': pairs}),
        ('wrong_key_case', 'FV', {'FV': {'normalturretweapon': '0', 'normalturretindex': '3'}}),
        ('case_insensitive_fv_gate', 'fv', {'fv': pairs}),
        ('non_fv_gate', 'FVCOPY', {'FVCOPY': pairs}),
        ('duplicate_slot_last_writer', 'FV', {'FV': {
            'NormalTurretWeapon': '4', 'NormalTurretIndex': '3',
            'RepairTurretWeapon': '4', 'RepairTurretIndex': '2',
            'GuardianTurretWeapon': '4', 'GuardianTurretIndex': '1'}}),
        ('signed_index', 'FV', {'FV': {'NormalTurretWeapon': '0', 'NormalTurretIndex': '-1'}}),
    ]
    for value in ['-1', '18', '2147483647', '2147483648', '$10', '2junk', '', 'bogus']:
        cases.append(('signed_ifv_mode_'+repr(value), 'E1', {'E1': {'IFVMode': value}}))
    for value in ['0', '1', '256', '257', '-1']:
        cases.append(('missing_weapon_alias_'+value, 'FV', {'FV': {
            'IsChargeTurret': 'yes', 'GuardianTurretIndex': value}}))
    cases += [
        ('charge_kept_all_weapons_present', 'FV', {'FV': {**pairs, 'IsChargeTurret': 'yes'}}),
        ('invalid_weapon_minus_two_alias', 'FV', {'FV': {'GuardianTurretWeapon': '-2', 'GuardianTurretIndex': '6'}}),
        ('invalid_weapon_eighteen', 'FV', {'FV': {'GuardianTurretWeapon': '18', 'GuardianTurretIndex': '2'}}),
    ]
    rows = []
    for label, name, sections in cases:
        m = Machine(name)
        rows.append(m.read_layer(sections, label))
    m = Machine()
    history = []
    for label, keys in [
        ('base', {'NormalTurretWeapon': '0', 'NormalTurretIndex': '3', 'IFVMode': '6'}),
        ('mode', {'RepairTurretWeapon': '0'}),
        ('map_no_section', None),
        ('map_section_missing_keys', {'Other': 'yes'}),
        ('map_normal_index_missing', {'NormalTurretWeapon': '0'}),
    ]:
        history.append(m.read_layer({} if keys is None else {'FV': keys}, label))
    return dict(constructor=Machine().constructor, rows=rows, layer_history=history)


def lifecycle(mapping):
    rows = []
    for charge in [False, True]:
        for mode in [*range(18), -1, -2, 18, 255, 2147483647, -2147483648]:
            m = Machine()
            initial = m.initialize(mapping, charge=charge)
            m.u.mem_write(PASSENGER_TYPE+0x688, dwords(mode))
            receive = m.operation(RECEIVE_GUNNER, [PASSENGER])
            remove = m.operation(REMOVE_GUNNER, [PASSENGER])
            # Failed vehicle placement re-adopts the removed passenger.
            retry = m.operation(RECEIVE_GUNNER, [PASSENGER])
            rows.append(dict(mode=mode, charge=charge, initialization=initial,
                             receive=receive, remove=remove, retry=retry))
    controls = []
    for gunner, count in [(False, 4), (True, 0), (True, -1)]:
        m = Machine()
        initial = m.initialize(mapping, gunner=gunner, count=count)
        m.regs(esp=SP, esi=ACTOR)
        m.reset_trace('init_from_type')
        run_checked(m.u, 0x74689B, 0x7468B4)
        controls.append(dict(gunner=gunner, count=count, initialization=initial,
                             init_from_type=m.object_state(), events=m.events.copy(),
                             remove_null=m.operation(REMOVE_GUNNER, [0])))
    # IsGattling belongs to consumers, not the selector's write gate.
    m = Machine()
    initial = m.initialize(mapping)
    m.u.mem_write(TYPE+0xCD5, b'\x01')
    gattling = dict(initialization=initial, select=m.operation(SET_GUNNER, [6]))
    temporal = []
    for running in [False, True]:
        m = Machine()
        m.initialize(mapping)
        m.u.mem_write(0xA8ED84, dwords(200))
        m.u.mem_write(PASSENGER_TYPE+0x688, dwords(10))
        m.u.mem_write(PASSENGER+0x274, dwords(TEMPORAL))
        m.u.mem_write(TEMPORAL+0x24, dwords(PASSENGER, 0))
        m.u.mem_write(PASSENGER+0x2EC, dwords(190, 0, 20 if running else 0))
        m.supply_rof = True
        receive = m.operation(RECEIVE_GUNNER, [PASSENGER])
        remove = m.operation(REMOVE_GUNNER, [PASSENGER])
        temporal.append(dict(running=running, supplied_rof=19, receive=receive, remove=remove))
    cargo = []
    for gunner in [False, True]:
        for count in [0, 1, 2]:
            m = Machine()
            m.initialize(mapping, gunner=gunner)
            m.operation(SET_GUNNER, [6])
            second = PASSENGER+0x300
            m.u.mem_write(ACTOR+0x114, dwords(count, PASSENGER if count else 0))
            m.u.mem_write(PASSENGER+0x30, dwords(second if count == 2 else 0))
            result = m.operation(0x4DE710)
            cargo.append(dict(gunner=gunner, initial_count=count, result=result,
                removed_first=m.u.reg_read(UC_X86_REG_EAX) == PASSENGER,
                count=signed(m.read32(ACTOR+0x114)),
                head='second' if m.read32(ACTOR+0x118) == second else 'none',
                removed_next_is_null=m.read32(PASSENGER+0x30) == 0))
    return dict(mapping=mapping, rows=rows, initialization_controls=controls,
                gattling_selector=gattling, temporal_boundary=temporal, cargo_removal=cargo)


def draw_decisions():
    rows = []
    for count, gattling in [(0, False), (-1, False), (4, False), (4, True)]:
        for turret in [False, True]:
            for index in [-1, 0, 1, 2, 3, 4, 17, 18]:
                m = Machine()
                u = m.u
                m.initialize([-1]*18, count=count)
                u.mem_write(TYPE+0xCA1, bytes([turret]))
                u.mem_write(TYPE+0xCD5, bytes([gattling]))
                u.mem_write(ACTOR+0x124, dwords(index))
                # Every possible supplied part pair in this bounded range is
                # nonnull. No pointer is dereferenced as a voxel by this slice.
                u.mem_write(TYPE+0xB0, dwords(*([0x12345678]*82)))
                m.reset_trace('draw')
                m.regs(esp=SP, ebp=ACTOR, ebx=TYPE)
                admitted = run_checked(u, 0x73B7A3, (0x73B8D9, 0x73B7CB)) == 0x73B8D9
                result = None
                if admitted:
                    m.regs(esp=SP, ebp=ACTOR, ebx=TYPE, esi=0x123456)
                    run_checked(u, 0x73B8D9, 0x73B94F)
                    selected = signed(m.read32(SP+0x1C))
                    key = u.reg_read(UC_X86_REG_ESI)
                    m.regs(esi=selected, ebx=TYPE)
                    run_checked(u, 0x73BD15, 0x73BD1C)
                    pointer_offset = signed(u.reg_read(UC_X86_REG_ECX)-TYPE)
                    result = dict(selected=selected, cache_key=key,
                                  indexed_turret_pointer_offset=pointer_offset,
                                  indexed_consumer=any(event['event'] == 'get_current_turret'
                                                       for event in m.events))
                rows.append(dict(turret_count=count, is_gattling=gattling,
                                 turret=turret, selected_index=index,
                                 gun_branch=admitted, result=result))
    filenames = []
    for index in range(4):
        for part, begin, end in [('turret', 0x5F7A90, 0x5F7AE6),
                                  ('barrel', 0x5F7DB0, 0x5F7E06)]:
            m = Machine()
            m.u.mem_write(SP, dwords(RET_MAGIC, TYPE+0x1F8, index))
            m.regs(esp=SP, ecx=TYPE)
            run_checked(m.u, begin, end)
            output = m.string(m.u.reg_read(UC_X86_REG_ESP)+0xEC)
            filenames.append(dict(index=index, part=part, native_basename=output))
    return dict(rows=rows, filenames=filenames)


def loader_decisions():
    rows = []
    for count, gattling, turret, failure in [
        (4, False, True, None), (4, False, False, None),
        (4, True, True, None), (4, True, False, None),
        (0, False, True, None), (-1, False, True, None),
        (1, False, True, None), (18, False, True, None),
        (4, False, True, ('turret', 2)), (4, False, True, ('barrel', 1)),
        (4, False, False, ('turret', 1)),
    ]:
        m = Machine()
        m.u.mem_write(TYPE+0x808, dwords(count))
        m.u.mem_write(TYPE+0xCD5, bytes([gattling]))
        m.u.mem_write(TYPE+0xCA1, bytes([turret]))
        m.loader_failure = failure
        m.reset_trace('loader')
        m.regs(esp=SP, esi=TYPE, ebx=0)
        turret_boundary = run_checked(m.u, 0x5F8640, (0x5F8844, 0x5F8696))
        # The base TUR asset IO for the plain branch is a supplied boundary.
        # Indexed failure keeps its actual failure byte while barrel loading runs.
        m.regs(esp=SP, esi=TYPE, ebx=0)
        barrel_boundary = run_checked(m.u, 0x5F8844, (0x5F8A60, 0x5F8A6A, 0x5F88AC))
        rows.append(dict(count=count, gattling=gattling, turret=turret,
            supplied_failure=list(failure) if failure else None,
            turret_boundary=hex(turret_boundary), barrel_boundary=hex(barrel_boundary),
            prior_failure=bool(m.u.mem_read(SP+0x13, 1)[0]), events=m.events.copy()))
    return rows


def charge_turrets():
    """Shared indexed consumer: original frame arithmetic, no weapon/ROF port."""
    m = Machine()
    m.regs(esi=ACTOR, ebx=0)
    run_checked(m.u, 0x6F2E98, 0x6F2E9E)
    constructor = signed(m.read32(ACTOR+0x2F8))
    writers = []
    for disk_laser in [False, True]:
        for berserk in [False, True]:
            for rof in [-2147483648, -101, -1, 0, 1, 99, 100, 2147483647]:
                m = Machine()
                m.u.mem_write(ACTOR+0x298, bytes([berserk]))
                m.u.mem_write(0xA8ED84, dwords(200))
                m.regs(esp=SP, esi=ACTOR, eax=rof, edi=0)
                m.reset_trace('fire_rearm_copy')
                begin, end = (0x6FE4A4, 0x6FE4C8) if disk_laser else (0x6FF28F, 0x6FF2BE)
                run_checked(m.u, begin, end)
                writers.append(dict(disk_laser=disk_laser, berserk=berserk,
                    supplied_get_rof=rof, frame=200,
                    charge_delay=signed(m.read32(ACTOR+0x2F8)),
                    timer_start=signed(m.read32(ACTOR+0x2EC)),
                    timer_duration=signed(m.read32(ACTOR+0x2F4)), writes=m.writes.copy()))
    base = dict(charge=True, count=4, gattling=False, frame=200, start=200,
                duration=100, charge_delay=100, initial_turret=2, initial_weapon=8)
    cases = [dict(name='ordinary_frame_'+str(frame), frame=frame)
             for frame in [199, 200, 201, 224, 225, 226, 249, 250, 251,
                           274, 275, 276, 299, 300, 301]]
    cases += [dict(name='startup', charge_delay=0, duration=0, initial_turret=-1),
              dict(name='no_charge', charge=False), dict(name='gattling', gattling=True),
              dict(name='zero_count', count=0), dict(name='negative_count', count=-1),
              dict(name='negative_delay', charge_delay=-1),
              dict(name='stopped_timer', start=-1, duration=25),
              dict(name='stopped_negative_timer', start=-1, duration=-25),
              dict(name='running_negative_timer', duration=-25),
              dict(name='future_timer', start=300),
              dict(name='frame_wrap', frame=-2147483648, start=2147483640),
              dict(name='elapsed_signed_wrap', frame=2147483647, start=-10),
              dict(name='product_wrap_negative', count=2147483647, start=-1, duration=2, charge_delay=2),
              dict(name='product_wrap_zero', count=65536, start=-1, duration=65536, charge_delay=100),
              dict(name='single_turret', count=1),
              dict(name='three_turrets_truncation', count=3, duration=99, charge_delay=100)]
    rows = []
    for sparse in cases:
        inputs = {**base, **sparse}
        m = Machine()
        m.u.mem_write(TYPE+0x810, bytes([inputs['charge']]))
        m.u.mem_write(TYPE+0x808, dwords(inputs['count']))
        m.u.mem_write(TYPE+0xCD5, bytes([inputs['gattling']]))
        m.u.mem_write(ACTOR+0x124, dwords(inputs['initial_turret']))
        m.u.mem_write(ACTOR+0x138, dwords(inputs['initial_weapon']))
        m.u.mem_write(ACTOR+0x2EC, dwords(inputs['start'], 0, inputs['duration'], inputs['charge_delay']))
        m.u.mem_write(0xA8ED84, dwords(inputs['frame']))
        m.regs(esp=SP, esi=ACTOR, ebp=0)
        m.reset_trace('charge_ai')
        run_checked(m.u, 0x6FA4FB, 0x6FA5BE)
        rows.append(dict(inputs=inputs, after=m.object_state(),
                         writes=m.writes.copy(), events=m.events.copy()))
    return dict(constructor_delay=constructor, fire_writers=writers, ai_rows=rows)


def physical_inputs():
    root = Path(os.environ.get('VERA20K_IFV_ASSETS', 'target/asset/ifv-turret-switching/extract'))
    files = {p.name.lower(): p for p in root.glob('*') if p.is_file()}
    assert 'rulesmd.ini' in files, 'VERA20K_IFV_ASSETS needs a physical RULESMD.INI'
    m = Machine()
    layers = []
    for file in ['rulesmd.ini', 'langrule.ini', 'mpbattlemd.ini']:
        if file not in files:
            layers.append(dict(file=file, absent=True))
            continue
        raw = files[file].read_bytes()
        sections, lines = lexical(raw, {'FV'})
        layers.append(dict(file=file, bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest(),
                           source_lines=lines, **m.read_layer(sections, file)))
    map_path = Path(os.environ.get('VERA20K_IFV_MAP', str(root/'ifv-turret-switching-owned.map')))
    assert map_path.is_file(), 'VERA20K_IFV_MAP needs the production observation map INI'
    raw = map_path.read_bytes()
    assert raw.lstrip().startswith((b';', b'[')), 'map input must be INI text, not a MIX archive'
    sections, lines = lexical(raw, {'FV'})
    layers.append(dict(file=map_path.name, bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest(),
                       source_lines=lines, **m.read_layer(sections, 'production_observation_map')))
    infantry = []
    for name in ['E1', 'E2', 'ENGINEER', 'GGI', 'SHK', 'CLEG', 'IVAN']:
        p = Machine(name)
        raw = files['rulesmd.ini'].read_bytes()
        sections, _ = lexical(raw, {name})
        infantry.append(dict(type=name, **p.read_layer(sections, name)))
    assets = []
    for stem in ['fv', 'fvtur', 'fvtur1', 'fvtur2', 'fvtur3',
                 'fvbarl', 'fvbarl1', 'fvbarl2', 'fvbarl3']:
        for ext in ['vxl', 'hva']:
            name = stem+'.'+ext
            path = files.get(name)
            if path is None:
                assets.append(dict(file=name, present_in_extract=False))
            else:
                raw = path.read_bytes()
                assets.append(dict(file=name, present_in_extract=True,
                    bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest(),
                    header24_hex=raw[:24].hex()))
    p = Machine('SREF')
    sref_sections, _ = lexical(files['rulesmd.ini'].read_bytes(), {'SREF'})
    return dict(layers=layers, final_type=m.type_state(), infantry=infantry, assets=assets,
                sref=p.read_layer(sref_sections, 'SREF'))


def generate():
    physical = physical_inputs()
    return dict(schema_version=1, pair_names=PAIR_NAMES, readers=reader_controls(),
                physical=physical, lifecycle=lifecycle(physical['final_type']['turrets']),
                draw=draw_decisions(), loader=loader_decisions(), charge=charge_turrets())


def metadata():
    result = provenance(
        scope='IFV native scalar/mapping readers, ordinary gunner cycle, initialization and bounded voxel draw decisions',
        assumptions=[
            'Original gamemd.exe bytes only. Supplied cached INI section/key indexes contain physical lexical values, never Rust-parsed scalars; original section admission, ReadInt, ReadBool and FV identity compare execute.',
            'Reader executes selected TechnoType blocks then complete FV pair block in original order. Intervening unrelated keys, original archive/INI loading and layer scheduling are not executed. Missing extracted layer is reported, not inferred to contain no override.',
            'Constructor slices execute with original established EBX=0, EBP=-1; supplied initialized Unit/Infantry vtables and type pointers. Unit constructor and InitFromType Gunner suffixes execute separately, not their whole bodies.',
            'Ordinary ReceiveGunner/RemoveGunner execute whole bodies with null TemporalClass. Failed vehicle-unload retry is represented by receive/remove/receive; six additional rows execute FootClass RemoveFirstPassenger and CargoClass head removal. Spatial placement remains outside this component.',
            'Temporal contrasts supply existing TemporalClass with no target. Active GetROF is answered19 and recorded: its real RNG and full temporal scheduling are not claimed. Native rearm arithmetic and ownership writes around that boundary execute.',
            'Draw rows execute admission and cache/index selection slices with supplied nonnull part pointers, then original indexed turret LEA. For the plain branch that LEA is a diagnostic only, not its production consumer. No renderer/raster, shadows, palette or crash parity claim.',
            'Loader rows execute indexed-loop admission, count limits, first-failure exits and the independent Turret flag gate before barrels. Each part-load result and plain TUR asset IO are supplied; filename rows separately execute original sprintf. Final failure cleanup is established statically, not executed here.',
            'Negative/oversized turret or mapping indices intentionally expose native unchecked memory aliasing. Rust must choose and document its safe malformed-mod policy, not mistake these rows for valid asset-table indices.',
            'Charge-turret shared consumer executes complete original6FA4FB..6FA5BE. Fire writer slices start after existing virtual GetROF and consume supplied signed ROF; the existing GetROF RNG/cadence owner is not reimplemented or claimed. Native multiplication, signed division, clamp and timer subtraction execute, including overflow controls.',
        ],
        substitutions=[
            'Existing Reader supplies bump allocator, no-op deallocator and CRT TLS while preparing INI caches; selected scalar/mapping/lifecycle/draw slices use no file or allocation substitution.',
            'Only temporal-boundary rows intercept actual virtual GetROF entries with result19; ordinary lifecycle has no substituted calls or RNG draws.',
            'Loader loop rows answer5F7A90/5F7DB0 with their declared boolean part-load verdict; these rows do not claim native VXL/HVA parsing or archive IO.',
        ],
        entry_points={'set_gunner': SET_GUNNER, 'receive_gunner': RECEIVE_GUNNER,
                      'remove_gunner': REMOVE_GUNNER, 'fv_reader': 0x747BBD,
                      'ifv_mode_reader': 0x714780, 'turret_draw_admission': 0x73B7A3,
                      'turret_draw_key': 0x73B8D9, 'turret_filename': 0x5F7A90,
                      'barrel_filename': 0x5F7DB0})
    original = image_bytes()
    result['original_slices'] = [dict(start=hex(a), end_exclusive=hex(b),
        sha256=hashlib.sha256(file_span(original,a,b-a)[1]).hexdigest(),
        hex=file_span(original,a,b-a)[1].hex()) for a,b in SPANS]
    return result


if __name__ == '__main__':
    finish_vectors(generate, Path(__file__).with_suffix('.json'), provenance=metadata)
