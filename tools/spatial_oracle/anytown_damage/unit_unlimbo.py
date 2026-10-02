"""Original MTNK placement, authored HIGH caller and whole Building ExitObject controls.

Mission remains the constructor, physical-input and runtime boundary owner.
This fixture changes only declared data inputs and observes original returns.
The naval MTNK control deliberately is not a legal retail Ship production case.
"""
from pathlib import Path
import hashlib
import json
import os

from unicorn import UC_HOOK_CODE
from unicorn.x86_const import (
    UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_EBP, UC_X86_REG_ECX,
    UC_X86_REG_EDI, UC_X86_REG_ESI, UC_X86_REG_ESP, UC_X86_REG_EFLAGS,
    UC_X86_REG_FPCW,
)

from tools.native_oracle import (
    NATIVE_SHA256, RET_MAGIC, file_span, finish_vectors, image_bytes, provenance, run_checked,
)
from tools.spatial_oracle.building_body_rules import SP, RULES, dwords
from . import foot_missions
from .mission import HERE, Mission, base

REPO = Path(__file__).resolve().parents[3]
GEOMETRY_CALLS = (0x735180, 0x735210, 0x735230, 0x735250, 0x7352F0)
DIRECT_PCS = (
    0x737BA0, 0x4D7170, 0x6F6CA0, 0x5F4EC0, 0x73F0A0, 0x4D9C10,
    0x55ABF0, 0x4D9C60, 0x4DB810, 0x4D3780, 0x747EB0, 0x5F4F42, 0x5F4FB4,
)
FACTORY_PCS = (
    0x443C60, 0x443C81, 0x443C88, 0x444565, 0x444575, 0x44457B,
    0x44F640, 0x44458C, 0x444592, 0x444400, 0x44440C, 0x444412,
    0x444445, 0x44447A, 0x44459F, 0x4445BD, 0x4445C9, 0x4445D6,
    0x4445E3, 0x4445F0, 0x444971, 0x444979, 0x44497E, 0x444EDE,
    0x444EE6, 0x444EEB, 0x737BA0, 0x4D7170, 0x6F6CA0, 0x5F4EC0,
    0x73F0A0, 0x73F34C, 0x73F405, 0x4DB810, 0x4D3780, 0x5F4F1B,
    0x5F4F42, 0x486840, 0x480A30,
)
RADIO_ENTRIES = {
    0x65ACB0: ('contact0_transmit', 1), 0x65AAA0: ('directed_transmit', 2),
    0x65A970: ('transmit', 3), 0x43C2D0: ('building_receive', 3),
    0x6F4AB0: ('techno_receive', 3), 0x65A820: ('radio_receive', 3),
}
RADIO_BOUNDARIES = {
    0x43CD2B, 0x43CDD4, 0x43CDDD, 0x43CE18, 0x6F4C29, 0x6F4C34,
    0x6F4C41, 0x6F4B8D, 0x6F4C50, 0x6F4BAD, 0x73A936, 0x73A939,
    0x73A93D, 0x73A943,
}
RADIO_RETS = {
    0x65ACCF, 0x65ACD4, 0x65AAB7, 0x65A9E5, 0x65A990, 0x43CE21,
    0x43CE12, 0x6F4C4D, 0x6F4C99, 0x6F4BBE, 0x65A8AA, 0x65A883, 0x6F4E52,
}


class InitialPlacementBoundary(Exception):
    """Retain the constructor result before ordinary Mission placement starts."""


