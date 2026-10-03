"""Original Walk Scatter and separate ordinary CLEG Cell destination controls."""
from pathlib import Path
import hashlib
import os
import struct
import sys
from tools.native_oracle import finish_vectors, provenance, image_bytes, file_span, NATIVE_SHA256
from tools.spatial_oracle.infantry_source_scatter import query


def generate():
    cases = [
        dict(process_probe=True), dict(power_off=True), dict(warp_in=True), dict(warp_out=True),
        dict(swap_active=True), dict(open_transport=True), dict(bunker=True), dict(mission=7),
        dict(head=[3200, 3200, 0], process_probe=True), dict(moving=True),
        dict(moving=True, power_off=True), dict(moving=True, raw=[[10, 10, 0x1C, 0]]),
        dict(moving=True, raw=[[10, 10, 0x20, 0]]),
        dict(moving=True, blocked_terrain=[[10, 10]]),
        dict(moving=True, cells=[[10, 10, 0, 0x100]], raw=[[10, 10, 0x20, 0]]),
        dict(moving=True, cells=[[10, 10, 0, 0x100]], raw=[[10, 10, 0, 0x20]]),
        dict(moving=True, swap_active=True),
        dict(cells=[[x, y, 0, 0x100] for x, y in
                    [(10, 9), (11, 9), (11, 10), (11, 11), (10, 11), (9, 11), (9, 10), (9, 9)]]),
    ]
    return [query(dict(case, live_entry=True, live_setter=True)) for case in cases]


def teleport_cases():
    """Two Cell requests before Process, including the retained-reservation Stop."""
    common = dict(teleport_destination=True, live_entry=True, live_setter=True,
                  actor=[2752, 2752, 0], doing=3, seed=31, human=True,
                  frame=100, game_speed=1)
    move_a = dict(kind='set_destination', cell=[12, 10], flag=1)
    move_b = dict(kind='set_destination', cell=[13, 10], flag=1)
    rows = [
        dict(name='guard_two_distinct_cells', mission=5, commands=[move_a, move_b]),
        dict(name='guard_same_cell_twice', mission=5, commands=[move_a, move_a]),
        dict(name='guard_move_stop_move', mission=5,
             commands=[move_a, dict(kind='stop_moving'), move_b]),
        dict(name='attack_same_cell_twice', mission=1, commands=[move_a, move_a]),
        dict(name='guard_queued_attack_same_cell_twice', mission=5, queued_mission=1,
             commands=[move_a, move_a]),
        dict(name='guard_queue_attack_between_same_cell_requests', mission=5,
             commands=[move_a, dict(kind='queue_mission', mission=1, start=0), move_a]),
        dict(name='guard_prone_two_cells', mission=5, prone=True, doing=6, commands=[move_a, move_b]),
    ]
    # Dependency controls: resolver deck choice uses physical Z and structural
    # destination flags, independently of the owner's retained OnBridge byte.
    bridge_cells = [[12, 10, 0, 0x100], [13, 10, 0, 0x100]]
    rows += [
        dict(name='bridge_threshold_equal_ground', mission=5, actor=[2752, 2752, 312],
             cells=bridge_cells, commands=[move_a, move_b]),
        dict(name='bridge_threshold_above_deck', mission=5, actor=[2752, 2752, 313],
             cells=bridge_cells, commands=[move_a, move_b]),
        dict(name='on_bridge_below_threshold_ground', mission=5, on_bridge=True,
             cells=bridge_cells, commands=[move_a, move_b]),
        dict(name='on_bridge_above_threshold_deck', mission=5, on_bridge=True,
             actor=[2752, 2752, 313], cells=bridge_cells, commands=[move_a, move_b]),
        dict(name='no_structural_bridge_above_threshold_ground', mission=5, on_bridge=True,
             actor=[2752, 2752, 313], commands=[move_a, move_b]),
    ]
    rows += [
        # The current Cell's supplied vehicle bit is retained through the
        # actual Infantry5217C0 initial mark; snapshots save the resulting raw.
        dict(name='current_vehicle_bit20_guard_two_cells', mission=5,
             raw=[[10, 10, 0x20, 0]], commands=[move_a, move_b]),
        dict(name='current_vehicle_bit20_attack_same_cell', mission=1,
             raw=[[10, 10, 0x20, 0]], commands=[move_a, move_a]),
        dict(name='human_prone_guard_same_reference_up7', mission=5,
             prone=True, doing=6, commands=[move_a, move_a]),
        dict(name='human_prone_attack_same_reference_up7', mission=1,
             prone=True, doing=6, commands=[move_a, move_a]),
        dict(name='destination_full_infantry_slots_refuses', mission=5,
             raw=[[12, 10, 0x1C, 0]], commands=[move_a, move_b]),
        dict(name='destination_vehicle_bit20_refuses', mission=5,
             raw=[[12, 10, 0x20, 0]], commands=[move_a, move_b]),
    ]
    rows += [dict(name=f'human_doing_{doing}_class_early_return', mission=5,
                  doing=doing, class_early_return=True, commands=[move_a, move_b])
             for doing in (27, 28, 29, 30)]
    return [{**common, **row} for row in rows]


