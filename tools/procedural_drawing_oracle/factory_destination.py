"""Original Building455D50 destination setter and factory Stop prerequisites.

Real Building/BuildingType vtables and unchanged retail instructions execute.
Prepared actor/type/House/map fields bound these comparisons; no Windows game,
whole input queue, native object construction or rendering is claimed here.
"""

from itertools import product
from pathlib import Path

from unicorn import UC_HOOK_CODE, UC_HOOK_MEM_WRITE
from unicorn.x86_const import UC_X86_REG_EDI, UC_X86_REG_ESI, UC_X86_REG_ESP

from tools.native_oracle import finish_vectors, provenance, run_checked
from tools.spatial_oracle.bridge_damage_admission import (
    base, call, words, read32, MEM, SP,
)

ACTOR, TYPE, HOUSE, OLD, NEW, SELECTED = (MEM + offset for offset in (
    0x10000, 0x12000, 0x14000, 0x1A000, 0x1A100, 0x1B000))
PROFILES = {
    'factory': dict(factory=0x28),
    'barracks': dict(factory=0x10),
    'repair': dict(unit_repair=True),
    'cloning': dict(cloning=True),
    'construction_yard': dict(construction_yard=True),
    'ordinary': {},
    'aircraft_factory': dict(factory=2),
}
TARGETS = {'null': 0, 'old': OLD, 'new': NEW}
NAMES = {value: key for key, value in TARGETS.items()}


def execute(case):
    u = base({'flags': 0, 'level': 0})
    profile = PROFILES[case['profile']]
    # Exact original vtables; every virtual actually reached runs native code.
    u.mem_write(ACTOR, words(0x7E3EBC))
    u.mem_write(TYPE, words(0x7E4570))  # constructor45E2CD
    u.mem_write(ACTOR + 0x14, words(1))
    u.mem_write(ACTOR + 0x6C, words(1000))
    u.mem_write(ACTOR + 0x81, bytes([case.get('limbo', False)]))
    u.mem_write(ACTOR + 0x90, bytes([case.get('alive', True)]))
    u.mem_write(ACTOR + 0x9C, words(10 * 256 + 128, 20 * 256 + 128, 0))
    u.mem_write(ACTOR + 0xAC, words(case.get('mission', 5)))
    u.mem_write(ACTOR + 0x218, words(OLD))
    u.mem_write(ACTOR + 0x21C, words(HOUSE))
    u.mem_write(ACTOR + 0x418, bytes([case.get('tethered', False)]))
    u.mem_write(ACTOR + 0x504, words(case.get('emp', 0)))
    u.mem_write(ACTOR + 0x520, words(TYPE))
    u.mem_write(TYPE + 0xEB8, words(profile.get('factory', -1)))
    for offset, key in ((0x16A9, 'unit_repair'), (0x16AC, 'cloning'),
                        (0x16B9, 'construction_yard')):
        u.mem_write(TYPE + offset, bytes([profile.get(key, False)]))
    u.mem_write(HOUSE + 0x1EC, b'\x01')
    u.mem_write(0xA8B238, words(0))
    u.mem_write(0xA83D4C, words(HOUSE))
    u.mem_write(0xA8ECBC, words(SELECTED))
    u.mem_write(0xA8ECC8, words(1))
    u.mem_write(SELECTED, words(ACTOR))
    before = bytes(u.mem_read(ACTOR, 0x1000))
    calls, writes = [], []
    tracked = {0x455D50: 'building_destination', 0x455DA0: 'has_rally_point',
               0x70C610: 'archive_target', 0x709A30: 'techno_destination',
               0x44F5C0: 'building_command_admission', 0x7010D0: 'armed_admission',
               0x65ACE0: 'radio_broadcast'}

    def observe(uc, address, _size, _data):
        if address in tracked:
            calls.append(tracked[address])

    def written(uc, _access, address, size, value, _data):
        if ACTOR <= address < ACTOR + 0x1000:
            writes.append(dict(offset=address - ACTOR, size=size,
                               value=NAMES.get(value, value)))

    u.hook_add(UC_HOOK_CODE, observe)
    u.hook_add(UC_HOOK_MEM_WRITE, written)
    kind = case['kind']
    result = {}
    if kind == 'destination':
        call(u, 0x455D50, ACTOR, (TARGETS[case['requested']], case.get('mode', 1)))
    elif kind == 'stop_event':
        # Event6 actor/token resolution ends at4C74E8. Keep every following
        # actor admission, terrain, radio and destination instruction intact;
        # stop before target clearing4C75F3 or at the refused-event epilogue.
        u.reg_write(UC_X86_REG_ESP, SP)
        u.reg_write(UC_X86_REG_ESI, ACTOR)
        u.reg_write(UC_X86_REG_EDI, 0)
        end = run_checked(u, 0x4C74E8, (0x4C75F3, 0x4C8109), count=20000,
                          context=case)
        result['destination_reached'] = end == 0x4C75F3
    elif kind == 'stop_input':
        # Stop's unchanged loop runs to the native enqueue entry, or the
        # speech/return boundaries. No voice or event return is supplied.
        u.reg_write(UC_X86_REG_ESP, SP)
        end = run_checked(u, 0x730EA0, (0x6FFE00, 0x730F00, 0x730F1C),
                          count=20000, context=case)
        result['event'] = read32(u, u.reg_read(UC_X86_REG_ESP) + 4) if end == 0x6FFE00 else None
    else:
        raise ValueError(kind)
    after = bytes(u.mem_read(ACTOR, 0x1000))
    result.update(archive=NAMES[read32(u, ACTOR + 0x218)], calls=calls,
                  writes=writes, changed_actor_bytes=[i for i, (a, b) in
                                                       enumerate(zip(before, after)) if a != b])
    return dict(input=case, expected=result)