class UnitUnlimboControls(Mission):
    def __init__(self, case, kind):
        assert kind in ('direct', 'factory')
        self.case, self.kind = case, kind
        self.placement_trace, self.factory_trace = [], []
        self.placement_pre, self.placement_rng_pre, self.after_unlimbo = None, None, None
        self.factory_pending = {}
        self.placement_stage_pre = self.stage_after_unlimbo = self.factory_stage_pre = None
        self.producer = 0
        super().__init__()
        self.unit_type_flags_after_ctor = dict(
            small_visceroid=self.u.mem_read(self.typ + 0xE18, 1)[0],
            large_visceroid=self.u.mem_read(self.typ + 0xE19, 1)[0],
        )
        # These are the original geometry static initializers, in the dependency
        # order already used by astar_hills_route, rather than fitted scalars.
        fields = (0xB1CFD0, 0xB1CFD8, 0xB1CFA0, 0xB1D0B8, 0xB1D0AC)
        before_rng = self.rng_bytes()
        self.unit_geometry_startup = dict(
            before={hex(p): bytes(self.u.mem_read(p, 8)).hex() for p in fields},
            original_calls=[],
        )
        for pc in GEOMETRY_CALLS:
            self.m.invoke(pc, 0)
            self.unit_geometry_startup['original_calls'].append(hex(pc))
        assert self.m.read32(0xB1D0B8) == self.m.read32(0x89E7C0)
        assert self.m.read32(0xB1D0AC) > 0
        assert self.rng_bytes() == before_rng
        self.unit_geometry_startup.update(
            after={hex(p): bytes(self.u.mem_read(p, 8)).hex() for p in fields},
            rng_unchanged=True,
        )

    def rng_bytes(self):
        return {k: bytes(self.u.mem_read(p, 1012)) for k, p in self.resident.rngs.items()}

    def actor(self):
        if not self.src:
            return None
        return dict(self.state(), on_bridge=self.u.mem_read(self.src + 0x8C, 1)[0],
                    in_playfield=self.u.mem_read(self.src + 0x3D5, 1)[0])

    def direct_post(self):
        m, u = self.m, self.u
        cell = self.resident.ptrs[87, 50]
        return dict(
            actor=self.state(), on_bridge=u.mem_read(self.src + 0x8C, 1)[0],
            stage_and_timer=[m.read32(self.src + off)
                             for off in (0xF8, 0xFC, 0x100, 0x104, 0x108, 0x10C)],
            occupation=[m.read32(cell + off) for off in (0x124, 0x128, 0x54, 0x58)],
            lists=[hex(m.read32(cell + off)) for off in (0xE4, 0xE8)],
        )

    def capture_unlimbo_return(self):
        # Mission invokes this after original737BA0 returns and before its
        # placement assertion or frame1 Attack command; no native input changes.
        self.after_unlimbo = dict(self.direct_post(), actor=self.actor(),
                                  in_playfield=self.u.mem_read(self.src + 0x3D5, 1)[0])
        self.stage_after_unlimbo = foot_missions.FootMissions.stage_clock_snap(self, self.src)

    def observe(self, u, pc, size, data):
        m, sp = self.m, u.reg_read(UC_X86_REG_ESP)
        if pc == 0x737BA0 and self.phase == 'placement':
            if self.kind == 'factory':
                raise InitialPlacementBoundary()
            cell = self.resident.ptrs[87, 50]
            u.mem_write(0xA8E7AC, dwords(self.case['scenario_init_counter']))
            u.mem_write(0xA8B238, dwords(self.case['actual_game_mode']))
            if self.case['bridge']:
                u.mem_write(cell + 0x140, dwords(m.read32(cell + 0x140) | 0x100))
            u.mem_write(cell + 0x124, dwords(self.case['ground_bits'], self.case['deck_bits']))
            u.mem_write(cell + 0x54, dwords(0, 0))
            u.mem_write(self.src + 0x8C, bytes([self.case['caller_on_bridge']]))
            coord = m.read32(sp + 4)
            if self.case['caller_deck_z']:
                u.mem_write(coord + 8, dwords(m.read32(coord + 8) + 4 * m.read32(0xABDE88)))
            self.placement_rng_pre = self.rng_bytes()
            self.placement_stage_pre = foot_missions.FootMissions.stage_clock_snap(self, self.src)
            self.placement_pre = dict(
                actor=self.state(), on_bridge=u.mem_read(self.src + 0x8C, 1)[0],
                requested_xyz=base.xyz(u, coord), cell_flags=m.read32(cell + 0x140),
                level=u.mem_read(cell + 0x11B, 1)[0],
                lists=[hex(m.read32(cell + off)) for off in (0xE4, 0xE8)],
                raw=[m.read32(cell + off) for off in (0x124, 0x128, 0x54, 0x58)],
            )
        if self.phase == 'placement' and pc in DIRECT_PCS:
            row = dict(pc=hex(pc), this=hex(u.reg_read(UC_X86_REG_ECX)))
            if pc == 0x73F0A0:
                row['args'] = [m.read32(sp + 4 + i * 4) for i in range(5)]
            if pc == 0x5F4F42:
                row['can_enter_return'] = u.reg_read(UC_X86_REG_EAX)
            if pc in (0x5F4FB4, 0x4D3780):
                row.update(xyz=base.xyz(u, self.src + 0x9C),
                           on_bridge=u.mem_read(self.src + 0x8C, 1)[0])
            self.placement_trace.append(row)
        if self.phase.startswith('factory'):
            if pc == 0x737BA0:
                self.factory_stage_pre = foot_missions.FootMissions.stage_clock_snap(self, self.src)
            if pc in self.factory_pending:
                row = self.factory_pending.pop(pc)
                row.update(returned_eax=u.reg_read(UC_X86_REG_EAX),
                           returned_al=u.reg_read(UC_X86_REG_EAX) & 255, after=self.actor())
            if pc in FACTORY_PCS:
                row = dict(pc=hex(pc), ecx=hex(u.reg_read(UC_X86_REG_ECX)),
                           counter=m.read32(0xA8E7AC), actual_game_mode=m.read32(0xA8B238),
                           actor=self.actor())
                if pc in (0x737BA0, 0x5F4EC0):
                    row['args'] = [m.read32(sp + 4 + i * 4) for i in range(2)]
                    row['requested_xyz'] = base.xyz(u, row['args'][0])
                    self.factory_pending[m.read32(sp)] = row
                if pc == 0x73F0A0:
                    row['args'] = [m.read32(sp + 4 + i * 4) for i in range(5)]
                    self.factory_pending[m.read32(sp)] = row
                if pc == 0x5F4F42:
                    row['class_entry_result'] = u.reg_read(UC_X86_REG_EAX)
                self.factory_trace.append(row)
        super().observe(u, pc, size, data)

    def run_direct(self):
        setup_failure = None
        try:
            super().setup(placement_observer=self.capture_unlimbo_return)
        except AssertionError as exc:
            assert self.after_unlimbo is not None
            assert self.placement_result & 255 == 0
            setup_failure = str(exc)
        assert self.after_unlimbo is not None
        after_rng = self.rng_bytes()
        rng = {
            k: dict(unchanged=after_rng[k] == before, before_hex=before.hex(),
                    after_hex=after_rng[k].hex(), before_sha256=hashlib.sha256(before).hexdigest(),
                    after_sha256=hashlib.sha256(after_rng[k]).hexdigest())
            for k, before in self.placement_rng_pre.items()
        }
        code_hash = self.text_hash()
        assert not self.pending, self.pending
        return dict(input=self.case, before=self.placement_pre, after=self.direct_post(),
                    after_unlimbo=self.after_unlimbo, returned_al=self.placement_result & 255,
                    trace=self.placement_trace, rng=rng, native_text_sha256=code_hash,
                    expected_setup_assertion=setup_failure,
                    unit_geometry_startup=self.unit_geometry_startup, inherited_inputs=self.inputs,
                    unit_type_flags_after_ctor=self.unit_type_flags_after_ctor,
                    stage_before=self.placement_stage_pre, stage_after=self.stage_after_unlimbo,
                    placement_events=[e for e in self.events if e.get('phase') == 'placement'])

    def retain_before_initial_placement(self):
        """Keep the original constructor result for a declared caller control."""
        assert self.kind == 'factory'
        try:
            super().setup()
        except InitialPlacementBoundary:
            pass
        else:
            raise AssertionError('Initial placement boundary not reached')

    def run_authored(self):
        """Execute the original Unit reader's HIGH arm and virtual Unlimbo call."""
        self.retain_before_initial_placement()
        self.phase = 'authored_caller'
        m, u = self.m, self.u
        case = self.case
        cell = self.resident.ptrs[87, 50]
        flags = m.read32(cell + 0x140)
        u.mem_write(cell + 0x140, dwords(flags | 0x100 if case['has_bridge']
                                      else flags & ~0x100))
        u.mem_write(cell + 0x124, dwords(case['ground_bits'], case['deck_bits']))
        u.mem_write(0xA8E7AC, dwords(case['scope']))
        u.mem_write(SP + 0x3C, dwords(22400, 12928, 0))
        for register, value in ((UC_X86_REG_ESP, SP), (UC_X86_REG_ESI, self.src),
                                (UC_X86_REG_EAX, case['parsed_high'])):
            u.reg_write(register, value)
        drive = m.read32(self.src + 0x674)
        force_slope = m.read32(m.read32(drive) + 0x50)
        selected = {0x7434EC, 0x7434F3, 0x7434FB, 0x578080, 0x47B3A0,
                    0x74350A, 0x743510, 0x7435C6, 0x7435D0, 0x737BA0,
                    0x4D7170, 0x5F4EC0, 0x5F4F1B, 0x73F0A0, 0x5F4F42,
                    force_slope}
        trace = []
        def observe(_u, pc, _size, _data):
            if pc in selected:
                trace.append(dict(pc=hex(pc), ecx=hex(u.reg_read(UC_X86_REG_ECX)),
                                  counter=m.read32(0xA8E7AC), actor=self.actor(),
                                  eax=u.reg_read(UC_X86_REG_EAX)))
        hook = u.hook_add(UC_HOOK_CODE, observe)
        before_rng, before = self.rng_bytes(), self.actor()
        try:
            run_checked(u, 0x7434EC, 0x743514,
                        required_addresses=(0x7434EC, 0x7434F3))
            assert u.reg_read(UC_X86_REG_ESP) == SP
            pre_unlimbo = dict(
                actor=self.actor(), requested_xyz=base.xyz(u, SP + 0x3C),
                counter=m.read32(0xA8E7AC), bridge_height=m.read32(0xB1D0AC),
                level=u.mem_read(cell + 0x11B, 1)[0], cell_flags=m.read32(cell + 0x140),
                # Preserve the external receipt's field name: this slot is
                # ForceSlope, not a second height or activation operation.
                drive_activate_slot50=hex(force_slope))
            u.reg_write(UC_X86_REG_EBP, 0x80)
            run_checked(u, 0x7435C6, 0x7435D6, count=1000000,
                        required_addresses=(0x7435D0, 0x737BA0, 0x5F4EC0))
        finally:
            u.hook_del(hook)
        assert u.reg_read(UC_X86_REG_ESP) == SP
        after_rng = self.rng_bytes()
        assert before_rng == after_rng
        assert m.read32(0xA8E7AC) == case['scope']
        assert not self.pending, self.pending
        return dict(
            input=case, before=before, pre_unlimbo=pre_unlimbo,
            returned_al=u.reg_read(UC_X86_REG_EAX) & 255, after=self.actor(),
            stage_after=foot_missions.FootMissions.stage_clock_snap(self, self.src),
            raw_after=[m.read32(cell + off) for off in (0x124, 0x128, 0x54, 0x58)],
            lists_after=[hex(m.read32(cell + off)) for off in (0xE4, 0xE8)],
            trace=trace, rng={k:dict(before_hex=v.hex(), after_hex=after_rng[k].hex(),
                                   unchanged=v == after_rng[k]) for k,v in before_rng.items()},
            text_sha256=self.text_hash(), inherited_inputs=self.inputs,
            geometry_startup=self.unit_geometry_startup)

    def prepare_factory(self):
        self.retain_before_initial_placement()
        self.phase = 'setup'
        m, u = self.m, self.u
        self.inputs['factory_type_layers'] = []
        u.mem_write(0xA83C68, dwords(0x7EB6D4, m.alloc(4096), 1024, 1, 0, 10))
        self.producer_type = m.alloc(0x1800)
        m.invoke(0x45DD90, self.producer_type, (m.cstring(self.case['producer_type']),))
        art_raw = (Path(os.environ['VERA20K_SHRAPNEL_INPUTS']) / 'ARTMD.INI').read_bytes()
        art, lines = base.lexical(art_raw, {self.case['producer_type']})
        m.make_ini(art)
        self.inputs['factory_art'] = dict(sha256=hashlib.sha256(art_raw).hexdigest(),
                                         sections=art, lines=lines)
        for name, path in base.layers():
            if not path.exists():
                continue
            sections, lines = base.lexical(path.read_bytes(), {self.case['producer_type']})
            m.rules_cache(sections)
            calls = []
            # Native Naval, Weeder, Refinery/WeaponsFactory and ExitCoord reads.
            for begin, end in ((0x714A63, 0x714A7D), (0x4604B2, 0x4604CC),
                               (0x460A38, 0x460A92), (0x460F9C, 0x460FE2)):
                for reg, value in ((UC_X86_REG_ESP, SP), (UC_X86_REG_EBP, self.producer_type),
                                   (UC_X86_REG_EBX, self.producer_type + 0x24),
                                   (UC_X86_REG_ESI, RULES), (UC_X86_REG_EDI, RULES)):
                    u.reg_write(reg, value)
                run_checked(u, begin, end, count=300000, required_addresses=(begin,))
                assert u.reg_read(UC_X86_REG_ESP) == SP
                calls.append([hex(begin), hex(end)])
            self.inputs['factory_type_layers'].append(dict(
                file=name, sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
                sections=sections, lines=lines, original_slices=calls))
        for reg, value in ((UC_X86_REG_ESP, SP), (UC_X86_REG_EBP, self.producer_type),
                           (UC_X86_REG_EBX, self.producer_type + 0x24),
                           (UC_X86_REG_EDI, self.producer_type + 0x1F8)):
            u.reg_write(reg, value)
        run_checked(u, 0x461225, 0x46125D, count=300000, required_addresses=(0x461225,))
        assert u.reg_read(UC_X86_REG_ESP) == SP
        self.producer = m.alloc(0x1000)
        m.invoke(0x45B1C0, 0)
        m.invoke(0x43B740, self.producer, (self.producer_type, 0))
        u.mem_write(self.producer + 0x21C, dwords(self.house))
        u.mem_write(self.producer + 0x14C, dwords(self.house))
        u.mem_write(self.producer + 0x9C, dwords(*self.case['producer_xyz']))
        u.mem_write(self.producer + 0xAC, dwords(5))
        u.mem_write(self.producer + 0xB4, dwords(-1))
        u.mem_write(self.producer + 0x218, dwords(0))
        u.mem_write(0xA8E7AC, dwords(self.case['scope_start']))
        u.mem_write(0xA8B238, dwords(self.case['actual_game_mode']))
        cell = self.resident.ptrs[87, 50]
        u.mem_write(cell + 0x124, dwords(self.case['ground_bits'], self.case['deck_bits']))
        if self.case.get('bridge'):
            u.mem_write(cell + 0x140, dwords(m.read32(cell + 0x140) | 0x100))
        u.mem_write(0xA8E9A0, bytes([self.case.get('scenario_active', True)]))
        self.producer_snapshot = dict(
            ptr=hex(self.producer), vtable=hex(m.read32(self.producer)),
            type_vtable=hex(m.read32(self.producer_type)), type=self.case['producer_type'],
            exit_coord=base.xyz(u, self.producer_type + 0xEC8),
            foundation=m.read32(self.producer_type + 0xEF0),
            flags={hex(off): u.mem_read(self.producer_type + off, 1)[0]
                   for off in (0x16BB, 0x16BC, 0x16BD, 0xCCE)},
            xyz=base.xyz(u, self.producer + 0x9C),
            slots={hex(slot): hex(m.read32(m.read32(self.producer) + slot))
                   for slot in (0x48, 0xB4, 0x184, 0x1E8, 0x278)},
        )
        self.before = self.actor()
        self.phase = 'factory_call'

    def run_factory(self):
        self.prepare_factory()
        m, u = self.m, self.u
        before_rng = self.rng_bytes()
        eax = m.invoke(0x443C60, self.producer, (self.src, 0))
        after_rng = self.rng_bytes()
        assert not self.factory_pending, self.factory_pending
        assert not self.pending, self.pending
        cell = self.resident.ptrs[87, 50]
        return dict(
            input=self.case, failure=None, eax=eax, before=self.before, after=self.actor(),
            counter_after=m.read32(0xA8E7AC), producer=self.producer_snapshot,
            producer_current=base.i32(u, self.producer + 0xAC),
            producer_queued=base.i32(u, self.producer + 0xB4), trace=self.factory_trace,
            rng={k: dict(before_hex=before.hex(), after_hex=after_rng[k].hex(),
                         unchanged=before == after_rng[k]) for k, before in before_rng.items()},
            pending={}, cell_after=self.resident.snapshot(cell),
            plane_after=[m.read32(cell + off) for off in (0xE4, 0xE8, 0x124, 0x128, 0x54, 0x58)],
            events=[e for e in self.events if e.get('phase', '').startswith('factory')],
            inputs=self.inputs, unit_geometry_startup=self.unit_geometry_startup,
            unit_type_flags_after_ctor=self.unit_type_flags_after_ctor,
            stage_before=self.factory_stage_pre,
            stage_after=foot_missions.FootMissions.stage_clock_snap(self, self.src),
            text_sha256=self.text_hash(),
        )

    def visceroid_reader_receipt(self):
        # Independent data controls before any actor construction. The shared
        # physical Rules cache and original CCINI bool reader remain the owners.
        m, u = self.m, self.u
        controls = [
            dict(name='constructor_missing', initial=[0, 0], passes=[{}]),
            dict(name='retained_true_missing', initial=[1, 1], passes=[{}]),
            dict(name='small_yes_large_no', initial=[0, 0],
                 passes=[dict(SmallVisceroid='yes', LargeVisceroid='no')]),
            dict(name='current_defaults_across_layers', initial=[0, 0],
                 passes=[dict(SmallVisceroid='yes', LargeVisceroid='no'), {},
                         dict(SmallVisceroid='no', LargeVisceroid='true')]),
            dict(name='invalid_retains_current', initial=[1, 0],
                 passes=[dict(SmallVisceroid='maybe', LargeVisceroid='maybe')]),
            dict(name='different_case_key_control', initial=[0, 0],
                 passes=[dict(smallvisceroid='yes', largevisceroid='true')]),
        ]
        before_rng = self.rng_bytes()
        rows = []
        for control in controls:
            u.mem_write(self.typ + 0xE18, bytes(control['initial']))
            passes = []
            for fields in control['passes']:
                before = list(u.mem_read(self.typ + 0xE18, 2))
                m.rules_cache({'MTNK': fields})
                for reg, value in ((UC_X86_REG_ESP, SP), (UC_X86_REG_EDI, self.typ),
                                   (UC_X86_REG_EBP, self.typ + 0x24), (UC_X86_REG_EBX, RULES),
                                   (UC_X86_REG_EAX, u.mem_read(self.typ + 0xE17, 1)[0])):
                    u.reg_write(reg, value)
                run_checked(u, 0x747862, 0x74789C,
                            required_addresses=(0x747862, 0x5295F0))
                assert u.reg_read(UC_X86_REG_ESP) == SP
                passes.append(dict(section='MTNK', fields=fields, before=before,
                                   after=list(u.mem_read(self.typ + 0xE18, 2))))
            rows.append(dict(input=control, passes=passes))
        after_rng = self.rng_bytes()
        assert before_rng == after_rng
        return dict(unit_type_constructor='0x007470D0',
                    constructor_flags=self.unit_type_flags_after_ctor,
                    reader_start='0x00747862', stop_before='0x0074789C',
                    native_keys={hex(p): m.string(p) for p in (0x83CC84, 0x83CC94)},
                    rows=rows,
                    rng={k: dict(before_hex=v.hex(), after_hex=after_rng[k].hex(),
                                 unchanged=v == after_rng[k]) for k, v in before_rng.items()},
                    native_text_sha256=self.text_hash())

    def run_reused_stage(self):
        control = self.case
        self.case = direct_case('ground_clear')
        initial = self.run_direct()
        assert initial['returned_al'] == 1
        self.phase = 'stage_limbo'
        limbo_before = self.actor()
        limbo_stage_before = foot_missions.FootMissions.stage_clock_snap(self, self.src)
        limbo_rng_before = self.rng_bytes()
        limbo_eax = self.m.invoke(0x7440B0, self.src, ())
        limbo_rng_after = self.rng_bytes()
        assert self.u.mem_read(self.src + 0x81, 1)[0] == 1
        limbo = dict(entry='0x007440B0', returned_al=limbo_eax & 255,
                     before=limbo_before, after=self.actor(),
                     stage_before=limbo_stage_before,
                     stage_after=foot_missions.FootMissions.stage_clock_snap(self, self.src),
                     rng={k: dict(before_hex=v.hex(), after_hex=limbo_rng_after[k].hex(),
                                  unchanged=v == limbo_rng_after[k])
                          for k, v in limbo_rng_before.items()})
        self.case = control
        m, u, stage = self.m, self.u, control['poisoned_stage']
        u.mem_write(self.src + 0xF8, dwords(stage['value']))
        u.mem_write(self.src + 0xFC, bytes([stage['changed']]))
        u.mem_write(self.src + 0x100, dwords(stage['start'], stage['aux_raw_u32'],
                                           stage['duration'], stage['rate'], stage['increment']))
        u.mem_write(self.typ + 0xE18, bytes([control['small_visceroid'], control['large_visceroid']]))
        self.frame = control['frame']
        u.mem_write(0xA8ED84, dwords(self.frame))
        before, rng_before = self.actor(), self.rng_bytes()
        rng_before_state = {k: base.sr.rng_state(u, p) for k, p in self.resident.rngs.items()}
        stage_before = foot_missions.FootMissions.stage_clock_snap(self, self.src)
        start, event_start = len(self.placement_trace), len(self.events)
        self.phase = 'placement'
        self.placement_result = m.invoke(0x737BA0, self.src, (self.coord, 0x80))
        self.capture_unlimbo_return()
        rng_after = self.rng_bytes()
        assert not self.pending, self.pending
        return dict(
            input=control, initial_placement=dict(returned_al=initial['returned_al'],
                before=initial['before'], after_unlimbo=initial['after_unlimbo'],
                stage_before=initial['stage_before'], stage_after=initial['stage_after'],
                rng=initial['rng']), limbo=limbo,
            before=before, after=self.actor(), stage_before=stage_before,
            stage_after=self.stage_after_unlimbo, returned_al=self.placement_result & 255,
            rng={k: dict(before_hex=v.hex(), after_hex=rng_after[k].hex(),
                         unchanged=v == rng_after[k]) for k, v in rng_before.items()},
            rng_before=rng_before_state,
            rng_after={k: base.sr.rng_state(u, p) for k, p in self.resident.rngs.items()},
            trace=self.placement_trace[start:], events=self.events[event_start:],
            native_text_sha256=self.text_hash(),
        )

    def radio_endpoint(self, actor):
        m, u = self.m, self.u
        vector, capacity = m.read32(actor + 0xE4), m.read32(actor + 0xE8)
        assert vector and 0 < capacity <= 64
        return dict(
            address=hex(actor), vtable=hex(m.read32(actor)),
            receive_slot_194=hex(m.read32(m.read32(actor) + 0x194)),
            contact0_slot_274=hex(m.read32(m.read32(actor) + 0x274)),
            radio_descriptor_hex=bytes(u.mem_read(actor + 0xE0, 0x10)).hex(),
            contact_capacity=capacity, contacts=[hex(m.read32(vector + 4 * i)) for i in range(capacity)],
            radio_history=[m.read32(actor + off) for off in (0xD4, 0xD8, 0xDC)],
            tether=u.mem_read(actor + 0x418, 1)[0], mission=base.i32(u, actor + 0xAC),
            queued=base.i32(u, actor + 0xB4), handler_state=base.i32(u, actor + 0xBC),
            dispatch=[base.i32(u, actor + off) for off in (0xC8, 0xD0)],
            position=base.xyz(u, actor + 0x9C),
            primary_facing=[m.read32(actor + off) for off in (0x388, 0x38C)],
            turret_facing=[m.read32(actor + off) for off in (0x3A0, 0x3A4)],
            alive=u.mem_read(actor + 0x90, 1)[0], health=base.i32(u, actor + 0x6C),
            limbo=u.mem_read(actor + 0x81, 1)[0], marked=u.mem_read(actor + 0x74, 1)[0],
            in_playfield=u.mem_read(actor + 0x3D5, 1)[0],
        )

    def radio_pair(self):
        return dict(unit=self.radio_endpoint(self.src), producer=self.radio_endpoint(self.producer),
                    actor=self.actor(), scope_counter=self.m.read32(0xA8E7AC),
                    producer_type_flags={name: self.u.mem_read(self.producer_type + off, 1)[0]
                                         for name, off in (('Helipad', 0x16A9), ('UnitRepair', 0x16AB),
                                                           ('WeaponsFactory', 0x16BD))})

    def radio_call(self, label, route='contact0'):
        trace, pending, entries = [], [], dict(RADIO_ENTRIES)
        entries[self.m.read32(self.m.read32(self.src) + 0x194)] = ('unit_receive', 3)
        def observe(u, pc, _size, _data):
            sp = u.reg_read(UC_X86_REG_ESP)
            while pending and pending[-1]['ret'] == pc and pending[-1]['sp'] == sp:
                pending.pop()['row']['returned_eax'] = u.reg_read(UC_X86_REG_EAX)
            if pc in entries:
                kind, count = entries[pc]
                row = dict(kind=kind, pc=hex(pc), this=hex(u.reg_read(UC_X86_REG_ECX)),
                           return_pc=hex(self.m.read32(sp)),
                           args=[self.m.read32(sp + 4 + 4 * i) for i in range(count)])
                trace.append(row)
                pending.append(dict(ret=self.m.read32(sp), sp=sp + 4 + 4 * count, row=row))
            elif pc in RADIO_BOUNDARIES:
                trace.append(dict(kind='original_boundary', pc=hex(pc),
                                  eax=u.reg_read(UC_X86_REG_EAX),
                                  scope_counter=self.m.read32(0xA8E7AC)))
            if pc in RADIO_RETS:
                trace.append(dict(kind='true_ret', pc=hex(pc), eax=u.reg_read(UC_X86_REG_EAX),
                                  return_pc=hex(self.m.read32(sp))))
        self.phase = 'radio_teardown'
        hook = self.u.hook_add(UC_HOOK_CODE, observe)
        before, rng_before, text_before = self.radio_pair(), self.rng_bytes(), self.text_hash()
        cw_before, start = self.u.reg_read(UC_X86_REG_FPCW), len(self.events)
        try:
            if route == 'contact0':
                eax, stop = self.m.invoke(0x65ACB0, self.src, (8,)), RET_MAGIC
            elif route == 'null_sender':
                eax, stop = self.m.invoke(0x43C2D0, self.producer, (0, 8, 0xA8EC30)), RET_MAGIC
            elif route == 'caller_cmp':
                self.u.reg_write(UC_X86_REG_ESP, SP)
                self.u.reg_write(UC_X86_REG_EBP, self.src)
                run_checked(self.u, 0x73A936, 0x73A946, count=100000,
                            required_addresses=(0x73A939, 0x73A93D, 0x73A943, 0x65ACB0,
                                                0x43CD2B, 0x6F4C29, 0x6F4C34, 0x6F4C41))
                eax, stop = self.u.reg_read(UC_X86_REG_EAX), 0x73A946
            else:
                raise AssertionError(route)
        finally:
            self.u.hook_del(hook)
        for frame in pending:
            if frame['ret'] == RET_MAGIC:
                frame['row']['returned_eax'] = eax
        after_rng, after = self.rng_bytes(), self.radio_pair()
        result = dict(
            name=label, route=route, eax=eax, stop_before=hex(stop),
            eflags=self.u.reg_read(UC_X86_REG_EFLAGS), fpcw_before=cw_before,
            fpcw_after=self.u.reg_read(UC_X86_REG_FPCW), before=before, after=after,
            trace=trace, events=self.events[start:],
            rng={k: dict(before_hex=v.hex(), after_hex=after_rng[k].hex(),
                         before_sha256=hashlib.sha256(v).hexdigest(),
                         after_sha256=hashlib.sha256(after_rng[k]).hexdigest(),
                         unchanged=v == after_rng[k]) for k, v in rng_before.items()},
            text_sha256_before=text_before, text_sha256_after=self.text_hash(),
        )
        assert rng_before == after_rng
        assert before['scope_counter'] == after['scope_counter']
        for side in ('unit', 'producer'):
            for field in ('mission', 'queued', 'handler_state', 'dispatch', 'position',
                          'primary_facing', 'turret_facing', 'health', 'alive', 'limbo',
                          'marked', 'in_playfield'):
                assert before[side][field] == after[side][field], (label, side, field)
        return result

    def run_factory_exit_radio(self, route):
        setup = self.run_factory()
        initial = self.radio_pair()
        assert setup['eax'] == 2
        assert initial['producer_type_flags'] == dict(Helipad=0, UnitRepair=0, WeaponsFactory=1)
        assert initial['unit']['contacts'][0] == hex(self.producer)
        assert initial['producer']['contacts'][0] == hex(self.src)
        supplied, preliminary = [], None
        if route == 'absent_contact0':
            pointer = self.m.read32(self.src + 0xE4)
            self.u.mem_write(pointer, dwords(0))
            supplied.append(dict(address=hex(pointer), field='Unit Contacts[0]', value=0,
                                 reason='Explicit caller no-contact boundary; producer retained.'))
            route = 'contact0'
        elif route == 'null_sender_without_contacts':
            preliminary = self.radio_call('clear_before_null_sender')
            assert preliminary['eax'] == 23
            route = 'null_sender'
        primary = self.radio_call(self.case['name'], route)
        if self.case['name'] == 'absent_contact0':
            assert primary['eax'] == 0
            assert primary['after']['producer']['contacts'][0] == hex(self.src)
            assert primary['after']['unit']['tether'] == primary['after']['producer']['tether'] == 1
        else:
            assert primary['eax'] == 23
            assert all(int(v, 16) == 0 for side in ('unit', 'producer')
                       for v in primary['after'][side]['contacts'])
            assert primary['after']['unit']['tether'] == primary['after']['producer']['tether'] == 0
        if route == 'caller_cmp':
            assert primary['eflags'] & 0x40
        row = dict(input=self.case, supplied_post_factory_data=supplied, factory=setup, primary=primary)
        if self.case['name'] == 'healthy_scope0':
            row['already_cleared'] = self.radio_call('already_cleared_contact0')
            assert row['already_cleared']['eax'] == 0
            assert row['already_cleared']['before'] == row['already_cleared']['after']
        if preliminary is not None:
            row['preliminary'] = preliminary
        return row

    def text_hash(self):
        digest = hashlib.sha256(bytes(self.u.mem_read(0x401000, 0x3E0000))).hexdigest()
        assert digest == self.resident.code_hash
        return digest