def teleport_type_inputs(records):
    """Reuse Mission's constructor, physical cache and original GUID OS owner.

    Whole InfantryType5236A0/5240A0 execute here, before the separate selected
    command fixture. Mission.setup/GeneralRules and the full scenario loader
    are not needed. Asset and OS boundaries remain those of this existing
    controller; no selected command result or type scalar is supplied.
    """
    from tools.spatial_oracle.anytown_damage.mission import Mission
    from tools.spatial_oracle.anytown_damage import mtnk_attack as base
    from tools.spatial_oracle.building_body_rules import RULES, dwords
    mission = Mission()
    m, u = mission.m, mission.u
    art_path = Path(os.environ['VERA20K_SHRAPNEL_INPUTS']) / 'ARTMD.INI'
    art_raw = art_path.read_bytes()
    art, lines = base.lexical(art_raw, {'CLEG', 'ClegSequence'})
    m.make_ini(art)
    for registry in (0xA8E348, 0xA83DE8):
        u.mem_write(registry, dwords(0x7EB6D4, m.alloc(4096), 1024, 1, 0, 10))
    typ = m.alloc(0x1900)

    def fields():
        return dict(name=m.string(typ + 0x24), image=m.string(typ + 0x1F8),
                    vtable=hex(m.read32(typ)), jumpjet=u.mem_read(typ + 0xD94, 1)[0],
                    movement_zone=base.i32(u, typ + 0x5B4), speed_type=base.i32(u, typ + 0x67C),
                    crawls=u.mem_read(typ + 0xEBD, 1)[0], fraidycat=u.mem_read(typ + 0xEBF, 1)[0],
                    deployer=u.mem_read(typ + 0xEAC, 1)[0], strength=base.i32(u, typ + 0xA0),
                    locomotor_guid=bytes(u.mem_read(typ + 0x34C, 16)).hex(),
                    fire_frames=list(struct.unpack('<4i', u.mem_read(typ + 0xE40, 16))))

    def read_records():
        ptr = m.read32(typ + 0xE3C)
        return [list(struct.unpack('<9i', u.mem_read(ptr + index * 36, 36)))
                for index in range(42)]

    def rng():
        return {name: bytes(u.mem_read(ptr, 1012)).hex()
                for name, ptr in mission.resident.rngs.items()}

    original = bytes(u.mem_read(0x401000, 0x3E0000))
    before_rng = rng()
    m.invoke(0x5236A0, typ, [m.cstring('CLEG')],
             context=dict(owner='infantry_scatter_destination', phase='CLEG type constructor'))
    constructor = dict(fields=fields(), records=read_records())
    layers = []
    for name, path in base.layers():
        if not path.exists():
            assert name == 'LANGRULE.INI'
            layers.append(dict(file=name, absent=True))
            continue
        raw = path.read_bytes()
        sections, source_lines = base.lexical(raw, {'CLEG'})
        m.rules_cache(sections)
        before = fields()
        event_start, read_start = len(mission.events), len(m.reads)
        eax = m.invoke(0x5240A0, typ, [RULES],
                       context=dict(owner='infantry_scatter_destination', phase='CLEG type read',
                                    layer=name, input_sha256=hashlib.sha256(raw).hexdigest()))
        layers.append(dict(file=name, sha256=hashlib.sha256(raw).hexdigest(), sections=sections,
                           source_lines=source_lines, before=before, after=fields(), eax=eax, al=eax & 255,
                           reads=m.reads[read_start:], setup_events=mission.events[event_start:]))
    actual_records = read_records()
    assert actual_records == records, 'Whole CLEG type reader disagrees with original sequence owner'
    assert original == bytes(u.mem_read(0x401000, 0x3E0000))
    return dict(constructor=constructor, layers=layers, fields=fields(), records=actual_records,
                art=dict(sha256=hashlib.sha256(art_raw).hexdigest(), sections=art, source_lines=lines),
                inherited_setup=dict(inputs=mission.inputs, world=mission.world),
                rng_before=before_rng, rng_after=rng(), code_unchanged=True,
                limits=['Existing Mission.prepare/attach_world bootstrap, without Mission.setup or GeneralRules.',
                        'Original constructor/full reader execute against physical lexical signed-CRC caches; file/archive loading is outside this boundary.',
                        'Existing Mission owns Windows ASCII/CLSID callbacks and setup-only type visual asset returns. No original gameplay callable result is supplied.'])


