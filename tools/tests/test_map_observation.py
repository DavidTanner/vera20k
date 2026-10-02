"""Portable child receipts exercise the wrapper, without retail files or a GPU."""
from copy import deepcopy
import contextlib
import io
import json
from pathlib import Path
import tempfile
import shutil
import unittest
from unittest.mock import patch

from tools import map_observation as observation
from tools.child_process import ChildResult
from tools.tactical_certification.core import OutputExistsError, ValidationError, sha256_bytes
from tools.tactical_certification.profile import repository_contract_path


class MapObservationTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.profile = json.loads((observation.ROOT / 'tools/map_observation.example.json').read_text())
        self.profile.update(width=2, height=2, ticks=3, timeout_seconds=1)
        # Rust owns admission and real extent limits. These small synthetic frames
        # intentionally exercise only wrapper receipt/byte validation.
        self.profile_path = self.root / 'profile.json'
        self.profile_path.write_text(json.dumps(self.profile))
        self.config = self.root / 'config.toml'
        self.config.write_text('[paths]\nra2_dir="fixture"\n')
        self.executable = self.root / 'game'
        self.executable.write_bytes(b'fake executable identity')
        self.contract = repository_contract_path()
        self.output = self.root / 'observation'
        self.unit_atlas = {
            'resident_sprite_count': 7, 'last_build_rasterized_sprite_count': 23,
            'pages': [
                {'extent': [2, 3, 1], 'format': 'R8Uint', 'dimension': 'D2',
                 'mip_level_count': 1, 'sample_count': 1, 'texel_payload_bytes': 6},
                {'extent': [5, 2, 1], 'format': 'R8Uint', 'dimension': 'D2',
                 'mip_level_count': 1, 'sample_count': 1, 'texel_payload_bytes': 10},
            ],
            'total_texel_payload_bytes': 16,
        }
        self.frame = bytes(range(16))
        self.actor_frames = {}
        self.rule_types = [{'type_id': name, 'interned_id': identity, 'category': 'Structure'}
                           for name, identity in [('GACNST', 40), ('GAPOWR', 41), ('GAPILE', 42)]]
        self.terrain_frames = {}
        self.change = lambda manifest: None
        self.result = ChildResult(42, 0, False, b'child output\n', b'', ())
        environment = patch.dict('os.environ', {}, clear=True)
        environment.start()
        self.addCleanup(environment.stop)

    def fake_child(self, command, **kwargs):
        self.assertEqual(command[:3], [str(self.executable), '--tactical-capture', 'map-observe-v1'])
        self.assertEqual(kwargs['cwd'], self.root)
        self.assertEqual(kwargs['timeout_seconds'], 1)
        directory = Path(command[-1])
        directory.mkdir()
        frame = self.frame
        (directory / 'frame.bgra').write_bytes(frame)
        ticks = self.profile['ticks']
        fingerprint = lambda tick: {'simulation_tick': tick, 'binary_frame': tick,
                                   'total_simulation_ms': tick * 22,
                                   'deterministic_state_hash': 7 + tick}
        receipt = lambda tick: {'tick_before': tick, 'tick_after': tick + 1,
                               'binary_frame_before': tick, 'binary_frame_after': tick + 1}
        identity = lambda path: {'path': str(path), 'byte_length': path.stat().st_size,
                                 'sha256': sha256_bytes(path.read_bytes())}
        manifest = {
            'schema_version': observation.CHILD_SCHEMA, 'status': 'COMPLETE',
            'profile': {'sha256': sha256_bytes(self.profile_path.read_bytes()),
                        'request': deepcopy(self.profile)},
            'contract': {'sha256': sha256_bytes(self.contract.read_bytes())},
            'inputs': {'config': identity(self.config), 'executable': identity(self.executable)},
            'initial': fingerprint(0), 'final': fingerprint(ticks), 'exact_step_count': ticks,
            'first_exact_step': receipt(0) if ticks else None,
            'last_exact_step': receipt(ticks - 1) if ticks else None,
            'startup': {'seed': self.profile['seed'], 'seed_source': 'Controlled',
                        'seed_authority_certifying': True, 'correlation': 1,
                        'classification': 'AcceptedExplicitFixedBattle'},
            'map_source': {'kind': 'mix', 'logical_name': 'Fight.MAP', 'source_archive': 'maps.mix',
                           'entry_id': -10, 'payload_len': 90, 'source_sha256': 'a' * 64},
            'lifecycle': {'window_hidden': True, 'window_focused': False,
                          'focus_violations': 0, 'input_violations': 0},
            'render': {'ready': True, 'sidebar_view_present': True,
                       'surface_extent': [2, 2], 'internal_extent': [2, 2],
                       'unit_atlas': deepcopy(self.unit_atlas),
                       'presentation_clock': self.clock(ticks),
                       'camera': {'requested_cell': self.profile.get('camera_cell'),
                                  'top_left': [0.0, 0.0], 'zoom': 1.0},
                       'neutral_input': {'static_default_cursor': True, 'camera_input_idle': True}},
            'frame': {'file_name': 'frame.bgra', 'width': 2, 'height': 2, 'row_stride': 8,
                      'byte_length': 16, 'sha256': sha256_bytes(frame),
                      'pixel_layout': 'BGRA8', 'surface_format': 'Bgra8UnormSrgb'},
            'native_comparator': 'NONE', 'parity_certification': 'NONE',
        }
        seen = set()
        frames = []
        for step in range(ticks + 1):
            actors = deepcopy(self.actor_frames.get(step, []))
            current_ids = {actor['stable_id'] for actor in actors}
            seen.update(current_ids)
            terrain = self.terrain_frames.get(step, [self.unallocated_cell(cell)
                                                     for cell in self.profile.get('terrain_cells', [])])
            frames.append({'completed_steps': step, 'simulation_tick': step, 'binary_frame': step,
                           'total_simulation_ms': step * 22, 'actors': actors,
                           'missing_actor_ids': sorted(seen - current_ids), 'terrain': deepcopy(terrain)})
        manifest['observations'] = {
            'policy': observation.OBSERVATION_POLICY, 'owners': self.profile.get('observe_owners', []),
            'rule_types': deepcopy(self.rule_types),
            'commands': [{'ordinal': index, 'issue_after_step': request['issue_after_step'],
                          'issued_simulation_tick': request['issue_after_step'],
                          'envelope_execute_tick': request['issue_after_step'],
                          'owner': request['owner'], 'payload': deepcopy(request['payload'])}
                         for index, request in enumerate(self.profile.get('commands', []))],
            'frames': frames,
        }
        self.change(manifest)
        (directory / 'capture.json').write_text(json.dumps(manifest))
        return self.result

    def run_capture(self):
        with patch.object(observation, 'run_child', side_effect=self.fake_child):
            return observation.capture(profile_path=self.profile_path, contract_path=self.contract,
                                       output=self.output, working_directory=self.root,
                                       executable=self.executable)

    @staticmethod
    def actor(identity=1, owner='Computer1', category='Infantry'):
        return {
            'stable_id': identity, 'owner': owner, 'type_id': 'E1' if category == 'Infantry' else 'MTNK',
            'category': category, 'cell': [87, 53], 'physical_leptons': [22400, 13696, 416],
            'on_bridge': True, 'health': 125, 'active': True, 'in_limbo': False, 'dying': False,
            'mission': {'current': 5, 'queued': -1, 'suspended': -1, 'effective': 5,
                        'handler_state': 0, 'start_frame': 0, 'ai_counter': 0,
                        'dispatch_timer': {'start_frame': 0, 'delay': 15}},
            'target': {'Entity': 8}, 'archive': {'Cell': [87, 53]},
            'nav': {'Object': {'id': 9}},
            'foot': None if category == 'Structure' else {
                     'retarget_after_stop_688': False, 'firing_sequence_latch_68d': 0,
                     'infantry_doing': 0 if category == 'Infantry' else None,
                     'navigation_leptons': [22400, 13696, 416], 'navigation_unavailable': None},
            'building': MapObservationTests.building(identity) if category == 'Structure' else None,
        }

    @staticmethod
    def building(owner=1):
        return {'body_state': 1, 'queued_body_state': -1, 'construction_control': [0, 25, 2],
                'stage': {'value': 0, 'changed': 0, 'rate': 0, 'increment': 1,
                          'timer': {'start_frame': 50, 'duration': 0}},
                'ready_latch': 1, 'actually_placed': True, 'last_operational': False,
                'animation_slots': [{'slot': 3, 'anim_id': 100,
                    'animation': {'stable_id': 100, 'native_id': 10, 'type_id': 'GAPOWR_A',
                        'interned_type_id': 80, 'physical_leptons': [22400, 13696, 416],
                        'in_logic_vector': True, 'owner_entity': None, 'building_slot': [owner, 3],
                        'runtime': {'current_frame': 0, 'frame_step': 1, 'delay_remaining': 0,
                            'rate_reload': 4, 'frame_timer': {'start_frame': 50, 'duration': 4},
                            'loop_remaining': 255, 'first_ai_guard': False,
                            'constructor_reverse': False, 'inactive': False, 'paused': False}}}]}

    @staticmethod
    def unit():
        return {'deployed_6e0': 0, 'deploying_6e1': 1, 'undeploying_6e2': 0,
                'landing_for_deploy_134': False, 'stage_f8': 2, 'body_counter_538': 7,
                'deploy_anim_130': {'stable_id': 100, 'live': {
                    'type_id': 'SCHPDEPL', 'frame': 4, 'owner_entity': 1}}}

    @staticmethod
    def unallocated_cell(coordinate):
        return {'cell': list(coordinate), 'allocated': False,
                **{key: None for key in ('final_tile_index', 'final_sub_tile', 'presentation_tile',
                                        'level', 'slope', 'raw_bridge_flags', 'bridge_state',
                                        'has_deck', 'deck_level', 'walkable', 'transition')}}

    def scripted_profile(self):
        self.profile.update(
            schema_version=observation.PROFILE_V2, observe_owners=['Computer1'],
            camera_cell=[87, 53], terrain_cells=[[87, 53]],
            commands=[{'issue_after_step': 0, 'owner': 'Computer1', 'payload': {'Stop': {'entity_id': 1}}},
                      {'issue_after_step': 0, 'owner': 'Computer1',
                       'payload': {'Guard': {'entity_id': 1, 'target_id': None}}},
                      {'issue_after_step': 2, 'owner': 'Computer1',
                       'payload': {'ForceAttackCell': {'attacker_id': 1, 'target_rx': 87, 'target_ry': 53}}}])
        self.profile_path.write_text(json.dumps(self.profile))
        self.actor_frames = {step: [self.actor()] for step in range(4)}

    def test_v2_records_exact_issue_order_without_input_delay_and_every_actor_frame(self):
        self.scripted_profile()
        report = self.run_capture()
        self.assertEqual(report['status'], 'VALID', report['errors'])
        transcript = report['capture']['observations']
        self.assertEqual([row['ordinal'] for row in transcript['commands']], [0, 1, 2])
        self.assertEqual([row['envelope_execute_tick'] for row in transcript['commands']], [0, 0, 2])
        self.assertEqual([row['completed_steps'] for row in transcript['frames']], [0, 1, 2, 3])
        self.assertEqual(transcript['frames'][0]['actors'][0]['nav'], {'Object': {'id': 9}})
        self.assertEqual(report['capture']['camera']['requested_cell'], [87, 53])
        self.assertEqual(observation.validate_run(self.output)['status'], 'VALID')

    def production_profile(self):
        self.scripted_profile()
        self.profile['commands'] = [
            {'issue_after_step': 0, 'owner': 'Computer1', 'payload': {'QueueProduction': {'type_id': 41}}},
            {'issue_after_step': 2, 'owner': 'Computer1',
             'payload': {'PlaceReadyBuilding': {'type_id': 41, 'rx': 87, 'ry': 53}}}]
        self.profile_path.write_text(json.dumps(self.profile))
        self.actor_frames = {step: [self.actor(category='Structure')] for step in range(4)}
        for actors in self.actor_frames.values():
            actors[0]['type_id'] = 'GAPOWR'

    def test_production_commands_use_recorded_handles_and_keep_building_animation_state(self):
        self.production_profile()
        self.actor_frames[3][0]['building']['last_operational'] = True
        self.actor_frames[3][0]['building']['animation_slots'][0]['anim_id'] = 101
        self.actor_frames[3][0]['building']['animation_slots'][0]['animation']['stable_id'] = 101
        report = self.run_capture()
        self.assertEqual(report['status'], 'VALID', report['errors'])
        self.assertEqual(report['schema_version'], 'vera20k.map-observation-run.v5')
        observed = report['capture']['observations']
        self.assertEqual(observed['rule_types'], self.rule_types)
        self.assertEqual([row['payload'] for row in observed['commands']],
                         [row['payload'] for row in self.profile['commands']])
        self.assertEqual(observed['frames'][3]['actors'][0]['building'],
                         self.actor_frames[3][0]['building'])
        self.assertEqual(observation.validate_run(self.output)['status'], 'VALID')

    def test_engineer_and_paid_repair_orders_keep_the_existing_command_payload(self):
        self.production_profile()
        self.profile['commands'] = [
            {'issue_after_step': 0, 'owner': 'Computer1',
             'payload': {'ToggleRepair': {'entity_id': 1}}},
            {'issue_after_step': 2, 'owner': 'Computer1',
             'payload': {'CaptureBuilding': {'engineer_id': 7, 'target_building_id': 1}}}]
        self.profile_path.write_text(json.dumps(self.profile))
        report = self.run_capture()
        self.assertEqual(report['status'], 'VALID', report['errors'])
        self.assertEqual([row['payload'] for row in report['capture']['observations']['commands']],
                         [row['payload'] for row in self.profile['commands']])
        self.assertEqual(observation.validate_run(self.output)['status'], 'VALID')

    def test_building_and_rule_handle_receipts_reject_wrong_types_or_unknown_fields(self):
        self.production_profile()
        changes = [lambda m: m['observations'].pop('rule_types'),
                   lambda m: m['observations']['rule_types'][0].update(interned_id=True),
                   lambda m: m['observations']['rule_types'][0].update(category='Unknown'),
                   lambda m: m['observations']['rule_types'][0].update(type_id=''),
                   lambda m: m['observations']['rule_types'][0].update(extra=0)]
        building_changes = [lambda b: b.update(body_state=True),
                            lambda b: b.update(ready_latch=256),
                            lambda b: b.update(actually_placed=1),
                            lambda b: b['stage']['timer'].update(start_frame=1.5),
                            lambda b: b['stage'].update(changed=False),
                            lambda b: b.update(construction_control=[0, 25]),
                            lambda b: b['animation_slots'][0].update(slot=21),
                            lambda b: b['animation_slots'].append(deepcopy(b['animation_slots'][0])),
                            lambda b: b['animation_slots'][0]['animation'].update(stable_id=101),
                            lambda b: b['animation_slots'][0]['animation']['runtime'].update(first_ai_guard=0),
                            lambda b: b['animation_slots'][0]['animation']['runtime'].update(rate_reload=65536),
                            lambda b: b['animation_slots'][0]['animation']['runtime'].update(loop_remaining=-1)]
        changes += [lambda m, change=change: change(m['observations']['frames'][1]['actors'][0]['building'])
                    for change in building_changes]
        for index, change in enumerate(changes):
            with self.subTest(index=index):
                self.output = self.root / f'building-state-invalid-{index}'
                self.change = change
                self.assertEqual(self.run_capture()['status'], 'INVALID')

    def test_missing_live_animation_is_explicit_and_slots_count_towards_sample_budget(self):
        self.production_profile()
        self.actor_frames[1][0]['building']['animation_slots'][0]['animation'] = None
        report = self.run_capture()
        self.assertEqual(report['status'], 'VALID', report['errors'])
        self.assertIsNone(report['capture']['observations']['frames'][1]['actors'][0]
                          ['building']['animation_slots'][0]['animation'])
        self.output = self.root / 'bounded-building-slots'
        # Four actor + four terrain + four occupied-slot observations.
        with patch.object(observation, 'MAX_OBSERVATION_SAMPLES', 11):
            self.assertEqual(self.run_capture()['status'], 'INVALID')

    def test_comparison_includes_rule_handles_and_animation_lifetime_and_runtime(self):
        self.production_profile()
        before = self.valid_capture('building-before')
        self.change = lambda m: m['observations']['frames'][1]['actors'][0]['building'] \
            ['animation_slots'][0]['animation']['runtime'].update(current_frame=1)
        after = self.valid_capture('building-after')
        report = observation.compare_runs(before, after)
        self.assertEqual(report['status'], 'MISMATCH', report['errors'])
        self.assertEqual([row['field'] for row in report['differences']],
                         ['observations.frames[1].actors[0].building.animation_slots[0].animation.runtime.current_frame'])
        self.change = lambda m: m['observations']['rule_types'][0].update(interned_id=70)
        after = self.valid_capture('rule-handle-after')
        report = observation.compare_runs(before, after)
        self.assertEqual(report['status'], 'MISMATCH', report['errors'])
        self.assertEqual([row['field'] for row in report['differences']],
                         ['observations.rule_types[0].interned_id'])

    def test_v5_building_and_terrain_resource_receipts_validate_and_compare_together(self):
        self.production_profile()
        cell = self.unallocated_cell([87, 53])
        cell.update(overlay={'id': None, 'density': 3},
                    terrain_object={'name': 'TIBTRE02', 'frame': 10, 'active': True})
        self.terrain_frames[1] = [cell]
        before = self.valid_capture('building-terrain-before')
        checked = observation.validate_run(before)
        self.assertEqual(checked['status'], 'VALID', checked['errors'])
        frame = checked['capture']['observations']['frames'][1]
        self.assertEqual(frame['actors'][0]['building'], self.actor_frames[1][0]['building'])
        self.assertEqual(frame['terrain'], [cell])

        def change(manifest):
            frame = manifest['observations']['frames'][1]
            frame['actors'][0]['building']['animation_slots'][0]['animation'] \
                ['runtime']['frame_timer']['duration'] = 5
            frame['terrain'][0]['overlay']['density'] = 4

        self.change = change
        after = self.valid_capture('building-terrain-after')
        report = observation.compare_runs(before, after)
        self.assertEqual(report['status'], 'MISMATCH', report['errors'])
        self.assertEqual([row['field'] for row in report['differences']], [
            'observations.frames[1].actors[0].building.animation_slots[0].animation.runtime.frame_timer.duration',
            'observations.frames[1].terrain[0].overlay.density'])

    def test_profile_version_order_field_types_and_budgets_are_checked_before_spawn(self):
        profile = deepcopy(self.profile)
        cases = [dict(profile, commands=[]), dict(profile, observe_owners=[]),
                 dict(profile, camera_cell=[87, 53]), dict(profile, terrain_cells=[]),
                 dict(profile, schema_version='unknown'), dict(profile, extra=True)]
        modern = dict(profile, schema_version=observation.PROFILE_V2)
        cases.extend(dict(modern, **extension) for extension in (
            {'commands': None}, {'observe_owners': None}, {'camera_cell': None}, {'terrain_cells': None},
            {'observe_owners': ['Computer1', 'Computer1']}, {'observe_owners': ['']},
            {'observe_owners': [1]}, {'observe_owners': [str(index) for index in range(31)]},
            {'terrain_cells': [[87, 53], [87, 53]]}, {'camera_cell': [True, 53]},
            {'camera_cell': [-1, 53]}, {'camera_cell': [65536, 53]},
            {'terrain_cells': [[index, 0] for index in range(257)]}))
        valid_command = {'issue_after_step': 0, 'owner': 'Computer1', 'payload': {'Stop': {'entity_id': 1}}}
        for key, value in (('issue_after_step', True), ('issue_after_step', 3),
                           ('issue_after_step', -1), ('issue_after_step', 0.0), ('owner', ''),
                           ('owner', 1), ('extra', True), ('payload', {}),
                           ('payload', {'Select': {'entity_ids': [1], 'additive': False}})):
            cases.append(dict(modern, commands=[dict(valid_command, **{key: value})]))
        cases.append(dict(modern, commands=[dict(valid_command, issue_after_step=2), valid_command]))
        cases.append(dict(modern, commands=[valid_command] * 1025))
        for index, candidate in enumerate(cases):
            with self.subTest(case=index), patch.object(observation, 'run_child') as child:
                self.profile_path.write_text(json.dumps(candidate))
                with self.assertRaises(ValidationError):
                    self.run_capture()
                child.assert_not_called()
                self.assertFalse(self.output.exists())

    def test_command_receipts_cannot_reorder_retime_omit_or_change_payload(self):
        self.scripted_profile()
        changes = [lambda rows: rows.pop(), lambda rows: rows.reverse(),
                   lambda rows: rows[0].update(extra=True),
                   lambda rows: rows[0]['payload']['Stop'].update(entity_id=2),
                   lambda rows: rows[2].update(issue_after_step=1),
                   lambda rows: rows[2].update(issued_simulation_tick=3),
                   lambda rows: rows[2].update(envelope_execute_tick=4),
                   lambda rows: rows[0].update(owner='Computer2')]
        for key in ('ordinal', 'issue_after_step', 'issued_simulation_tick', 'envelope_execute_tick'):
            changes.append(lambda rows, key=key: rows[0].update({key: True}))
        for index, change in enumerate(changes):
            with self.subTest(case=index):
                self.output = self.root / f'command-receipt-{index}'
                self.change = lambda manifest, change=change: change(manifest['observations']['commands'])
                report = self.run_capture()
                self.assertEqual(report['status'], 'INVALID', report)
                self.assertTrue(any('observations.commands' in error for error in report['errors']))

    def test_terrain_resource_extension_accepts_native_frame_and_rejects_invalid_values(self):
        cell = self.unallocated_cell([74, 32])
        cell['overlay'] = {'id': None, 'density': 3}
        cell['terrain_object'] = {'name': 'TIBTRE02', 'frame': 10, 'active': True}
        observation._terrain(cell, [74, 32], 'terrain')
        for key, value in (('frame', True), ('frame', 1 << 31), ('active', 1), ('name', '')):
            bad = deepcopy(cell)
            bad['terrain_object'][key] = value
            with self.assertRaises(observation.ValidationError):
                observation._terrain(bad, [74, 32], 'terrain')
        for key, value in (('id', 256), ('density', -1), ('density', True)):
            bad = deepcopy(cell)
            bad['overlay'][key] = value
            with self.assertRaises(observation.ValidationError):
                observation._terrain(bad, [74, 32], 'terrain')
        for key in ('overlay', 'terrain_object'):
            bad = deepcopy(cell)
            bad.pop(key)
            with self.assertRaises(observation.ValidationError):
                observation._terrain(bad, [74, 32], 'terrain')
        cell['overlay'] = None
        cell['terrain_object'] = {'name': 'TREE01', 'frame': None, 'active': None}
        observation._terrain(cell, [74, 32], 'terrain')
        cell['terrain_object'] = None
        observation._terrain(cell, [74, 32], 'terrain')

    def test_unit_extension_retains_animation_lifetime_and_compares_observations(self):
        self.scripted_profile()
        self.actor_frames = {step: [dict(self.actor(category='Unit'), unit=self.unit())]
                             for step in range(4)}
        self.actor_frames[0][0]['unit']['deploy_anim_130'] = None
        self.actor_frames[2][0]['unit']['deploy_anim_130']['live'] = None
        before = self.valid_capture('unit-before')
        checked = observation.validate_run(before)
        self.assertEqual(checked['status'], 'VALID', checked['errors'])
        for index, frame in enumerate(checked['capture']['observations']['frames']):
            self.assertEqual(frame['actors'][0]['unit'], self.actor_frames[index][0]['unit'])
        self.assertEqual(checked['capture']['observations']['policy'], observation.OBSERVATION_POLICY)
        self.change = lambda m: m['observations']['frames'][1]['actors'][0]['unit'] \
            ['deploy_anim_130']['live'].update(frame=5)
        after = self.valid_capture('unit-after')
        report = observation.compare_runs(before, after)
        self.assertEqual(report['status'], 'MISMATCH', report['errors'])
        self.assertEqual([row['field'] for row in report['differences']],
                         ['observations.frames[1].actors[0].unit.deploy_anim_130.live.frame'])

    def test_unit_extension_validates_storage_types_without_asserting_gameplay(self):
        actor = dict(self.actor(category='Unit'), unit=self.unit())
        # These are typed observations, including stale pointers and raw flag bytes.
        actor['unit'].update(deployed_6e0=255, deploying_6e1=255, undeploying_6e2=255,
                             landing_for_deploy_134=True, stage_f8=-(1 << 31),
                             body_counter_538=(1 << 32) - 1)
        anim = actor['unit']['deploy_anim_130']
        anim['stable_id'] = (1 << 64) - 1
        anim['live'].update(frame=-(1 << 31), owner_entity=(1 << 64) - 1)
        observation._actor(actor, 'actor')
        actor['unit']['stage_f8'] = (1 << 31) - 1
        anim['live'].update(frame=(1 << 31) - 1, owner_entity=None)
        observation._actor(actor, 'actor')
        anim['live'] = None
        observation._actor(actor, 'actor')
        actor['unit']['deploy_anim_130'] = None
        observation._actor(actor, 'actor')
        for category in ('Infantry', 'Aircraft', 'Structure'):
            with self.subTest(category=category):
                actor = dict(self.actor(category=category), unit=None)
                observation._actor(actor, 'actor')
                actor['unit'] = self.unit()
                with self.assertRaises(ValidationError):
                    observation._actor(actor, 'actor')

    def test_unit_extension_rejects_malformed_fields_and_animation_identity(self):
        actor = dict(self.actor(category='Unit'), unit=self.unit())
        integer_fields = [
            (('unit', key), 0, 255)
            for key in ('deployed_6e0', 'deploying_6e1', 'undeploying_6e2')]
        integer_fields += [
            (('unit', 'stage_f8'), -(1 << 31), (1 << 31) - 1),
            (('unit', 'body_counter_538'), 0, (1 << 32) - 1),
            (('unit', 'deploy_anim_130', 'stable_id'), 1, (1 << 64) - 1),
            (('unit', 'deploy_anim_130', 'live', 'frame'), -(1 << 31), (1 << 31) - 1),
            (('unit', 'deploy_anim_130', 'live', 'owner_entity'), 1, (1 << 64) - 1)]
        for path, minimum, maximum in integer_fields:
            for value in (True, False, 1.0, '1', minimum - 1, maximum + 1):
                with self.subTest(path=path, value=value):
                    bad = deepcopy(actor)
                    field = bad
                    for key in path[:-1]:
                        field = field[key]
                    field[path[-1]] = value
                    with self.assertRaises(ValidationError):
                        observation._actor(bad, 'actor')
        changes = [lambda a: a.update(unit=None),
                   lambda a: a['unit'].update(landing_for_deploy_134=0),
                   lambda a: a['unit'].update(landing_for_deploy_134=None),
                   lambda a: a['unit'].update(deploy_anim_130=[]),
                   lambda a: a['unit']['deploy_anim_130'].update(live=[]),
                   lambda a: a['unit']['deploy_anim_130']['live'].update(type_id=''),
                   lambda a: a['unit']['deploy_anim_130']['live'].update(type_id=1)]
        for path in (('unit',), ('unit', 'deploy_anim_130'),
                     ('unit', 'deploy_anim_130', 'live')):
            original = actor
            for key in path:
                original = original[key]
            for key in (*original, 'unknown'):
                def change(candidate, path=path, key=key):
                    field = candidate
                    for part in path:
                        field = field[part]
                    if key == 'unknown':
                        field[key] = None
                    else:
                        field.pop(key)
                changes.append(change)
        for index, change in enumerate(changes):
            with self.subTest(change=index):
                bad = deepcopy(actor)
                change(bad)
                with self.assertRaises(ValidationError):
                    observation._actor(bad, 'actor')

    def test_older_unit_receipts_remain_unchanged_without_optional_extension(self):
        self.scripted_profile()
        self.actor_frames = {step: [self.actor(category='Unit')] for step in range(4)}
        for historical in (False, True):
            with self.subTest(historical=historical):
                run = self.valid_capture(f'unit-without-extension-{historical}')
                if historical:
                    self.make_historical_trajectory(run)
                paths = (run / 'run.json', run / 'child-output/capture.json')
                originals = {path: path.read_bytes() for path in paths}
                checked = observation.validate_run(run)
                self.assertEqual(checked['status'], 'VALID', checked['errors'])
                for frame in checked['capture']['observations']['frames']:
                    self.assertNotIn('unit', frame['actors'][0])
                for path, original in originals.items():
                    self.assertEqual(path.read_bytes(), original)

    def test_actor_history_retains_capture_and_disappearance_without_rebinding(self):
        self.scripted_profile()
        self.actor_frames[1][0]['owner'] = 'Computer2'
        self.actor_frames[2] = []
        self.actor_frames[3] = [self.actor(identity=2)]
        report = self.run_capture()
        self.assertEqual(report['status'], 'VALID', report['errors'])
        rows = report['capture']['observations']['frames']
        self.assertEqual(rows[1]['actors'][0]['owner'], 'Computer2')
        self.assertEqual(rows[2]['missing_actor_ids'], [1])
        self.assertEqual(rows[3]['missing_actor_ids'], [1])
        self.assertEqual(rows[3]['actors'][0]['stable_id'], 2)

    def test_actor_and_frame_transcripts_fail_closed_on_missing_order_or_bad_state(self):
        self.scripted_profile()
        changes = [lambda rows: rows.pop(), lambda rows: rows.reverse(),
                   lambda rows: rows[1].update(simulation_tick=2),
                   lambda rows: rows[1].update(total_simulation_ms=0),
                   lambda rows: rows[1].update(actors=[], missing_actor_ids=[]),
                   lambda rows: rows[1].update(missing_actor_ids=[1]),
                   lambda rows: rows[0]['actors'][0].update(owner='Computer2'),
                   lambda rows: rows[1]['actors'].append(deepcopy(rows[1]['actors'][0])),
                   lambda rows: rows[1]['actors'][0]['mission'].update(queued=True),
                   lambda rows: rows[1]['actors'][0]['foot'].update(firing_sequence_latch_68d=256),
                   lambda rows: rows[1]['actors'][0]['foot'].update(retarget_after_stop_688=1),
                   lambda rows: rows[1]['actors'][0]['foot'].update(infantry_doing=None),
                   lambda rows: rows[1]['actors'][0].update(nav={'Unknown': {'id': 9}}),
                   lambda rows: rows[1]['actors'][0].update(archive={'Cell': [True, 53]}),
                   lambda rows: rows[1]['actors'][0]['foot'].update(navigation_leptons=None),
                   lambda rows: rows[1]['actors'][0].update(physical_leptons=[1, 2]),
                   lambda rows: rows[1]['actors'][0].update(extra='unknown')]
        for index, change in enumerate(changes):
            with self.subTest(case=index):
                self.output = self.root / f'actor-receipt-{index}'
                self.change = lambda manifest, change=change: change(manifest['observations']['frames'])
                self.assertEqual(self.run_capture()['status'], 'INVALID')

    def test_allocated_terrain_and_explicit_unavailable_navigation_are_retained(self):
        self.scripted_profile()
        allocated = {'cell': [87, 53], 'allocated': True, 'final_tile_index': 700,
                     'final_sub_tile': 2, 'presentation_tile': [700, 2], 'level': 4, 'slope': 0,
                     'raw_bridge_flags': 256, 'bridge_state': 0, 'has_deck': True, 'deck_level': 8,
                     'walkable': True, 'transition': False}
        self.terrain_frames[1] = [allocated]
        self.actor_frames[1][0]['foot'].update(navigation_leptons=None,
                                              navigation_unavailable='active locomotor unavailable')
        report = self.run_capture()
        self.assertEqual(report['status'], 'VALID', report['errors'])
        self.assertEqual(report['capture']['observations']['frames'][1]['terrain'], [allocated])
        changes = [lambda m: m['observations']['frames'][1]['terrain'][0].update(level=True),
                   lambda m: m['observations']['frames'][1]['terrain'][0].update(cell=[88, 53]),
                   lambda m: m['observations']['frames'][1]['terrain'][0].update(allocated=False),
                   lambda m: m['observations']['frames'][0]['terrain'][0].update(level=0),
                   lambda m: m['render']['camera'].update(requested_cell=[88, 53]),
                   lambda m: m['render']['camera'].update(zoom=0),
                   lambda m: m['render']['camera'].update(zoom=1 << 2048),
                   lambda m: m['render']['camera'].update(top_left=[True, 0]),
                   lambda m: m['render']['camera'].update(extra=True)]
        for index, change in enumerate(changes):
            self.output = self.root / f'terrain-camera-{index}'
            self.change = change
            self.assertEqual(self.run_capture()['status'], 'INVALID')

    def test_sample_budget_is_checked_and_comparison_includes_actor_trajectory(self):
        self.scripted_profile()
        with patch.object(observation, 'MAX_OBSERVATION_SAMPLES', 7):
            report = self.run_capture()
            self.assertEqual(report['status'], 'INVALID')
            self.assertTrue(any('sample budget' in error for error in report['errors']))
        before = self.valid_capture('trajectory-before')
        self.change = lambda m: m['observations']['frames'][1]['actors'][0]['mission'].update(handler_state=1)
        after = self.valid_capture('trajectory-after')
        report = observation.compare_runs(before, after)
        self.assertEqual(report['status'], 'MISMATCH', report['errors'])
        self.assertEqual([row['field'] for row in report['differences']],
                         ['observations.frames[1].actors[0].mission.handler_state'])
        self.assertNotIn('observations', report['before']['capture'])
        self.assertEqual(report['before']['observation_transcript']['frame_count'], 4)

    def test_complete_receipt_and_logs_are_bound_to_inputs(self):
        report = self.run_capture()
        self.assertEqual(report['status'], 'VALID', report['errors'])
        self.assertEqual(report['capture']['exact_step_count'], 3)
        self.assertEqual(report['capture']['unit_atlas'], self.unit_atlas)
        self.assertEqual(report['capture']['presentation_clock']['draws'], [
            {'completed_steps': 1, 'radar_ms': 22, 'tooltip_ms': 22, 'message_ms': 22},
            {'completed_steps': 2, 'radar_ms': 44, 'tooltip_ms': 44, 'message_ms': 44},
            {'completed_steps': 3, 'radar_ms': 66, 'tooltip_ms': 66, 'message_ms': 66},
        ])
        self.assertEqual((self.output / 'stdout.log').read_bytes(), b'child output\n')
        self.assertEqual((self.output / 'profile.json').read_bytes(), self.profile_path.read_bytes())
        self.assertEqual(json.loads((self.output / 'run.json').read_text()), report)
        self.assertEqual(report['parity_certification'], 'NONE')
        self.assertEqual(report['schema_version'], observation.RUN_SCHEMA)
        self.assertEqual((self.output / 'config.toml').read_bytes(), self.config.read_bytes())
        self.assertEqual((self.output / 'contract.json').read_bytes(), self.contract.read_bytes())

    def test_empty_atlas_receipt_is_valid(self):
        self.unit_atlas.update(resident_sprite_count=0, last_build_rasterized_sprite_count=0,
                               pages=[], total_texel_payload_bytes=0)
        report = self.run_capture()
        self.assertEqual(report['status'], 'VALID', report['errors'])
        self.assertEqual(report['capture']['unit_atlas'], self.unit_atlas)

    def test_atlas_statistics_remain_required(self):
        changes = [lambda m: m['render'].pop('unit_atlas'),
                   lambda m: m.update(schema_version='vera20k.map-observation.v1')]
        for value in (None, [], 1, 'statistics'):
            changes.append(lambda m, v=value: m['render'].update(unit_atlas=v))
        for key in self.unit_atlas:
            changes.append(lambda m, k=key: m['render']['unit_atlas'].pop(k))
        for index, change in enumerate(changes):
            with self.subTest(case=index):
                self.output = self.root / f'atlas-missing-{index}'
                self.change = change
                report = self.run_capture()
                self.assertEqual(report['status'], 'INVALID')
                self.assertIsNone(report['capture'])

    def test_atlas_counts_require_nonnegative_integers(self):
        for key in ('resident_sprite_count', 'last_build_rasterized_sprite_count',
                    'total_texel_payload_bytes'):
            for index, value in enumerate((-1, True, False, 1.0, '1', None)):
                with self.subTest(key=key, value=value):
                    self.output = self.root / f'atlas-count-{key}-{index}'
                    self.change = lambda m, k=key, v=value: m['render']['unit_atlas'].update({k: v})
                    self.assertEqual(self.run_capture()['status'], 'INVALID')

    def test_atlas_page_descriptor_and_payload_arithmetic_are_checked(self):
        cases = [('format', 'Rgba8Uint'), ('dimension', 'D3'),
                 ('mip_level_count', 2), ('mip_level_count', True),
                 ('sample_count', 4), ('sample_count', 1.0),
                 ('texel_payload_bytes', 7), ('texel_payload_bytes', 6.0),
                 ('texel_payload_bytes', True)]
        for extent in ([], [2, 3], [2, 3, 1, 1], '2,3,1', None,
                       [0, 3, 1], [2, -1, 1], [True, 3, 1], [2, False, 1],
                       [2.0, 3, 1], [2, '3', 1], [2, 3, 2], [2, 3, True], [2, 3, 1.0]):
            cases.append(('extent', extent))
        for index, (key, value) in enumerate(cases):
            with self.subTest(key=key, value=value):
                self.output = self.root / f'atlas-page-{index}'
                self.change = lambda m, k=key, v=value: m['render']['unit_atlas']['pages'][0].update({k: v})
                self.assertEqual(self.run_capture()['status'], 'INVALID')
        for index, key in enumerate(self.unit_atlas['pages'][0]):
            with self.subTest(missing=key):
                self.output = self.root / f'atlas-page-missing-{index}'
                self.change = lambda m, k=key: m['render']['unit_atlas']['pages'][0].pop(k)
                self.assertEqual(self.run_capture()['status'], 'INVALID')

    def test_atlas_pages_and_total_payload_are_checked(self):
        changes = [lambda m: m['render']['unit_atlas'].update(total_texel_payload_bytes=15)]
        for value in (None, {}, 'pages', [None], [[]], [1]):
            changes.append(lambda m, v=value: m['render']['unit_atlas'].update(pages=v))
        for index, change in enumerate(changes):
            with self.subTest(case=index):
                self.output = self.root / f'atlas-total-{index}'
                self.change = change
                self.assertEqual(self.run_capture()['status'], 'INVALID')

    def test_zero_steps_requires_unchanged_initial_state(self):
        self.profile['ticks'] = 0
        self.profile_path.write_text(json.dumps(self.profile))
        report = self.run_capture()
        self.assertEqual(report['status'], 'VALID')
        self.assertEqual(report['capture']['presentation_clock']['draws'], [
            {'completed_steps': 0, 'radar_ms': 0, 'tooltip_ms': 0, 'message_ms': 0}])

    def test_clock_bounds_include_one_and_maximum_step_budgets(self):
        for ticks in (1, 100_000):
            with self.subTest(ticks=ticks):
                clock = self.clock(ticks)
                self.assertEqual(observation._presentation_clock(clock, ticks), clock)
        self.assertEqual(self.clock(100_000)['draws'][-1]['radar_ms'], 2_200_000)
        for ticks in (-1, 100_001):
            with self.subTest(ticks=ticks), self.assertRaises(ValidationError):
                observation._presentation_clock(self.clock(1), ticks)

    def test_clock_schema_and_every_consumed_time_are_strict(self):
        changes = [lambda c: c.update(extra='unrecognized'),
                   lambda c: c.update(policy='wall-clock'),
                   lambda c: c.update(draws=c['draws'][1:]),
                   lambda c: c.update(draws=c['draws'] + [c['draws'][-1]]),
                   lambda c: c.update(draws=list(reversed(c['draws']))),
                   lambda c: c['draws'].__setitem__(1, deepcopy(c['draws'][0])),
                   lambda c: c['draws'][0].update(completed_steps=0),
                   lambda c: c['draws'][1].update(extra=True)]
        for key in ('policy', 'origin_ms', 'interval_ms', 'draws'):
            changes.append(lambda c, k=key: c.pop(k))
        for key in ('completed_steps', 'radar_ms', 'tooltip_ms', 'message_ms'):
            changes.append(lambda c, k=key: c['draws'][1].pop(k))
            for value in (None, True, False, 44.0, '44', -1, 0, 66, 1 << 64):
                changes.append(lambda c, k=key, v=value: c['draws'][1].update({k: v}))
        for key, values in (('policy', (None, 1, True)),
                            ('origin_ms', (None, True, False, 0.0, -1, 22)),
                            ('interval_ms', (None, True, 22.0, 0, 16, -1)),
                            ('draws', (None, {}, 'draws', [], [None, None, None],
                                       [{}, {}, {}], [1, 2, 3]))):
            for value in values:
                changes.append(lambda c, k=key, v=value: c.update({k: v}))
        for index, change in enumerate(changes):
            with self.subTest(case=index):
                self.output = self.root / f'clock-{index}'
                self.change = lambda m, f=change: f(m['render']['presentation_clock'])
                report = self.run_capture()
                self.assertEqual(report['status'], 'INVALID', report)
                self.assertTrue(any('presentation_clock' in error for error in report['errors']))
        for index, value in enumerate((None, [], 'clock', 1)):
            self.output = self.root / f'clock-object-{index}'
            self.change = lambda m, v=value: m['render'].update(presentation_clock=v)
            self.assertEqual(self.run_capture()['status'], 'INVALID')
        self.output = self.root / 'clock-missing'
        self.change = lambda m: m['render'].pop('presentation_clock')
        self.assertEqual(self.run_capture()['status'], 'INVALID')

    def test_neutral_input_evidence_is_required_and_strict(self):
        changes = [lambda r: r.pop('neutral_input'),
                   lambda r: r.update(neutral_input=None),
                   lambda r: r['neutral_input'].update(extra=True)]
        for key in ('static_default_cursor', 'camera_input_idle'):
            changes.append(lambda r, k=key: r['neutral_input'].pop(k))
            for value in (False, 1, 1.0, None, 'true'):
                changes.append(lambda r, k=key, v=value: r['neutral_input'].update({k: v}))
        for index, change in enumerate(changes):
            with self.subTest(case=index):
                self.output = self.root / f'neutral-{index}'
                self.change = lambda m, f=change: f(m['render'])
                self.assertEqual(self.run_capture()['status'], 'INVALID')

    def test_live_capture_never_accepts_legacy_clock(self):
        self.change = lambda m: (m.update(schema_version=observation.LEGACY_CHILD_SCHEMA),
                                 m['render'].pop('presentation_clock'),
                                 m['render'].pop('neutral_input'))
        report = self.run_capture()
        self.assertEqual(report['status'], 'INVALID')
        self.assertIn('schema_version', report['errors'][0])

    def test_loose_map_receipt_is_supported(self):
        self.change = lambda m: m['map_source'].update(kind='loose', path='/retail/Fight.MAP')
        self.assertEqual(self.run_capture()['status'], 'VALID')

    def test_existing_output_is_never_reused(self):
        self.output.mkdir()
        with patch.object(observation, 'run_child') as child:
            with self.assertRaises(OutputExistsError):
                self.run_capture()
            child.assert_not_called()

    def test_changed_contract_cannot_remove_guards(self):
        path = self.root / 'contract.json'
        document = json.loads(self.contract.read_text())
        document['environment_denylist'] = []
        path.write_text(json.dumps(document))
        self.contract = path
        with self.assertRaisesRegex(ValidationError, 'denylist'):
            self.run_capture()
        self.assertFalse(self.output.exists())

    def test_denied_environment_rejected_without_mutation(self):
        with patch.dict('os.environ', {'RA2_DIR': '/unexpected'}):
            with self.assertRaisesRegex(ValidationError, 'denied'):
                self.run_capture()
        self.assertFalse(self.output.exists())

    def test_contract_timeout_maximum_is_enforced_before_spawn(self):
        self.profile['timeout_seconds'] = 100000
        self.profile_path.write_text(json.dumps(self.profile))
        with self.assertRaisesRegex(ValidationError, 'maximum'):
            self.run_capture()

    def test_release_resolution_has_no_guessed_target_fallback(self):
        with patch.object(observation, 'resolve_binary', return_value=(None, None)) as resolver:
            with self.assertRaisesRegex(ValidationError, 'verified release'):
                observation.capture(profile_path=self.profile_path, contract_path=self.contract,
                                    output=self.output, working_directory=self.root)
            resolver.assert_called_once_with(observation.ROOT, 'vera20k', 'release')

    def test_failed_child_keeps_diagnostics_and_cannot_pass_valid_receipt(self):
        for result in (ChildResult(42, 2, False, b'out', b'failed', ()),
                       ChildResult(42, -9, True, b'out', b'timed out', ('timeout',)),
                       ChildResult(None, None, False, b'', b'', ('spawn failed',))):
            with self.subTest(result=result):
                self.output = self.root / f'run-{result.pid}-{result.exit_status}'
                self.result = result
                report = self.run_capture()
                self.assertEqual(report['status'], 'INVALID')
                self.assertTrue(report['errors'])
                self.assertEqual((self.output / 'stderr.log').read_bytes(), result.stderr)

    def test_receipt_tampering_fails_closed(self):
        cases = [('profile', 'sha256', '0' * 64), ('contract', 'sha256', '0' * 64),
                 ('final', 'simulation_tick', 2), ('final', 'binary_frame', 4),
                 ('initial', 'simulation_tick', 1), ('last_exact_step', 'tick_before', 1),
                 ('first_exact_step', 'binary_frame_after', 2),
                 ('frame', 'sha256', '0' * 64), ('frame', 'byte_length', 15),
                 ('map_source', 'kind', 'generated'), ('map_source', 'source_sha256', 'bad'),
                 ('lifecycle', 'input_violations', 1), ('render', 'ready', False),
                 ('render', 'internal_extent', [2.0, 2]),
                 ('startup', 'seed_source', 'Random')]
        for index, (section, key, value) in enumerate(cases):
            with self.subTest(section=section, key=key):
                self.output = self.root / f'tamper-{index}'
                self.change = lambda m, s=section, k=key, v=value: m[s].update({k: v})
                self.assertEqual(self.run_capture()['status'], 'INVALID')

    def test_input_changed_during_child_cannot_pass(self):
        self.change = lambda m: self.config.write_text('changed')
        report = self.run_capture()
        self.assertEqual(report['status'], 'INVALID')
        self.assertTrue(any('changed' in error for error in report['errors']))

    def test_frame_corruption_cannot_pass(self):
        self.change = lambda m: (self.output / 'child-output/frame.bgra').write_bytes(b'bad')
        self.assertEqual(self.run_capture()['status'], 'INVALID')

    def test_failed_manifest_and_missing_outputs_are_invalid(self):
        self.change = lambda m: (m.update(status='FAILED', failure={'stage': 'loading', 'message': 'load failed'}),
                                 (self.output / 'child-output/frame.bgra').unlink())
        report = self.run_capture()
        self.assertEqual(report['status'], 'INVALID')
        self.assertTrue(any('load failed' in error and 'loading' in error for error in report['errors']))
        self.output = self.root / 'missing'
        with patch.object(observation, 'run_child', return_value=self.result):
            report = observation.capture(profile_path=self.profile_path, contract_path=self.contract,
                                         output=self.output, working_directory=self.root,
                                         executable=self.executable)
        self.assertEqual(report['status'], 'INVALID')

    def valid_capture(self, name):
        self.output = self.root / name
        report = self.run_capture()
        self.assertEqual(report['status'], 'VALID', report['errors'])
        return self.output

    @staticmethod
    def edit_json(path, change):
        document = json.loads(path.read_text())
        change(document)
        path.write_text(json.dumps(document))

    @staticmethod
    def clock(ticks):
        return {'policy': 'map-exact-step-presentation-v1', 'origin_ms': 0, 'interval_ms': 22,
                'draws': [{'completed_steps': step, 'radar_ms': step * 22,
                           'tooltip_ms': step * 22, 'message_ms': step * 22}
                          for step in (range(1, ticks + 1) if ticks else [0])]}

    def make_legacy_clock(self, run):
        manifest = run / 'child-output/capture.json'
        def convert_child(document):
            document['schema_version'] = observation.LEGACY_CHILD_SCHEMA
            document['render'].pop('presentation_clock')
            document['render'].pop('neutral_input')
            document['render'].pop('camera')
            document.pop('observations')
        self.edit_json(manifest, convert_child)
        def convert_wrapper(document):
            document['schema_version'] = observation.LEGACY_CLOCK_RUN_SCHEMA
            document['capture'].pop('presentation_clock')
            document['capture'].pop('neutral_input')
            document['capture'].pop('camera')
            document['capture'].pop('observations')
            document['capture']['manifest'].update(byte_length=manifest.stat().st_size,
                                                    sha256=sha256_bytes(manifest.read_bytes()))
        self.edit_json(run / 'run.json', convert_wrapper)

    def make_legacy(self, run):
        self.make_legacy_clock(run)
        self.edit_json(run / 'run.json',
                       lambda report: report.update(schema_version=observation.LEGACY_RUN_SCHEMA))
        (run / 'config.toml').unlink()
        (run / 'contract.json').unlink()

    def test_historical_v3_is_explicitly_readable_without_clock_override(self):
        run = self.valid_capture('historical-v3')
        manifest = run / 'child-output/capture.json'
        def convert_child(document):
            document['schema_version'] = observation.PRIOR_CHILD_SCHEMA
            document.pop('observations')
            document['render'].pop('camera')
        self.edit_json(manifest, convert_child)
        def convert_wrapper(document):
            document['schema_version'] = observation.PRIOR_RUN_SCHEMA
            document['capture'].pop('observations')
            document['capture'].pop('camera')
            document['capture']['manifest'].update(byte_length=manifest.stat().st_size,
                                                    sha256=sha256_bytes(manifest.read_bytes()))
        self.edit_json(run / 'run.json', convert_wrapper)
        original = (run / 'run.json').read_bytes()
        report = observation.validate_run(run)
        self.assertEqual(report['status'], 'VALID', report['errors'])
        self.assertEqual(report['presentation_clock']['policy'], observation.CLOCK_POLICY)
        self.assertNotIn('observations', report['capture'])
        self.assertEqual((run / 'run.json').read_bytes(), original)
        current = self.valid_capture('current-v5')
        report = observation.compare_runs(run, current)
        self.assertEqual(report['status'], 'INVALID')
        self.assertIn('observation policies differ', report['errors'][0])
        self.edit_json(manifest, lambda value: value.update(observations={'policy': observation.OBSERVATION_POLICY}))
        self.assertEqual(observation.validate_run(run)['status'], 'INVALID')

    def make_historical_trajectory(self, run):
        manifest = run / 'child-output/capture.json'

        def observations(document):
            trajectory = document['observations']
            trajectory['policy'] = observation.TRAJECTORY_OBSERVATION_POLICY
            trajectory.pop('rule_types')
            for frame in trajectory['frames']:
                for actor in frame['actors']:
                    actor.pop('building')

        def convert_child(document):
            document['schema_version'] = observation.TRAJECTORY_CHILD_SCHEMA
            observations(document)

        self.edit_json(manifest, convert_child)

        def convert_wrapper(document):
            document['schema_version'] = observation.TRAJECTORY_RUN_SCHEMA
            observations(document['capture'])
            document['capture']['manifest'].update(byte_length=manifest.stat().st_size,
                                                   sha256=sha256_bytes(manifest.read_bytes()))

        self.edit_json(run / 'run.json', convert_wrapper)

    def test_historical_v4_trajectory_validates_unchanged_and_compares_only_same_policy(self):
        self.scripted_profile()
        before = self.valid_capture('historical-v4-before')
        after = self.valid_capture('historical-v4-after')
        self.make_historical_trajectory(before)
        self.make_historical_trajectory(after)
        originals = {path: path.read_bytes() for path in
                     (before / 'run.json', before / 'child-output/capture.json')}
        report = observation.validate_run(before)
        self.assertEqual(report['status'], 'VALID', report['errors'])
        self.assertNotIn('rule_types', report['capture']['observations'])
        self.assertNotIn('building', report['capture']['observations']['frames'][0]['actors'][0])
        report = observation.compare_runs(before, after)
        self.assertEqual(report['status'], 'MATCH', report['errors'])
        current = self.valid_capture('current-building-state')
        report = observation.compare_runs(before, current)
        self.assertEqual(report['status'], 'INVALID')
        self.assertIn('observation policies differ', report['errors'][0])
        for path, raw in originals.items():
            self.assertEqual(path.read_bytes(), raw)

    def test_historical_v4_retains_optional_terrain_resource_fields_unchanged(self):
        self.scripted_profile()
        cell = self.unallocated_cell([87, 53])
        cell.update(overlay={'id': 102, 'density': 3},
                    terrain_object={'name': 'TREE01', 'frame': None, 'active': None})
        self.terrain_frames[1] = [cell]
        before = self.valid_capture('historical-v4-terrain-before')
        after = self.valid_capture('historical-v4-terrain-after')
        for run in (before, after):
            self.make_historical_trajectory(run)
        paths = [run / name for run in (before, after)
                 for name in ('run.json', 'child-output/capture.json')]
        originals = {path: path.read_bytes() for path in paths}
        report = observation.validate_run(before)
        self.assertEqual(report['status'], 'VALID', report['errors'])
        self.assertEqual(report['capture']['observations']['frames'][1]['terrain'], [cell])
        report = observation.compare_runs(before, after)
        self.assertEqual(report['status'], 'MATCH', report['errors'])
        for path, raw in originals.items():
            self.assertEqual(path.read_bytes(), raw)

    def test_historical_v4_rejects_v5_fields_and_production_commands(self):
        self.scripted_profile()
        for index, change in enumerate((
                lambda m: m['observations'].update(rule_types=self.rule_types),
                lambda m: m['observations']['frames'][0]['actors'][0].update(building=None))):
            run = self.valid_capture(f'historical-v4-smuggle-{index}')
            self.make_historical_trajectory(run)
            self.edit_json(run / 'child-output/capture.json', change)
            self.assertEqual(observation.validate_run(run)['status'], 'INVALID')
        self.production_profile()
        run = self.valid_capture('historical-v4-production')
        self.make_historical_trajectory(run)
        report = observation.validate_run(run)
        self.assertEqual(report['status'], 'INVALID')
        self.assertIn('outside ordinary order coverage', report['errors'][0])

    def test_wrapper_v5_requires_child_v5_and_new_trajectory_fields(self):
        for index, change in enumerate((
                lambda m: m.update(schema_version=observation.TRAJECTORY_CHILD_SCHEMA),
                lambda m: m['observations'].update(policy=observation.TRAJECTORY_OBSERVATION_POLICY))):
            self.output = self.root / f'v5-generation-mismatch-{index}'
            self.change = change
            self.assertEqual(self.run_capture()['status'], 'INVALID')

    def test_map_receipt_read_and_write_limits_are_explicit(self):
        # Small overrides exercise both bounded paths without allocating 128 MiB.
        with patch.object(observation, 'MAX_RECEIPT_BYTES', 1):
            with self.assertRaisesRegex(ValidationError, 'exceeds'):
                self.run_capture()
        run = self.valid_capture('bounded-receipt')
        with patch.object(observation, 'MAX_RECEIPT_BYTES', 1):
            report = observation.validate_run(run)
            self.assertEqual(report['status'], 'INVALID')
            self.assertIn('too large', report['errors'][0])

    def test_sealed_run_revalidates_without_original_profile_config_or_contract(self):
        original_contract = self.root / 'original-contract.json'
        original_contract.write_bytes(self.contract.read_bytes())
        self.contract = original_contract
        run = self.valid_capture('sealed')
        for path in (self.profile_path, self.config, self.contract):
            path.unlink()
        report = observation.validate_run(run)
        self.assertEqual(report['status'], 'VALID', report['errors'])
        self.assertEqual(report['input_provenance'], {
            'profile': 'SEALED_COPY', 'config': 'SEALED_COPY', 'contract': 'SEALED_COPY',
            'executable': 'EXTERNALLY_REVALIDATED'})
        self.assertEqual(report['capture']['unit_atlas'], self.unit_atlas)

    def test_capture_detects_each_retained_copy_changed_by_child(self):
        for name, filename in observation.COPIES.items():
            with self.subTest(input=name):
                self.output = self.root / f'copy-changed-{name}'
                self.change = lambda m, f=filename: (self.output / f).write_bytes(b'changed copy')
                report = self.run_capture()
                self.assertEqual(report['status'], 'INVALID')
                self.assertTrue(any(f'{name} copy' in error for error in report['errors']))

    def test_offline_validator_reads_every_retained_artifact(self):
        artifacts = ('profile.json', 'config.toml', 'contract.json', 'stdout.log', 'stderr.log',
                     'child-output/capture.json', 'child-output/frame.bgra')
        for index, artifact in enumerate(artifacts):
            with self.subTest(artifact=artifact):
                run = self.valid_capture(f'tamper-artifact-{index}')
                path = run / artifact
                path.write_bytes(path.read_bytes() + b'changed')
                report = observation.validate_run(run)
                self.assertEqual(report['status'], 'INVALID')
                self.assertTrue(report['errors'])
        run = self.valid_capture('missing-frame')
        (run / 'child-output/frame.bgra').unlink()
        self.assertEqual(observation.validate_run(run)['status'], 'INVALID')

    def test_offline_validator_rechecks_child_semantics_after_manifest_rehash(self):
        run = self.valid_capture('semantic-corruption')
        manifest = run / 'child-output/capture.json'
        self.edit_json(manifest, lambda value: value['lifecycle'].update(input_violations=1))
        self.edit_json(run / 'run.json', lambda value: value['capture']['manifest'].update(
            byte_length=manifest.stat().st_size, sha256=sha256_bytes(manifest.read_bytes())))
        report = observation.validate_run(run)
        self.assertEqual(report['status'], 'INVALID')
        self.assertIn('input_violations', report['errors'][0])

    def test_clock_semantics_survive_consistent_child_and_wrapper_rehash(self):
        run = self.valid_capture('clock-rehash')
        manifest = run / 'child-output/capture.json'
        self.edit_json(manifest, lambda m: m['render']['presentation_clock']['draws'][1].update(
            message_ms=43))
        def rehash(report):
            report['capture']['manifest'].update(byte_length=manifest.stat().st_size,
                                                 sha256=sha256_bytes(manifest.read_bytes()))
            report['capture']['presentation_clock']['draws'][1]['message_ms'] = 43
        self.edit_json(run / 'run.json', rehash)
        report = observation.validate_run(run)
        self.assertEqual(report['status'], 'INVALID')
        self.assertIn('draws[1].message_ms', report['errors'][0])

    def test_legacy_clock_permission_is_separate_and_projection_stays_historical(self):
        run = self.valid_capture('wall-clock')
        self.make_legacy_clock(run)
        original = (run / 'run.json').read_bytes()
        for options in ({}, {'allow_legacy_inputs': True}):
            report = observation.validate_run(run, **options)
            self.assertEqual(report['status'], 'INVALID')
            self.assertIn('--allow-legacy-clock', report['errors'][0])
        report = observation.validate_run(run, allow_legacy_clock=True)
        self.assertEqual(report['status'], 'VALID', report['errors'])
        self.assertEqual(report['presentation_clock'], {'policy': 'legacy-wall-clock'})
        self.assertNotIn('presentation_clock', report['capture'])
        self.assertNotIn('neutral_input', report['capture'])
        self.assertEqual((run / 'run.json').read_bytes(), original)

    def test_legacy_clock_cannot_smuggle_diagnostic_guarantees(self):
        for key, value in (('presentation_clock', self.clock(3)),
                           ('neutral_input', {'static_default_cursor': True, 'camera_input_idle': True})):
            run = self.valid_capture(f'legacy-smuggle-{key}')
            self.make_legacy_clock(run)
            self.edit_json(run / 'child-output/capture.json',
                           lambda m, k=key, v=value: m['render'].update({k: v}))
            report = observation.validate_run(run, allow_legacy_clock=True)
            self.assertEqual(report['status'], 'INVALID')
            self.assertIn('legacy child v2', report['errors'][0])

    def test_wrapper_and_child_clock_versions_must_correspond(self):
        for index, (wrapper, child) in enumerate((
                (observation.LEGACY_CLOCK_RUN_SCHEMA, observation.CHILD_SCHEMA),
                (observation.RUN_SCHEMA, observation.LEGACY_CHILD_SCHEMA),
                (observation.LEGACY_RUN_SCHEMA, observation.CHILD_SCHEMA))):
            run = self.valid_capture(f'wrong-generation-{index}')
            if wrapper == observation.LEGACY_RUN_SCHEMA:
                self.make_legacy(run)
            self.edit_json(run / 'run.json', lambda r, v=wrapper: r.update(schema_version=v))
            self.edit_json(run / 'child-output/capture.json',
                           lambda m, v=child: m.update(schema_version=v))
            report = observation.validate_run(run, allow_legacy_inputs=True, allow_legacy_clock=True)
            self.assertEqual(report['status'], 'INVALID')
            self.assertIn('schema_version', report['errors'][0])

    def test_mixed_clock_policies_are_invalid_even_with_equal_frame_bytes(self):
        before = self.valid_capture('clock-before')
        after = self.valid_capture('clock-after')
        self.make_legacy_clock(before)
        self.assertEqual((before / 'child-output/frame.bgra').read_bytes(),
                         (after / 'child-output/frame.bgra').read_bytes())
        report = observation.compare_runs(before, after, allow_legacy_clock=True)
        self.assertEqual(report['status'], 'INVALID')
        self.assertIn('presentation_clock', report['errors'][0])
        self.make_legacy_clock(after)
        report = observation.compare_runs(before, after, allow_legacy_clock=True)
        self.assertEqual(report['status'], 'MATCH', report['errors'])

    def test_offline_validator_rejects_receipt_inconsistency_and_bad_types(self):
        changes = [lambda r: r.update(status='INVALID'),
                   lambda r: r.update(errors=['capture failed']),
                   lambda r: r.update(schema_version='unknown'),
                   lambda r: r.update(native_comparator='NATIVE'),
                   lambda r: r['child'].update(exit_status=False),
                   lambda r: r['child'].update(timed_out=True),
                   lambda r: r['child'].update(pid=None),
                   lambda r: r['capture']['final'].update(deterministic_state_hash=10.0),
                   lambda r: r['capture']['unit_atlas']['pages'][0].update(sample_count=True),
                   lambda r: r['inputs']['config'].update(path='/wrong/config.toml'),
                   lambda r: r['inputs']['executable'].update(sha256='x' * 64),
                   lambda r: r['inputs']['profile'].pop('byte_length'),
                   lambda r: r.update(command=['some other command']),
                   lambda r: r.update(capture=None)]
        for index, change in enumerate(changes):
            with self.subTest(case=index):
                run = self.valid_capture(f'bad-receipt-{index}')
                self.edit_json(run / 'run.json', change)
                report = observation.validate_run(run)
                self.assertEqual(report['status'], 'INVALID')
                self.assertTrue(report['errors'])
        run = self.valid_capture('duplicate-json')
        (run / 'run.json').write_text('{"status":"VALID","status":"VALID"}')
        self.assertEqual(observation.validate_run(run)['status'], 'INVALID')

    def test_live_and_offline_nested_profile_types_are_strict(self):
        # bool is equal to integer 1 in Python, but not in the recorded launch DTO.
        self.change = lambda m: m['profile']['request']['launch']['options'].update(bases=1)
        report = self.run_capture()
        self.assertEqual(report['status'], 'INVALID')
        self.assertTrue(any('bases' in error for error in report['errors']))

    def test_legacy_requires_explicit_original_input_revalidation(self):
        run = self.valid_capture('legacy')
        self.make_legacy(run)
        rejected = observation.validate_run(run)
        self.assertEqual(rejected['status'], 'INVALID')
        self.assertIn('--allow-legacy-inputs', rejected['errors'][0])
        rejected = observation.validate_run(run, allow_legacy_inputs=True)
        self.assertEqual(rejected['status'], 'INVALID')
        self.assertIn('--allow-legacy-clock', rejected['errors'][0])
        rejected = observation.validate_run(run, allow_legacy_clock=True)
        self.assertEqual(rejected['status'], 'INVALID')
        self.assertIn('--allow-legacy-inputs', rejected['errors'][0])
        report = observation.validate_run(run, allow_legacy_inputs=True, allow_legacy_clock=True)
        self.assertEqual(report['status'], 'VALID', report['errors'])
        self.assertEqual(report['input_provenance']['config'], 'EXTERNALLY_REVALIDATED_UNSEALED')
        self.assertEqual(report['input_provenance']['contract'], 'EXTERNALLY_REVALIDATED_UNSEALED')
        self.assertEqual(report['input_provenance']['profile'], 'SEALED_COPY')
        self.config.write_text('changed after original capture')
        self.assertEqual(observation.validate_run(run, allow_legacy_inputs=True, allow_legacy_clock=True)['status'], 'INVALID')
        self.config.unlink()
        self.assertEqual(observation.validate_run(run, allow_legacy_inputs=True, allow_legacy_clock=True)['status'], 'INVALID')

    def test_legacy_contract_cannot_be_replaced_or_silently_resealed(self):
        original_contract = self.root / 'legacy-contract.json'
        original_contract.write_bytes(self.contract.read_bytes())
        self.contract = original_contract
        run = self.valid_capture('legacy-contract')
        self.make_legacy(run)
        self.contract.write_bytes(self.contract.read_bytes() + b'\n')
        report = observation.validate_run(run, allow_legacy_inputs=True, allow_legacy_clock=True)
        self.assertEqual(report['status'], 'INVALID')
        self.assertIn('contract', report['errors'][0])
        self.contract.unlink()
        self.assertEqual(observation.validate_run(run, allow_legacy_inputs=True, allow_legacy_clock=True)['status'], 'INVALID')

    def test_offline_contract_semantics_survive_consistent_rehashing(self):
        run = self.valid_capture('contract-guards')
        contract = run / 'contract.json'
        self.edit_json(contract, lambda value: value.update(environment_denylist=[]))
        digest = sha256_bytes(contract.read_bytes())
        self.edit_json(run / 'run.json', lambda value: value['inputs']['contract'].update(
            sha256=digest, byte_length=contract.stat().st_size))
        self.edit_json(run / 'child-output/capture.json',
                       lambda value: value['contract'].update(sha256=digest))
        report = observation.validate_run(run)
        self.assertEqual(report['status'], 'INVALID')
        self.assertIn('denylist', report['errors'][0])

    def test_original_executable_must_still_exist_and_match(self):
        run = self.valid_capture('binary-evidence')
        self.executable.write_bytes(b'different executable')
        self.assertEqual(observation.validate_run(run)['status'], 'INVALID')
        self.executable.unlink()
        self.assertEqual(observation.validate_run(run)['status'], 'INVALID')

    def test_comparison_matches_verified_different_binaries(self):
        before = self.valid_capture('before')
        self.executable = self.root / 'new-game'
        self.executable.write_bytes(b'new executable identity')
        after = self.valid_capture('after')
        report = observation.compare_runs(before, after)
        self.assertEqual(report['status'], 'MATCH', report['errors'])
        self.assertEqual(report['differences'], [])
        self.assertNotEqual(report['before']['inputs']['executable']['sha256'],
                            report['after']['inputs']['executable']['sha256'])
        self.assertEqual(report['native_comparator'], 'NONE')
        self.assertEqual(report['parity_certification'], 'NONE')

    def test_comparison_reports_meaningful_valid_differences(self):
        before = self.valid_capture('difference-before')
        changes = [('initial.deterministic_state_hash',
                    lambda m: m['initial'].update(deterministic_state_hash=8)),
                   ('final.deterministic_state_hash',
                    lambda m: m['final'].update(deterministic_state_hash=12)),
                   ('map_source.source_sha256',
                    lambda m: m['map_source'].update(source_sha256='b' * 64)),
                   ('unit_atlas.resident_sprite_count',
                    lambda m: m['render']['unit_atlas'].update(resident_sprite_count=8)),
                   ('frame.surface_format',
                    lambda m: m['frame'].update(surface_format='Bgra8Unorm'))]
        for index, (field, change) in enumerate(changes):
            with self.subTest(field=field):
                self.change = change
                after = self.valid_capture(f'difference-after-{index}')
                report = observation.compare_runs(before, after)
                self.assertEqual(report['status'], 'MISMATCH', report['errors'])
                self.assertEqual([d['field'] for d in report['differences']], [field])
        self.change = lambda m: None
        self.frame = bytes(reversed(self.frame))
        after = self.valid_capture('different-frame')
        report = observation.compare_runs(before, after)
        self.assertEqual(report['status'], 'MISMATCH', report['errors'])
        self.assertEqual([d['field'] for d in report['differences']], ['frame.bytes'])

    def test_comparison_requires_same_input_bytes(self):
        before = self.valid_capture('input-before')
        self.config.write_text('different production settings')
        after = self.valid_capture('config-after')
        report = observation.compare_runs(before, after)
        self.assertEqual(report['status'], 'INVALID')
        self.assertIn('config input bytes differ', report['errors'][0])
        self.config.write_text('[paths]\nra2_dir="fixture"\n')
        self.profile['ticks'] = 4
        self.profile_path.write_text(json.dumps(self.profile))
        after = self.valid_capture('profile-after')
        report = observation.compare_runs(before, after)
        self.assertEqual(report['status'], 'INVALID')
        self.assertIn('profile input bytes differ', report['errors'][0])

    def test_comparison_checks_frame_bytes_instead_of_copied_wrapper_hash(self):
        before = self.valid_capture('raw-before')
        after = self.valid_capture('raw-after')
        (after / 'child-output/frame.bgra').write_bytes(b'changed raw data!')
        report = observation.compare_runs(before, after)
        self.assertEqual(report['status'], 'INVALID')
        self.assertTrue(report['errors'])

    def test_comparison_rechecks_first_run_after_reading_second(self):
        before = self.valid_capture('race-before')
        after = self.valid_capture('race-after')
        load = observation._load_run

        def load_and_change(directory, allow_legacy, allow_clock):
            result = load(directory, allow_legacy, allow_clock)
            if directory == after:
                (before / 'config.toml').write_bytes(b'changed during comparison')
            return result

        with patch.object(observation, '_load_run', side_effect=load_and_change):
            report = observation.compare_runs(before, after)
        self.assertEqual(report['status'], 'INVALID')
        self.assertTrue(any('changed' in error for error in report['errors']))

    def test_same_alias_and_relocated_runs_are_rejected(self):
        run = self.valid_capture('location')
        report = observation.compare_runs(run, run / '.')
        self.assertEqual(report['status'], 'INVALID')
        self.assertIn('distinct', report['errors'][0])
        alias = self.root / 'alias'
        try:
            alias.symlink_to(run, target_is_directory=True)
        except OSError:
            pass  # Windows can require a privilege for symlink creation.
        else:
            self.assertEqual(observation.compare_runs(run, alias)['status'], 'INVALID')
        copied = self.root / 'copied'
        shutil.copytree(run, copied)
        report = observation.validate_run(copied)
        self.assertEqual(report['status'], 'INVALID')
        self.assertIn('run.command', report['errors'][0])

    def test_legacy_comparison_is_explicit_and_child_v1_remains_unsupported(self):
        before = self.valid_capture('legacy-before')
        after = self.valid_capture('legacy-after')
        self.make_legacy(before)
        self.make_legacy_clock(after)
        self.assertEqual(observation.compare_runs(before, after)['status'], 'INVALID')
        report = observation.compare_runs(before, after, allow_legacy_inputs=True, allow_legacy_clock=True)
        self.assertEqual(report['status'], 'MATCH', report['errors'])
        self.edit_json(before / 'child-output/capture.json',
                       lambda m: m.update(schema_version='vera20k.map-observation.v1'))
        report = observation.compare_runs(before, after, allow_legacy_inputs=True, allow_legacy_clock=True)
        self.assertEqual(report['status'], 'INVALID')
        self.assertIn('schema_version', report['errors'][0])

    def test_labeled_capture_uses_shared_resolver_and_never_falls_back(self):
        with patch.object(observation, 'resolve_labeled_binary',
                          return_value=(self.executable, 'release')) as resolver, \
             patch.object(observation, 'resolve_binary') as latest, \
             patch.object(observation, 'run_child', side_effect=self.fake_child):
            report = observation.capture(profile_path=self.profile_path, contract_path=self.contract,
                                         output=self.output, working_directory=self.root,
                                         build_label='before-build')
        self.assertEqual(report['status'], 'VALID', report['errors'])
        resolver.assert_called_once_with(observation.ROOT, 'before-build', 'vera20k', 'release')
        latest.assert_not_called()
        with patch.object(observation, 'resolve_labeled_binary', return_value=(None, None)), \
             patch.object(observation, 'resolve_binary') as latest:
            with self.assertRaisesRegex(ValidationError, 'build label'):
                observation.capture(profile_path=self.profile_path, contract_path=self.contract,
                                    output=self.root / 'missing-label', working_directory=self.root,
                                    build_label='missing')
            latest.assert_not_called()
        with self.assertRaisesRegex(ValidationError, 'mutually exclusive'):
            observation.capture(profile_path=self.profile_path, contract_path=self.contract,
                                output=self.root / 'conflicting-selector', working_directory=self.root,
                                executable=self.executable, build_label='before-build')

    def test_cli_checks_have_distinct_verdicts_and_exclusive_outputs(self):
        before = self.valid_capture('cli-before')
        after = self.valid_capture('cli-after')
        output = self.root / 'comparison.json'
        arguments = ['compare', '--before', str(before), '--after', str(after),
                     '--output', str(output)]
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(observation.main(arguments), 0)
            self.assertEqual(json.loads(output.read_text())['status'], 'MATCH')
            previous_bytes = output.read_bytes()
            self.assertEqual(observation.main(arguments), 2)
            self.assertEqual(output.read_bytes(), previous_bytes)
            self.frame = b'changed frame!!!'
            changed = self.valid_capture('cli-changed')
            self.assertEqual(observation.main(['compare', '--before', str(before),
                                               '--after', str(changed), '--output',
                                               str(self.root / 'mismatch.json')]), 1)
            self.assertEqual(observation.main(['validate', '--run', str(before), '--output',
                                               str(self.root / 'validation.json')]), 0)
            (after / 'child-output/frame.bgra').unlink()
            self.assertEqual(observation.main(['validate', '--run', str(after), '--output',
                                               str(self.root / 'invalid.json')]), 2)
            self.assertEqual(observation.main(['validate', '--run', str(before), '--output',
                                               str(before / 'forbidden-report.json')]), 2)
            self.assertFalse((before / 'forbidden-report.json').exists())

    def test_cli_legacy_clock_flag_is_offline_and_explicit(self):
        run = self.valid_capture('cli-legacy-clock')
        self.make_legacy_clock(run)
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(observation.main(['validate', '--run', str(run), '--output',
                                               str(self.root / 'clock-denied.json')]), 2)
            self.assertEqual(observation.main(['validate', '--run', str(run),
                                               '--allow-legacy-clock', '--output',
                                               str(self.root / 'clock-allowed.json')]), 0)
            with self.assertRaises(SystemExit) as error:
                observation.main(['--allow-legacy-clock', '--profile', str(self.profile_path),
                                  '--contract', str(self.contract), '--output', str(self.root / 'unused')])
            self.assertEqual(error.exception.code, 2)
        report = json.loads((self.root / 'clock-allowed.json').read_text())
        self.assertEqual(report['presentation_clock']['policy'], 'legacy-wall-clock')
        self.assertEqual(report['parity_certification'], 'NONE')


if __name__ == '__main__':
    unittest.main()