def direct_case(name, **values):
    case = dict(name=name, scenario_init_counter=0, actual_game_mode=0, bridge=False,
                ground_bits=0, deck_bits=0, caller_on_bridge=False, caller_deck_z=False)
    return dict(case, **values)


def cases():
    direct = [
        direct_case('ground_clear'), direct_case('ground_busy', ground_bits=0x20),
        direct_case('ground_busy_priority', ground_bits=0x20, scenario_init_counter=1),
        direct_case('ground_busy_actual_mode1', ground_bits=0x20, actual_game_mode=1),
        direct_case('bridge_clear_ground_input', bridge=True),
        direct_case('bridge_deck_busy_ground_input', bridge=True, deck_bits=0x20),
        direct_case('bridge_ground_busy_deck_clear_ground_input', bridge=True, ground_bits=0x20),
        direct_case('bridge_clear_deck_input', bridge=True, caller_on_bridge=True, caller_deck_z=True),
        direct_case('bridge_deck_busy_deck_input', bridge=True, deck_bits=0x20,
                    caller_on_bridge=True, caller_deck_z=True),
    ]
    factory_base = dict(producer_type='GAWEAP', producer_xyz=[21888, 12672, 416],
                        scope_start=0, actual_game_mode=0, ground_bits=0, deck_bits=0)
    factory = [
        dict(factory_base, name='land_clear'),
        dict(factory_base, name='land_busy', ground_bits=32),
        dict(factory_base, name='land_busy_mode1', ground_bits=32, actual_game_mode=1),
        dict(factory_base, name='land_busy_scope3', ground_bits=32, scope_start=3),
        dict(factory_base, name='land_refused_scenario_inactive', scenario_active=False),
        dict(factory_base, name='land_refused_scenario_inactive_scope3',
             scenario_active=False, scope_start=3),
        dict(factory_base, name='land_bridge_ground_pose', bridge=True, ground_bits=32),
        dict(factory_base, name='naval_arm_mtnk_control', producer_type='GAYARD',
             producer_xyz=[21888, 12416, 416]),
    ]
    return direct, factory