def generate_teleport(cases=None):
    from tools.spatial_oracle.jumpjet_infantry_actions import default_motion_records
    records, receipts, read = default_motion_records()
    type_inputs = teleport_type_inputs(records)
    image = image_bytes()
    spans = [(0x51AA40, 0x7A1), (0x4D94B0, 0x274), (0x718000, 0x80),
             (0x55A710, 0x60), (0x718080, 0x20), (0x718100, 0x123),
             (0x718230, 0x2D), (0x718B70, 1870), (0x481180, 795),
             (0x51DAF0, 0xDF), (0x51D6F0, 0x400),
             (0x5217C0, 0x8F), (0x521850, 0x89), (0x51BF90, 0xAE8),
             (0x50B730, 0x23), (0x5236A0, 0x2D7), (0x5240A0, 0x6A9)]
    identity = dict(native_sha256=NATIVE_SHA256,
                    spans=[dict(address=hex(address), bytes=length,
                                sha256=hashlib.sha256(file_span(image, address, length)[1]).hexdigest())
                           for address, length in spans],
                    limits='File-backed byte identity for explicit spans; sizes do not assert exhaustive function or caller coverage.')
    return dict(records=records, retail_inputs=receipts, sequence_reader=read,
                type_inputs=type_inputs, native_identity=identity,
                rows=[query(case, teleport_records=records, teleport_type_inputs=type_inputs)
                      for case in (teleport_cases() if cases is None else cases)])


