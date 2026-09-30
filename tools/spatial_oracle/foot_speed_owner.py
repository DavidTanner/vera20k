"""Original Foot speed setter and complete Drive/Ship constructor ownership.

These are supplied CPU frames, not command/gameplay or callback admission proofs.
All executed functions use original bytes, including the common base constructor.
Setter-only rows carry the requested and stored doubles as hex bits, since the
reference JSON cannot hold NaN or infinity.
"""
from pathlib import Path
import struct

from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_ESP
from tools.native_oracle import finish_vectors, provenance, run_checked, RET_MAGIC, SCRATCH
from tools.spatial_oracle.locomotor_track_cursor import OriginalCursor, LOCO, FOOT, SP
from tools.spatial_oracle.map_queries import dwords


def run_setter(family, requested):
    """The setter alone on one requested double, as bits; returns the stored bits."""
    n = OriginalCursor(family)
    n.seed(-1, -1, False, 0)
    u = n.uc
    # Existing owner memory is separate from the object being constructed.
    u.mem_write(FOOT, bytes([0xA5]) * 0x800)
    u.reg_write(UC_X86_REG_ECX, FOOT)
    u.mem_write(SP, dwords(RET_MAGIC) + struct.pack('<Q', requested))
    run_checked(u, 0x4D3710, RET_MAGIC, count=40)
    assert u.reg_read(UC_X86_REG_ESP) == SP + 12
    return n, struct.unpack('<Q', u.mem_read(FOOT + 0x578, 8))[0]


def witness(family, requested):
    n, applied = run_setter(family, struct.unpack('<Q', struct.pack('<d', requested))[0])
    applied = struct.unpack('<d', struct.pack('<Q', applied))[0]
    u = n.uc
    before = bytes(u.mem_read(FOOT, 0x800))

    ctor = 0x4AF540 if family == 'drive' else 0x69EC50
    u.mem_write(LOCO, bytes([0x5A]) * 0x80)
    u.reg_write(UC_X86_REG_ECX, LOCO)
    u.reg_write(UC_X86_REG_ESP, SP)
    u.mem_write(SP, dwords(RET_MAGIC))
    run_checked(u, ctor, RET_MAGIC, count=100, required_addresses=(0x55A6C0,))
    assert u.reg_read(UC_X86_REG_EAX) == LOCO
    assert u.reg_read(UC_X86_REG_ESP) == SP + 4
    assert bytes(u.mem_read(FOOT, 0x800)) == before
    target = struct.unpack('<d', u.mem_read(LOCO + 0x50, 8))[0]
    result = dict(applied=applied, constructor_target=target,
                  constructor_preserves_owner=True)

    if family == 'drive':
        # Supply linked owner and a populated stash; END transfers it without
        # invoking the caller's release or executing a callback.
        interface, output, stash = LOCO + 0x18, SCRATCH + 0x7000, SCRATCH + 0x7100
        u.mem_write(LOCO + 0xC, dwords(FOOT))
        u.mem_write(LOCO + 0x68, dwords(stash))
        u.reg_write(UC_X86_REG_ESP, SP)
        u.mem_write(SP, dwords(RET_MAGIC, interface, output))
        run_checked(u, 0x4AF930, RET_MAGIC, count=30, required_addresses=(0x4AF94D,))
        assert u.reg_read(UC_X86_REG_ESP) == SP + 12
        assert n.ints(output) == [stash] and n.ints(LOCO + 0x68) == [0]
        assert bytes(u.mem_read(FOOT, 0x800)) == before
        result['end_preserves_owner'] = True
    return dict(input=dict(family=family, requested=requested), output=result)


def setter_bits(requested):
    _, applied = run_setter('drive', requested)
    return dict(input=dict(family='setter', requested_bits=f'{requested:016x}'),
                output=dict(applied_bits=f'{applied:016x}'))


# Signed zeros, NaNs, infinities, denormals and the neighbours of 1.0.
SETTER_EDGE_BITS = (
    0x0000000000000000, 0x8000000000000000, 0x7FF8000000000000, 0xFFF8000000000000,
    0x7FF0000000000001, 0x7FF0000000000000, 0xFFF0000000000000, 0x0000000000000001,
    0x8000000000000001, 0x0010000000000000, 0x3FEFFFFFFFFFFFFF, 0x3FF0000000000000,
    0x3FF0000000000001,
)


def generate():
    return [witness(family, requested) for family in ('drive', 'ship')
            for requested in (-0.5, 0.0, 0.25, 0.5, 1.0, 1.25)] + [
        setter_bits(requested) for requested in SETTER_EDGE_BITS]


def metadata():
    return provenance(
        scope='Foot applied-fraction clamp, including non-finite and signed-zero requests, and preservation across complete Drive/Ship construction and successful Drive END',
        assumptions=[
            'Supplied disjoint owner/class memory, finite exactly representable fractions (setter-only rows: any double) and startup x87 state',
            'Constructor global frame/null values use mapped image state; their runtime producers are excluded',
            'END uses supplied owner-link/stash; caller release, active-slot replacement and callback admission are excluded',
        ], substitutions=['No code patches, hooks returning call results, or omitted constructor callees'],
        entry_points={'setter': 0x4D3710, 'drive_constructor': 0x4AF540,
                      'ship_constructor': 0x69EC50, 'common_constructor': 0x55A6C0,
                      'drive_end': 0x4AF930})


if __name__ == '__main__':
    finish_vectors(generate, Path(__file__).with_suffix('.json'), provenance=metadata)