def stage_cases():
    return [
        direct_case(f'reused_small{int(small)}_large{int(large)}_'
                    f'{"accepted" if bits == 0 else "refused"}',
                    ground_bits=bits, small_visceroid=small, large_visceroid=large, frame=123,
                    poisoned_stage=dict(value=77, changed=1, start=-7,
                                        aux_raw_u32=0x13579BDF, duration=19, rate=-3, increment=5))
        for small in (False, True) for large in (False, True) for bits in (0, 32)
    ]


def radio_cases():
    factory_base = cases()[1][0]
    return [(dict(factory_base, name=name, scope_start=scope), route)
            for name, scope, route in (
                ('healthy_scope0', 0, 'contact0'), ('raised_scope1', 1, 'contact0'),
                ('nested_scope3', 3, 'contact0'), ('native_caller_cmp', 0, 'caller_cmp'),
                ('absent_contact0', 0, 'absent_contact0'),
                ('null_sender_with_contact0', 0, 'null_sender'),
                ('null_sender_without_contacts', 0, 'null_sender_without_contacts'),
            )]


def authored_cases():
    cases = [dict(name=f'high{high}_bridge{int(bridge)}_scope2_raw_busy',
                  parsed_high=high, has_bridge=bridge, scope=2,
                  ground_bits=32, deck_bits=32)
             for high in (0, 1) for bridge in (False, True)]
    cases.extend([
        dict(name='high0_bridge0_scope0_raw_busy', parsed_high=0, has_bridge=False,
             scope=0, ground_bits=32, deck_bits=0),
        dict(name='high1_bridge1_scope0_raw_busy', parsed_high=1, has_bridge=True,
             scope=0, ground_bits=0, deck_bits=32),
    ])
    return cases