def teleport_metadata():
    return provenance(
        scope='Original active ordinary CLEG Cell SetDestination51AA40 through Teleport resolver718B70, actual Cell481180/Infantry51BF90/raw receivers; repeated requests and Stop before Process.',
        entry_points=dict(infantry_destination=0x51AA40, foot_destination=0x4D94B0,
                          teleport_constructor=0x718000, link=0x55A710,
                          teleport_is_moving=0x718080, teleport_move_to=0x718100,
                          teleport_stop=0x718230, teleport_resolver=0x718B70,
                          cell_placement=0x481180, infantry_entry=0x51BF90,
                          raw_put=0x5217C0, raw_remove=0x521850,
                          stop_driver=0x51DAF0, do_action=0x51D6F0,
                          infantry_type_constructor=0x5236A0, infantry_type_reader=0x5240A0,
                          human_predicate=0x50B730, infantry_ready=0x521B60, commence=0x5B3570,
                          subcell_startup=0x48E480, teleport_height_startup=0x717EC0),
        assumptions=[
            'Single infantry_source_scatter command controller: original Infantry/Cell/ILocomotion vtables, Teleport constructor718000 and Link55A710, valid refcount1 and42 ClegSequence records. Independent original InfantryType5236A0 and whole5240A0 execute through the existing Mission reader/controller on physical RULESMD, absent LANGRULE, Battle and AnyTown CLEG sections, with fixed ARTMD. Actual JumpJet+D94 byte0 and MovementZone+5B4 Infantry7 initialize the already-live command fixture with the other retained scalar fields; its record bank must equal the existing original523D00 owner exactly. The corpus retains full inherited reader setup/asset inputs. Whole Infantry/House/scenario construction is outside the command boundary.',
            'Human House fields are explicit supplied inputs: index0, +1EC1, +1ED0 and image-default GameMode0. Original50B730 executes and saves its actual return and complete RNG before/after, including the per-command predicates. This establishes the predicate from these declared fields; it does not claim the full House-loading lifecycle.',
            'The command fixture explicitly supplies blocked-delay Rules+1768=22 through global8871E0, with frame100 and game speed1. This is a test input, not a physical retail GeneralRules result; Mission.setup/GeneralRules is excluded from the type prerequisite. Saved blocked-timer writes consume this supplied value.',
            '32x32 original Cells with widened supplied playfield, empty content/overlay lists, per-row declared raw occupation, flat level0 and nonzero land0/zero land1 speed-table fixture inputs. Original Cell/Infantry/Teleport CRT tables and subcell48E480 producer execute; cold geometry values are not supplied. Current vehicle20 inputs retain original Infantry initial-mark writes, and full-slot/vehicle destination refusals execute recursive classNULL without FNPC.',
            'Class51AA40 calls reach their real RET8. Original CanEnter51BF90, StopDriver51DAF0, DoAction51D6F0, MoveTo718100, resolver718B70, Cell481180 and5217C0/521850 execute. Full three1012-byte seeded RNG objects, callback/preflight/return order, raw planes, armed+1C and retained+28 XYZ are saved.',
            'Commands are direct original class calls at frame100, before any Process. Resolver Ready521B60/Commence5B3570 execute in original order, including queuedAttack continuation. Pure Cell NavCom selects priority0; instruction-defined priority gates for Eaten8/Capture9/Enter7/Patrol25 object destinations remain outside executable coverage. Complete EventClass/command batch, FNPC/object destinations, Chronosphere and the whole Process/request lifetime are outside this corpus. Any FNPC56DC20 or TeleportProcess7192F0 visit fails these controls.',
        ],
        substitutions=['Command controls substitute only OS InterlockedIncrement/Decrement imports with their pointed-count stdcall operations. Type prerequisite reads inherit existing Mission/Reader bounded allocation/cache/TLS, original GUID Windows OS callbacks and setup-only visual-asset boundaries. No original selected gameplay callable return is supplied.'])