def generate():
    cases = [dict(kind='destination', profile=profile, mission=mission, requested=requested)
             for profile, mission, requested in product(PROFILES, (5, 18, 19), TARGETS)]
    cases += [dict(kind='destination', profile='factory', mission=5,
                   requested='new', mode=0),
              dict(kind='destination', profile='factory', mission=5,
                   requested='null', emp=10)]
    cases += [dict(kind='stop_event', profile=profile, mission=mission)
              for profile, mission in product(PROFILES, (5, 18, 19))]
    cases += [dict(kind='stop_event', profile='factory', mission=5, **extra)
              for extra in ({'tethered': True}, {'limbo': True}, {'alive': False})]
    cases += [dict(kind='stop_input', profile=profile, emp=emp)
              for profile, emp in product(PROFILES, (-1, 0, 10))]
    return dict(cases=[execute(case) for case in cases])


def metadata():
    return provenance(
        scope='Building455D50 complete setter, Event6 admitted actor through null destination, and Stop selected-building input through enqueue admission.',
        assumptions=[
            'Real vtables, prepared live actor/type/House fields, original HasRallyPoint and supplied map cell. Type keys are separately established by the rally input reader corpus.',
            'Event6 token/house resolution is a supplied actor boundary at4C74E8; admitted execution stops before target clear4C75F3, refused execution at4C8109.',
            'Stop input uses one selected building and human House; ends at original enqueue entry6FFE00 or speech/return, excluding queue insertion, voice and full UI lifetime.',
            'Destination matrix covers null, unchanged/new pointer, Selling and Construction missions, every HasRallyPoint arm, ConstructionYard, mode0/1 and an EMP-positive setter control.',
            'No native RNG call, timer write or detach is performed by the Building setter. Event prefix includes the real empty-contact radio broadcast; nonempty contact detach remains its existing owner.',
            'EMP-positive native input refusal is recorded; Rust has no EMP504 lifecycle/producer and its active-retail creation reachability remains unestablished.',
        ],
        substitutions=[],
        entry_points=dict(building_destination=0x455D50, has_rally_point=0x455DA0,
                          archive_target=0x70C610, techno_destination=0x709A30,
                          stop_event_actor=0x4C74E8, stop_input=0x730EA0))


if __name__ == '__main__':
    finish_vectors(generate, Path(__file__).with_suffix('.json'), provenance=metadata,
                   source_paths={'oracle': Path(__file__)})