def generate():
    direct, factory = cases()
    result = dict(schema_version=1, native_sha256=NATIVE_SHA256,
                  visceroid_reader_receipt=UnitUnlimboControls(
                      direct_case('visceroid_reader'), 'direct').visceroid_reader_receipt(),
                  direct_rows=[], factory_rows=[], stage_rows=[], factory_exit_radio_rows=[],
                  authored_rows=[])
    for case in direct:
        row = UnitUnlimboControls(case, 'direct').run_direct()
        result['direct_rows'].append(row)
        print(f'direct {case["name"]}: original AL{row["returned_al"]}', flush=True)
    for case in factory:
        row = UnitUnlimboControls(case, 'factory').run_factory()
        result['factory_rows'].append(row)
        print(f'factory {case["name"]}: original EAX{row["eax"]}', flush=True)
    for case in stage_cases():
        row = UnitUnlimboControls(case, 'direct').run_reused_stage()
        result['stage_rows'].append(row)
        print(f'stage {case["name"]}: original AL{row["returned_al"]}', flush=True)
    for case, route in radio_cases():
        row = UnitUnlimboControls(case, 'factory').run_factory_exit_radio(route)
        result['factory_exit_radio_rows'].append(row)
        print(f'radio {case["name"]}: original reply{row["primary"]["eax"]}', flush=True)
    for case in authored_cases():
        row = UnitUnlimboControls(case, 'factory').run_authored()
        result['authored_rows'].append(row)
        print(f'authored {case["name"]}: original AL{row["returned_al"]}', flush=True)
    return result