def teleport_source_paths():
    # Final publication binds this existing controller and the production
    # owners; callers stage unbound native data while those Rust files change.
    from tools.spatial_oracle.anytown_damage import foot_missions
    root = Path(__file__).resolve().parents[2]
    # One existing inventory owns the Mission reader/controller dependency
    # closure. Keep its historical receipt, adding current mechanism owners.
    paths = {name: path for name, path in foot_missions.source_paths().items()
             if path.suffix == '.py'}
    names = [
        'tools/native_oracle.py', 'tools/native_inspect.py',
        'tools/spatial_oracle/infantry_scatter_destination.py',
        'tools/spatial_oracle/infantry_source_scatter.py',
        'tools/spatial_oracle/jumpjet_infantry_actions.py',
        'tools/spatial_oracle/infantry_sequence_rules.py',
        'tools/spatial_oracle/infantry_entry_raw.py',
        'tools/projectile_oracle/bridge_render_inputs.py',
        'src/sim/movement/teleport_movement.rs', 'src/sim/movement/infantry_scatter.rs',
        'src/sim/movement/motion_query.rs', 'src/sim/movement/walk_head.rs',
        'src/sim/movement/bump_crush.rs', 'src/sim/cell_kernel.rs',
        'src/sim/movement/locomotor.rs', 'src/sim/movement/locomotor_owner.rs',
        'src/sim/movement/mod.rs', 'src/sim/movement/teleport_cell_destination_tests.rs',
        'src/sim/game_entity.rs', 'src/sim/mission/authority.rs',
        'src/sim/mission/readiness.rs', 'src/sim/mission/verb.rs',
        'src/rules/object_type.rs', 'src/rules/native_processing.rs',
        'src/rules/ruleset.rs', 'src/rules/ini_value.rs',
    ]
    paths.update({name: root / name for name in names})
    return paths


if __name__ == '__main__' and '--teleport-cell' in sys.argv:
    args = [arg for arg in sys.argv[1:] if arg != '--teleport-cell']
    finish_vectors(generate_teleport, Path(__file__).with_name('infantry_teleport_destination.json'),
                   provenance=teleport_metadata, source_paths=teleport_source_paths(), argv=args)
elif __name__ == '__main__':
    finish_vectors(generate, Path(__file__).with_suffix('.json'), provenance=lambda: provenance(
        scope='Actual source-aware Infantry Scatter, CanEnter, QueueMission, Infantry/Foot destination setter, Walk MoveTo/Stop; supplied ordinary non-JumpJet Fraidycat. Optional separate next-Process probes stop at first FindPath or paid-step boundary.',
        entry_points={'scatter': 0x51D0D0, 'entry': 0x51BF90, 'infantry_set_destination': 0x51AA40,
                      'foot_set_destination': 0x4D94B0, 'walk_constructor': 0x75AA90,
                      'walk_move_to': 0x75ACB0, 'failed_path_receiver': 0x51DAF0,
                      'clear_cell': 0x4834A0, 'walk_stop': 0x75ADA0, 'walk_process': 0x75AEC0},
        assumptions=[
            'Inherits infantry_source_scatter live-entry fixture: widened synthetic playfield, 32x32 allocated original Cell vtables, empty lists/overlays, supplied raw occupation and nonzero land0/zero land1 speed rows. Original heading startup and seeded ScenarioRandom execute.',
            'Original Walk constructor and actual Infantry/Foot/Cell vtables. Linked owner and reference count1 supplied. No JumpJet, reciprocal Rocker/lift links, retained fire particles, EMP or +6AC suppression latch. Nonhuman Fraidycat avoids the unrelated human prone DoAction7 branch.',
            'Frame100 and blocked delay22; initial movement timer50/5, blocked timer40/6, retry count7, path backing2/3/4/5, reference9/8, null NavCom and aux123. Foot gate/warp bytes, optional paid head, power and moving prestates are supplied. Moving rows retain an old destination7808,2688,0.',
            'Failed-path receiver executes actual DoAction with supplied zero sequence counts (no animation acceptance), actual CanEnter and Stop. Initial +6DC is1 so receiver writes are observable. No invented result substituted for any gameplay callable.',
            'Source-aware Scatter never calls Process. Two explicit later probes stop at first FindPath or paid numeric approach; pathfinder core and complete movement are outside this native corpus.',
            'Original 6D1830/6D18C0/6D1BF0 initialize translation scale with FPCW0E7F, including actual structural-bridge destination rise414.',
        ],
        substitutions=['Only OS InterlockedIncrement/Decrement imports implement their pointed-count stdcall operations. No gameplay callable substituted.']))