def metadata():
    result = provenance(
        scope='Nine original MTNK Unit/Foot/Techno/Object Unlimbo controls, eight whole '
              'Building443C60 controls, eight original Unit Limbo/reused Stage controls, '
              'six selected UnitType bool-reader histories and seven factory Contact0 '
              'radio8 fixtures (nine radio calls), six original Unit reader HIGH/caller '
              'controls, with physical layered MTNK and selected '
              'producer inputs. Immediate Unit return is distinct from the inherited later command.',
        entry_points=dict(unit_ctor=0x7353C0, unit_unlimbo=0x737BA0, foot_unlimbo=0x4D7170,
                          techno_unlimbo=0x6F6CA0, object_unlimbo=0x5F4EC0,
                          object_scope_gate=0x5F4F1B, unit_can_enter=0x73F0A0,
                          unit_scope_gate=0x73F34C, unit_crate_mode_gate=0x73F405,
                          factory_exit_object=0x443C60, produced_playfield_prewrite=0x443C81,
                          factory_scope_increment=0x444575, factory_unlimbo_call=0x44458C,
                          factory_scope_success=0x444979, factory_scope_failure=0x444EE6,
                          naval_unlimbo_call=0x44440C, factory_exit_coords=0x44F640,
                          cell_ground_coords=0x480A30, building_type_ctor=0x45DD90,
                          building_ctor=0x43B740, foundation_startup=0x45B1C0,
                          unit_limbo=0x7440B0, unit_stage_tail=0x737BF5,
                          unit_type_ctor=0x7470D0, unit_visceroid_reader=0x747862,
                          bool_reader=0x5295F0, unit_stage_rng=0x737C47,
                          contact0_transmit=0x65ACB0, directed_transmit=0x65AAA0,
                          transmit=0x65A970, building_receive=0x43C2D0,
                          building_request8=0x43CD2B, techno_receive=0x6F4AB0,
                          techno_request8=0x6F4C29, techno_untether=0x6F4B8D,
                          techno_break=0x6F4C50, radio_receive=0x65A820,
                          unit_radio_caller=0x73A936, unit_reply_cmp=0x73A943,
                          authored_unit_reader=0x743270, authored_high_start=0x7434EC,
                          authored_high_stop_before=0x743514,
                          authored_unlimbo_start=0x7435C6,
                          authored_unlimbo_stop_before=0x7435D6,
                          map_ground_height=0x578080, unit_type_coord_clamp=0x747EB0,
                          drive_force_slope_slot50=0x4B04D0, drive_force_slope=0x4AFB40,
                          ship_force_slope=0x69F250,
                          read_scenario_scope_raise=0x684685,
                          full_init_scope_raise=0x686B4F,
                          full_init_unit_reader_call=0x687AA7,
                          rmg_scope_raise=0x598A9A, rmg_scope_unwind=0x59934A,
                          startup_scope_clear=0x68691C, startup_scope_restore=0x686945),
        assumptions=[
            'Mission is the sole constructor, physical crop and native callback owner. '
            'Original geometry735180/735210/735230/735250/7352F0 executes first; retained '
            'scalars and all three full1012-byte RNG objects are recorded. No code or '
            'original vtable is modified; original .text retains its measured hash.',
            'Direct inputs supply independent A8E7AC scope and A8B238 actual GameMode, '
            'raw ground/deck planes, HasBridge and caller OnBridge/Z. after_unlimbo is '
            'captured immediately after737BA0 returns; after preserves the historical '
            'whole Mission.setup output, including the frame1 Attack command if admitted.',
            'Factory inputs retain the fresh limbo Unit before the ordinary Mission '
            'placement. Original BuildingType/Building constructors, selected layered '
            'Naval/Weeder/Refinery/WeaponsFactory/ExitCoord reads and original ART '
            'Foundation read run. Producer coordinates, Guard mission and archive0 '
            'are supplied. Whole original443C60 and its native callbacks then execute.',
            'Land controls measure produced+3D5 and the counter0->1->0 or3->4->3 '
            'through Unlimbo, Mark, radio and producer QueueUnload. Inactive Scenario '
            'controls measure original refusal and scope unwind. Counter scope is '
            'distinct from actual GameMode; it is not a session-mode alias.',
            'Original Object5F3993..5F39C0 copies Location+9C/+A0/+A4 from '
            'AC1380/AC1384/AC1388. Static initializer5F38A0 writes all three '
            'default-coordinate words zero and CRT slot814204 points to it. '
            'Those original spans/caller prefixes are saved as static receipts; '
            'complete CRT registration traversal is not executed by this fixture. '
            'The original complete Unit constructor retains Location[0,0,0] '
            'before placement, as recorded in the direct and factory rows.',
            'Stage decoding composes the existing FootMissions.stage_clock_snap '
            'owner. Original UnitType ctor74714F/747155 writes E18/E19=false; '
            'reader747862..74789C uses literal SmallVisceroid/LargeVisceroid and '
            'current-byte defaults through5295F0. Six independent reader controls '
            'retain full RNG and native strings, including later missing keys.',
            'Reused Stage rows perform original first placement and frame1 '
            'Attack command, then complete Unit7440B0 Limbo. Poisoned retained '
            'Stage, four type-flag combinations, frame123 and raw0/32 are explicit '
            'second-call inputs. Full737BA0 is measured. Either flag selects '
            'ScenarioRanged(0,29); original Stage writes are captured, including '
            'retained FC changed/110 increment and raw104 stack padding.',
            'Factory radio rows execute whole443C60 before original Contact0 '
            'Transmit8. Actual Unit/Building contacts and tethers are measured '
            'through nested19/3 sends, original returns and scopes0/1/3. Original '
            '73A936..73A946 caller slice stops after CMP EAX,23; no radio result '
            'or RNG is substituted. Full three RNG objects and .text are unchanged.',
            'Authored controls retain the fresh native Unit, then execute original '
            '7434EC..743514 HIGH preparation and7435C6..7435D6 virtual+D8 caller. '
            'Parsed HIGH writes OnBridge directly. True adds initialized native '
            'B1D0AC to Map578080 ground Z without HasBridge/bridge-walkable admission. '
            'Scope2 admits raw-busy cells; scope0 controls measure actual refusal. '
            'All three full RNG objects and .text are unchanged.',
            'Static original caller spans establish nested ReadScenario/FullInit '
            'scope through authored readers, the distinct live RMG Generate bracket, '
            'and temporary scope0 around PostMapInit starting-unit callbacks. '
            'FullInit6878E6 is early failure unwind, not an ordinary decrement '
            'before Unit loading. The fixture does not execute the full Scenario.',
            'Original Drive slot50/4B04D0 samples the Cell slope and calls '
            'slot7C/4AFB40. That helper and Ship69F250 write only slope/cache/timer '
            'state, not Location or SetHeight. UnitType747EB0 owns max(inputZ, '
            'Map578080 groundZ), before this ForceSlope callback.',
        ],
        substitutions=[
            'Full Scenario, House/map load and producer visual asset loading are '
            'excluded. A supplied human House binds after null-House constructors '
            'with selected valid counters. Producer is not installed into foundation '
            'content lists. Complete factory controller/paid queue scheduling, other '
            'objects and global match phases are outside these controls.',
            'HasBridge and caller deck state are supplied controls, not evidence of '
            'full authored bridge materialization or bridge placement lifetime. '
            'Authored controls supply parsed HIGH EAX, centered XY/Z0 stack input, '
            'ESI actor, EBP facing128, raw planes, HasBridge and scope0/2; complete '
            'Unit line tokenization/atoi and House/type lookup are excluded. '
            'The GAYARD/MTNK '
            'row is deliberately invalid retail naval production: it establishes '
            'the original no-bracket branch control, not Ship/FNPC naval completion.',
            'Inherited physical source-order INI caches, successful heap/file '
            'mappings, OS ASCII/CLSID/COM activation into original Drive, OleRun, '
            'Interlocked/SEH/CRT/wall-clock setup, visual/radar/sound recording '
            'boundaries and cropped map/zone storage remain explicit. Original '
            'connectivity executes; whole-map hierarchy586990 remains inherited. '
            'Original admission, Unlimbo, Mark, radio and queue returns are not substituted.',
            'Native corpus publication is independent of Rust. These bounded '
            'controls do not certify navigation continuation, whole native match, '
            'complete BuildingType loading, complete RNG/audio or bridge parity.',
            'Reused Stage inputs poison an actually reused object but do not '
            'claim a native producer for arbitrary poisoned field values or '
            'MTNK visceroid flags. Reader controls supply lexical cached sections; '
            'full native physical CCINI parsing and Rules chronology are excluded.',
            'Radio NULL-sender and absent-Unit-contact controls are adversarial '
            'boundaries. Original65A970 NULL target resolves Contacts[0] when '
            'present; this does not certify valid reciprocal lifetime for the '
            'absent-contact input. Helipad/UnitRepair distance/repair/docking, '
            'BuildingUnload FSM, earlier/full UnitPerCell admission and later '
            'navigation/harvest/slave/rally/hunt branches are excluded.',
        ],
    )
    result['promotion_sources'] = {
        'direct_probe_sha256': '7e0a5eaa8a89164fa5739c65117205a18649977e9dfa57a9702929aa6c23c1e6',
        'direct_results_sha256': '58be18cc77fb440d8fac8357daad7253b6dfd02bf46d391198eb2343dc89a210',
        'factory_probe_sha256': '63d888881f2a39135e54990a3958e5e3f07c7f1a0dcfd5e2c8ee831e1f36c258',
        'factory_results_sha256': '5287929bd37031a380faca689a94c0f58a0d52a2d080c423d12dd6de93c0d390',
        'factory_radio8_payload_sha256': '96f4f67894e7a358495bc1456e8ddffa207d36beb41f5e3d93f2972fb6704103',
        'factory_radio8_probe_sha256': '863e27c45e6749791d6cfad143a00dbc4c9cd49bc65e686156d711d6799fd06e',
        'factory_radio8_results_sha256': '443a058f1f8e928d70b28297aeeccabea84a3e0b897da42fa1759ee1212974fd',
        'authored_caller_probe_sha256': 'c93b3b32bfaf50e09aee21060803bea90456094aac0c30368ef76468838d9e48',
        'authored_caller_results_sha256': '705dfe7ac278a1ef0a75471a5d9fe4682b11a8de137e8e56dd1f4e02e82ea543',
        'authored_caller_payload_sha256': '24a70d5a8ed2a0ae2c300bd01c92a2053862b36382d986db69e835a966aec88b',
    }
    result['constructor_location_caller_chain'] = dict(
        unit_to_foot='0x007353CE', infantry_to_foot='0x00517A5B',
        aircraft_to_foot='0x00413D2B', building_to_techno='0x0043B74B',
        foot_to_techno='0x004D31EA', techno_to_radio='0x006F2B46',
        radio_to_mission='0x0065A753', mission_to_object='0x005B2DA3',
        terrain_to_object='0x0071BB99', terrain_default_to_object='0x0071BDF9',
        terrain_distinction='Ordinary71BB90 later calls71D000 Unlimbo from its '
                            'supplied Cell; its complete constructor is not a held-limbo proof.',
    )
    result['native_spans'] = {}
    binary = image_bytes()
    for name, address, length in (
        ('factory_exit_object', 0x443C60, 0x1A48), ('factory_exit_coords', 0x44F640, 0x9C),
        ('cell_ground_coords', 0x480A30, 0x4E), ('unit_scope_and_mode', 0x73F34C, 0xD4),
        ('infantry_scope', 0x51C13A, 0xA0), ('producer_naval_read', 0x714A63, 0x1A),
        ('producer_weeder_read', 0x4604B2, 0x1A),
        ('producer_refinery_factory_read', 0x460A38, 0x5A),
        ('producer_exit_coord_read', 0x460F9C, 0x46),
        ('producer_foundation_read', 0x461225, 0x38),
        ('default_coord_initializer', 0x5F38A0, 0x12),
        ('default_coord_crt_slot', 0x814204, 4),
        ('object_constructor_location', 0x5F3993, 0x30),
        ('mission_constructor_prefix', 0x5B2DA0, 8),
        ('radio_constructor_prefix', 0x65A750, 8),
        ('techno_constructor_prefix', 0x6F2B40, 0x0B),
        ('foot_constructor_prefix', 0x4D31E0, 0x0F),
        ('unit_constructor_prefix', 0x7353C0, 0x13),
        ('infantry_constructor_prefix', 0x517A50, 0x10),
        ('aircraft_constructor_prefix', 0x413D20, 0x10),
        ('building_constructor_prefix', 0x43B740, 0x10),
        ('terrain_constructor_prefix', 0x71BB90, 0x0E),
        ('terrain_default_constructor_prefix', 0x71BDF0, 0x0E),
        ('unit_unlimbo', 0x737BA0, 0xE8),
        ('unit_visceroid_ctor_defaults', 0x7470D0, 0xC0),
        ('unit_visceroid_reader', 0x747862, 0x3A),
        ('small_visceroid_key', 0x83CC84, 15), ('large_visceroid_key', 0x83CC94, 15),
        ('contact0_transmit', 0x65ACB0, 0x27), ('directed_transmit', 0x65AAA0, 0x1A),
        ('radio_transmit', 0x65A970, 0x105), ('building_radio_dispatch', 0x43C2D0, 0x28),
        ('building_radio8', 0x43CD2B, 0xF9), ('techno_radio8', 0x6F4C29, 0x27),
        ('techno_untether', 0x6F4B8D, 0x34), ('techno_break_link', 0x6F4C50, 0x4C),
        ('unit_radio_reply_gate', 0x73A936, 0x1A),
        ('building_radio_table', 0x43CE60, 0x3B), ('techno_radio_table', 0x6F4E5C, 0x49),
        ('authored_unit_reader', 0x743270, 0x467),
        ('map_ground_height', 0x578080, 0x77), ('unit_type_coord_clamp', 0x747EB0, 0x61),
        ('read_scenario_raise', 0x68467C, 0x0E),
        ('read_scenario_dispatch', 0x68495B, 0x75),
        ('read_scenario_failure_unwind', 0x684A5F, 0x12),
        ('read_scenario_success_unwind', 0x684B58, 0x14),
        ('full_init_raise', 0x686B35, 0x20), ('full_init_failure', 0x6878C3, 0x61),
        ('full_init_unit_call', 0x687A9B, 0x11), ('full_init_unwind', 0x687C2B, 0x1F),
        ('post_map_startup_scope', 0x686905, 0x46),
        ('rmg_scope_raise', 0x598A84, 0x1C), ('rmg_scope_unwind', 0x59933C, 0x1A),
        ('starting_mcv', 0x5D7030, 0xAE), ('house_start_coords', 0x50DF30, 0xA4),
        ('starting_unlimbo_first_pose', 0x688F3B, 0x69),
        ('starting_unlimbo_fallback_pose', 0x68921A, 0x6F),
        ('drive_force_slope_slot50', 0x4B04D0, 0x25),
        ('drive_force_slope_slot7c', 0x7E7F2C, 4),
        ('drive_force_slope', 0x4AFB40, 0x35), ('ship_force_slope', 0x69F250, 0x35),
    ):
        offset, raw = file_span(binary, address, length)
        if name == 'default_coord_crt_slot':
            assert int.from_bytes(raw, 'little') == 0x5F38A0
        result['native_spans'][name] = dict(address=f'0x{address:08X}', file_offset=offset,
                                           length=length, sha256=hashlib.sha256(raw).hexdigest(),
                                           hex=raw.hex())
    return result


def source_paths():
    # Reuse the existing Mission's transitive inventory, preserving its historical
    # receipt. New outputs bind current source hashes without republishing old ones.
    paths = foot_missions.source_paths()
    for relative in ('tools/spatial_oracle/anytown_damage/unit_unlimbo.py',
                     'src/rules/object_type.rs', 'src/rules/native_processing.rs',
                     'src/sim/stage.rs',
                     'src/sim/cell_kernel.rs', 'src/sim/combat/in_range.rs',
                     'src/sim/movement/navcom.rs', 'src/sim/movement/foot_approach.rs',
                     'src/sim/world/world_spawn.rs', 'src/sim/world/lifecycle.rs',
                     'src/sim/movement/ground_pose.rs', 'src/sim/movement/slope_transition.rs'):
        paths[relative] = REPO / relative
    return paths


def publish(argv=None):
    finish_vectors(generate, HERE / 'unit_unlimbo.json', provenance=metadata,
                   source_paths=source_paths(), argv=argv)
