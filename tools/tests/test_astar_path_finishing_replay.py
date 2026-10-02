"""Portable hash-attribution receipts; no gameplay or native golden producer."""
from copy import deepcopy
import contextlib
import gzip
import hashlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from tools.spatial_oracle import astar_path_finishing_replay as replay


class HashCompositionReplayTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.path = self.root / 'receipt.json'
        self.binary_sha = self.digest(b'one saved test binary')
        self.source_sha = self.digest(b'its exact build source')
        self.receipt = {
            'schema_version': 1, 'kind': 'same-binary-rust-hash-composition',
            'binary': {'manifest_file': 'manifest.json', 'sha256': self.binary_sha,
                       'source_sha256': self.source_sha},
            'gate': {'file': 'gate.patch.gz', 'sha256': self.digest(b'test-only hash gate\n'),
                     'environment': 'VERA20K_DIAGNOSTIC_LEGACY_BUILDING_HASH'},
            'runs': {mode: {'receipt_file': f'{mode}.execution.json'}
                     for mode in ('control', 'current')},
            'inputs': [], 'incoming_main': {'head': '1234567', 'pins': {'custom': 70}},
            'comparisons': [{'fixture': 'custom', 'rows': 2, 'control_hash': 70,
                             'current_hash': 80}],
        }
        manifest = {'schema': 1, 'source': {'source_sha256': self.source_sha},
                    'artifacts': [{'file': '0/testbinary', 'source': '/build/testbinary',
                                   'sha256': self.binary_sha}]}
        self.retain('manifest.json', self.encode(manifest))
        self.retain('gate.patch.gz', b'test-only hash gate\n')
        self.retain('build.log.gz', b'recorded build output\n')
        self.observations = {}
        for mode, gate, state_hash, exit_code in (('control', '1', 70, 0),
                                                 ('current', None, 80, 101)):
            execution = {'schema_version': 1, 'mode': mode,
                         'command': ['/preserved/testbinary', '--nocapture', 'custom_replay'],
                         'cwd': '/original/checkout', 'exit_code': exit_code, 'seconds': 0.5,
                         'binary_sha256': self.binary_sha, 'binary_unchanged': True,
                         'source_sha256': self.source_sha, 'legacy_building_hash': gate}
            self.retain(f'{mode}.execution.json', self.encode(execution))
            seed = {'next_frame': 0, 'tick_result': None, 'commands': [],
                    'entities': [{'id': 1, 'health': 125, 'navigation': {'path': [[1, 2]]}}],
                    'logic_order': [1], 'fire_events_accumulated': [],
                    'lifecycle_outputs_accumulated': [],
                    'rng': {'scenario': {'index': 0, 'table': [11, 22]},
                            'main': {'index': 0, 'table': [33, 44]},
                            'mapgen': {'index': 0, 'table': [55, 66]}},
                    'draws': [{'stream': 'main', 'raw': 7, 'callers': 'owner.rs:10:2'}],
                    'retained_path_inputs': {'navigation_present': True, 'team_count': 2}}
            tick = deepcopy(seed)
            tick.update(next_frame=1, tick_result={'state_hash': state_hash, 'tick': 1,
                                                 'committed': True})
            self.observations[mode] = [seed, tick]
            self.retain(f'{mode}.jsonl.gz', self.lines(self.observations[mode]),
                        mode=mode, fixture='custom')
        self.save_receipt()

    @staticmethod
    def digest(data):
        return hashlib.sha256(data).hexdigest()

    @staticmethod
    def encode(document):
        return json.dumps(document, sort_keys=True).encode()

    @classmethod
    def lines(cls, observations):
        return b''.join(cls.encode(row) + b'\n' for row in observations)

    def retain(self, name, data, **metadata):
        saved = gzip.compress(data, mtime=0) if name.endswith('.gz') else data
        (self.root / name).write_bytes(saved)
        item = next((item for item in self.receipt['inputs'] if item['file'] == name), None)
        if item is None:
            item = {'file': name, **metadata}
            self.receipt['inputs'].append(item)
        item.update(sha256=self.digest(saved), raw_sha256=self.digest(data))

    def save_receipt(self):
        self.path.write_bytes(self.encode(self.receipt))

    def rewrite_json(self, name, change):
        document = json.loads((self.root / name).read_bytes())
        change(document)
        self.retain(name, self.encode(document))
        self.save_receipt()

    def check(self):
        return replay.check_hash_composition(self.path)

    def test_generic_fixture_compares_every_field_except_tick_hash_and_leaves_bytes_unchanged(self):
        originals = {path: path.read_bytes() for path in self.root.iterdir()}
        result = self.check()
        self.assertEqual(result['comparisons'], self.receipt['comparisons'])
        self.assertEqual(result['normalization'], ['tick_result.state_hash only'])
        self.assertEqual(result['comparison_sha256'], self.digest(replay._canonical(result['comparisons'])))
        for path, data in originals.items():
            self.assertEqual(path.read_bytes(), data)

    def use_unit_deploy_gate(self):
        self.receipt['gate']['environment'] = 'VERA20K_DIAGNOSTIC_LEGACY_UNIT_DEPLOY_HASH'
        for mode in ('control', 'current'):
            def change(document):
                document['legacy_unit_deploy_hash'] = document.pop('legacy_building_hash')
                document['exit_code'] = 0
            self.rewrite_json(f'{mode}.execution.json', change)
        self.save_receipt()

    def test_unit_deploy_gate_requires_its_own_field_and_two_passing_runs(self):
        self.use_unit_deploy_gate()
        self.assertEqual(self.check()['execution_exit_codes'], {'control': 0, 'current': 0})
        for mode in ('control', 'current'):
            self.rewrite_json(f'{mode}.execution.json', lambda doc: doc.update(exit_code=101))
            with self.assertRaisesRegex(ValueError, 'execution exit'):
                self.check()
            self.rewrite_json(f'{mode}.execution.json', lambda doc: doc.update(exit_code=0))
        self.rewrite_json('current.execution.json',
                          lambda doc: doc.update(legacy_unit_deploy_hash='1'))
        with self.assertRaisesRegex(ValueError, 'gate identity differs'):
            self.check()
        self.rewrite_json('current.execution.json',
                          lambda doc: doc.update(legacy_unit_deploy_hash=None,
                                                 legacy_building_hash=None))
        with self.assertRaises(ValueError):
            self.check()

    def test_unknown_gate_is_rejected_without_weakening_historical_contract(self):
        self.receipt['gate']['environment'] = 'VERA20K_DIAGNOSTIC_UNRECORDED_HASH'
        self.save_receipt()
        with self.assertRaisesRegex(ValueError, 'not a supported hash-composition control'):
            self.check()

    def test_actor_rng_draw_and_caller_changes_reject_after_valid_input_rehash(self):
        changes = (
            lambda row: row['entities'][0].update(health=124),
            lambda row: row['rng']['main']['table'].__setitem__(1, 45),
            lambda row: row['draws'][0].update(raw=8),
            lambda row: row['draws'][0].update(callers='owner.rs:11:2'),
            lambda row: row['tick_result'].update(committed=False),
        )
        for change in changes:
            with self.subTest(change=change):
                observations = deepcopy(self.observations['current'])
                change(observations[1])
                self.retain('current.jsonl.gz', self.lines(observations))
                self.save_receipt()
                with self.assertRaisesRegex(ValueError, 'control/current gameplay delta'):
                    self.check()

    def test_every_retained_input_is_hashed_even_non_observation_logs(self):
        for item in self.receipt['inputs']:
            with self.subTest(file=item['file']):
                path = self.root / item['file']
                data = path.read_bytes()
                path.write_bytes(data + b'changed saved bytes')
                try:
                    with self.assertRaisesRegex(ValueError, 'input SHA mismatch'):
                        self.check()
                finally:
                    path.write_bytes(data)

    def test_raw_gzip_identity_and_gate_patch_identity_are_independent(self):
        item = next(item for item in self.receipt['inputs'] if item['file'] == 'current.jsonl.gz')
        item['raw_sha256'] = 'a' * 64
        self.save_receipt()
        with self.assertRaisesRegex(ValueError, 'raw input SHA mismatch'):
            self.check()
        item['raw_sha256'] = self.digest(self.lines(self.observations['current']))
        self.receipt['gate']['sha256'] = 'a' * 64
        self.save_receipt()
        with self.assertRaisesRegex(ValueError, 'gate patch SHA mismatch'):
            self.check()

    def test_escaping_and_alias_duplicate_input_paths_reject(self):
        original = deepcopy(self.receipt['inputs'])
        cases = [dict(original[0], file='../manifest.json'),
                 dict(original[0], file=str(self.root / 'manifest.json')),
                 dict(original[0], file='./manifest.json')]
        for item in cases:
            with self.subTest(file=item['file']):
                self.receipt['inputs'] = deepcopy(original) + [item]
                self.save_receipt()
                with self.assertRaisesRegex(ValueError, 'input path'):
                    self.check()

    def test_execution_identity_gate_command_cwd_and_control_exit_are_checked(self):
        path = self.root / 'control.execution.json'
        original = path.read_bytes()
        changes = (
            lambda doc: doc.update(binary_sha256='a' * 64),
            lambda doc: doc.update(source_sha256='a' * 64),
            lambda doc: doc.update(binary_unchanged=False),
            lambda doc: doc.update(legacy_building_hash=None),
            lambda doc: doc.update(command=['/preserved/testbinary', 'different_replay']),
            lambda doc: doc.update(cwd='/another/checkout'),
            lambda doc: doc.update(exit_code=101),
        )
        for change in changes:
            with self.subTest(change=change):
                self.retain(path.name, original)
                self.rewrite_json(path.name, change)
                with self.assertRaises(ValueError):
                    self.check()

    def test_current_gate_is_null_and_manifest_binary_source_must_match(self):
        self.rewrite_json('current.execution.json', lambda doc: doc.update(legacy_building_hash='1'))
        with self.assertRaisesRegex(ValueError, 'gate identity differs'):
            self.check()
        self.rewrite_json('current.execution.json', lambda doc: doc.update(legacy_building_hash=None))
        original = (self.root / 'manifest.json').read_bytes()
        for change in (lambda doc: doc['source'].update(source_sha256='a' * 64),
                       lambda doc: doc['artifacts'][0].update(sha256='a' * 64)):
            self.retain('manifest.json', original)
            self.rewrite_json('manifest.json', change)
            with self.assertRaises(ValueError):
                self.check()

    def test_recorded_absolute_paths_are_independent_of_the_checking_host(self):
        for command, cwd in (('/preserved/testbinary', '/original/checkout'),
                             (r'C:\preserved\testbinary.exe', r'C:\original\checkout'),
                             (r'\\server\share\testbinary.exe', r'\\server\share\checkout')):
            with self.subTest(command=command, cwd=cwd):
                for mode in ('control', 'current'):
                    self.rewrite_json(f'{mode}.execution.json',
                                      lambda doc: doc.update(command=[command, '--nocapture',
                                                                       'custom_replay'], cwd=cwd))
                self.check()

    def test_recorded_relative_paths_reject_in_either_platform_syntax(self):
        original = (self.root / 'control.execution.json').read_bytes()
        for field in ('command', 'cwd'):
            for path in ('relative/path', r'C:relative\path', r'\relative\path'):
                with self.subTest(field=field, path=path):
                    self.retain('control.execution.json', original)
                    self.rewrite_json('control.execution.json',
                                      lambda doc: doc.update(**{field: [path, '--nocapture',
                                                                       'custom_replay']
                                                               if field == 'command' else path}))
                    with self.assertRaisesRegex(ValueError, 'command/cwd must be absolute'):
                        self.check()

    def test_pins_and_comparisons_cannot_replace_control_reproduction(self):
        self.receipt['incoming_main']['pins']['custom'] = 71
        self.save_receipt()
        with self.assertRaisesRegex(ValueError, 'control does not reproduce incoming pin'):
            self.check()
        self.receipt['incoming_main']['pins']['custom'] = 70
        self.receipt['comparisons'][0]['current_hash'] = 81
        self.save_receipt()
        with self.assertRaisesRegex(ValueError, 'comparison receipt changed'):
            self.check()

    def test_fixture_coverage_schema_and_no_other_normalization_are_strict(self):
        original = deepcopy(self.receipt)
        changes = (lambda doc: doc.update(schema_version=True),
                   lambda doc: doc.update(final_followups=[]),
                   lambda doc: doc.update(normalization=['draw.callers .rs line/column only']),
                   lambda doc: doc['incoming_main']['pins'].update(missing=90),
                   lambda doc: doc['comparisons'][0].update(rows=True),
                   lambda doc: doc['comparisons'].append(deepcopy(doc['comparisons'][0])),
                   lambda doc: doc['inputs'][0].update(mode='control'),
                   lambda doc: doc['comparisons'][0].update(control_hash=True))
        for change in changes:
            with self.subTest(change=change):
                self.receipt = deepcopy(original)
                change(self.receipt)
                self.save_receipt()
                with self.assertRaises(ValueError):
                    self.check()

    def cli(self, *arguments):
        with patch('sys.argv', ['astar_path_finishing_replay', '--check', *arguments]), \
                contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            replay.main()

    def test_historical_parser_choice_is_preserved(self):
        with patch.object(replay, 'check', return_value={'historical': True}) as old, \
                patch.object(replay, 'check_main963', return_value={'main963': True}) as main:
            self.cli()
            old.assert_called_once_with(None)
            main.assert_called_once_with(replay.EVIDENCE / 'main963' / 'receipt.json')
        with patch.object(replay, 'check') as old, \
                patch.object(replay, 'check_main963', return_value={}) as main:
            self.cli('--main963-receipt', str(self.path), '--current-observations', 'fresh',
                     '--normalize-final-caller-positions')
            old.assert_not_called()
            main.assert_called_once_with(self.path, Path('fresh'), True)

    def test_generic_cli_rejects_historical_extensions_and_selects_only_generic_owner(self):
        with patch.object(replay, 'check') as old, patch.object(replay, 'check_main963') as main, \
                patch.object(replay, 'check_hash_composition', return_value={}) as generic:
            self.cli('--hash-composition-receipt', str(self.path))
            old.assert_not_called()
            main.assert_not_called()
            generic.assert_called_once_with(self.path)
        for arguments in (('--current-observations', 'fresh'),
                          ('--normalize-final-caller-positions',),
                          ('--main963-receipt', str(self.path))):
            with self.subTest(arguments=arguments), self.assertRaises(SystemExit):
                self.cli('--hash-composition-receipt', str(self.path), *arguments)

    def test_generic_cli_accepts_relative_receipt_path(self):
        with contextlib.chdir(self.root):
            self.cli('--hash-composition-receipt', 'receipt.json')

class GunnerMigrationReplayTests(unittest.TestCase):
    def setUp(self):
        actor = {'stable_id': 1, 'health': 125, 'last_fire_frame': -100,
                 'rearm_timer': {'start_frame': 0, 'duration': 0},
                 'current_weapon_index': 0, 'weapon_override': None}
        seed = {'next_frame': 0, 'tick_result': None, 'commands': [],
                'entities': [actor], 'logic_order': [1],
                'fire_events_accumulated': [], 'lifecycle_outputs_accumulated': [],
                'rng': {'scenario': [11, 22], 'main': [33, 44], 'mapgen': [55, 66]},
                'draws': [{'value': 7, 'callers': 'owner\n at owner.rs:10:2'}],
                'retained_path_inputs': {'house': 9}}
        tick = deepcopy(seed)
        tick.update(next_frame=1, tick_result={'state_hash': 70, 'tick': 1, 'committed': True})
        tick['entities'][0].update(last_fire_frame=0,
                                  rearm_timer={'start_frame': 0, 'duration': 52})
        tick['fire_events_accumulated'].append({'attacker': 1, 'weapon': '105mm'})
        later = deepcopy(tick)
        later.update(next_frame=2, tick_result={'state_hash': 71, 'tick': 2, 'committed': True})
        # A non-FireAt timer writer does not replace the saved charge duration.
        later['entities'][0]['rearm_timer'] = {'start_frame': 1, 'duration': 0}
        self.baseline = [seed, tick, later]
        self.current = deepcopy(self.baseline)
        for index, row in enumerate(self.current):
            row['entities'][0].pop('current_weapon_index')
            row['entities'][0].pop('weapon_override')
            row['entities'][0].update(current_weapon_number=0, current_turret_index=-1,
                                      charge_turret_delay=0 if index == 0 else 52)
            row['draws'][0]['callers'] = 'owner\n at owner.rs:20:4'
            if index:
                row['tick_result']['state_hash'] += 10

    def check(self):
        return replay.compare_gunner_migration(self.baseline, self.current, 'probe')

    def test_explicit_fields_and_last_fired_rearm_history_are_attributed(self):
        before, after = deepcopy(self.baseline), deepcopy(self.current)
        result = self.check()
        self.assertEqual(result['actor_observations'], 3)
        self.assertEqual(result['legacy_last_shot_slot_counts'], {'0': 3})
        self.assertEqual(result['charge_turret_delay_counts'], {'0': 1, '52': 2})
        self.assertEqual(result['fire_rearm_copies'], [
            {'frame': 0, 'actor': 1, 'weapon': '105mm', 'duration': 52}])
        self.assertEqual(result['caller_positions_changed'], 3)
        self.assertEqual(result['changed_tick_hashes'], 2)
        self.assertEqual(self.baseline, before)
        self.assertEqual(self.current, after)

    def test_new_owner_fields_are_checked_before_any_projection(self):
        for field, value in [('current_weapon_number', 1), ('current_turret_index', 0),
                             ('charge_turret_delay', 51)]:
            with self.subTest(field=field):
                original = deepcopy(self.current)
                self.current[1]['entities'][0][field] = value
                with self.assertRaises(ValueError):
                    self.check()
                self.current = original
        self.current[0]['entities'][0]['charge_turret_delay'] = 52
        with self.assertRaisesRegex(ValueError, 'charge_turret_delay'):
            self.check()

    def test_legacy_slots_and_overrides_cannot_be_blindly_removed(self):
        for field, value in [('current_weapon_index', 1), ('current_weapon_index', False),
                             ('weapon_override', 0)]:
            with self.subTest(field=field, value=value):
                original = deepcopy(self.baseline)
                self.baseline[1]['entities'][0][field] = value
                with self.assertRaises(ValueError):
                    self.check()
                self.baseline = original

    def test_retained_delay_does_not_follow_non_fire_rearm_writers(self):
        self.current[2]['entities'][0]['charge_turret_delay'] = 0
        with self.assertRaisesRegex(ValueError, 'charge_turret_delay'):
            self.check()

    def test_positions_health_retained_inputs_rng_and_call_paths_are_not_masked(self):
        changes = [lambda row: row['entities'][0].update(health=124),
                   lambda row: row['retained_path_inputs'].update(house=8),
                   lambda row: row['rng']['main'].__setitem__(1, 45),
                   lambda row: row['draws'][0].update(value=8),
                   lambda row: row['draws'][0].update(callers='other\n at owner.rs:20:4'),
                   lambda row: row['draws'][0].update(callers='owner\n at other.rs:20:4')]
        for change in changes:
            with self.subTest(change=change):
                original = deepcopy(self.current)
                change(self.current[1])
                with self.assertRaises(ValueError):
                    self.check()
                self.current = original

    def test_unexpected_schema_missing_rows_and_fire_history_reject(self):
        self.current[1]['entities'][0]['new_unexplained_field'] = 0
        with self.assertRaisesRegex(ValueError, 'schema migration'):
            self.check()
        self.current[1]['entities'][0].pop('new_unexplained_field')
        self.baseline[2]['fire_events_accumulated'] = []
        with self.assertRaisesRegex(ValueError, 'FireEvent history'):
            self.check()
        self.current.pop()
        with self.assertRaisesRegex(ValueError, 'observation coverage'):
            self.check()

    def test_fire_copy_requires_the_same_frame_as_the_recorded_shot(self):
        self.baseline[1]['entities'][0]['rearm_timer']['start_frame'] = -1
        with self.assertRaisesRegex(ValueError, 'rearm start'):
            self.check()

    def test_cli_selects_only_gunner_migration_and_rejects_other_modes(self):
        with patch.object(replay, 'check') as old, \
                patch.object(replay, 'check_gunner_migration', return_value={}) as gunner, \
                patch('sys.argv', ['replay', '--check', '--gunner-migration-receipt', 'receipt.json']), \
                contextlib.redirect_stdout(io.StringIO()):
            replay.main()
            old.assert_not_called()
            gunner.assert_called_once_with(Path('receipt.json'))
        for arguments in [('--hash-composition-receipt', 'other.json'),
                          ('--main963-receipt', 'other.json'),
                          ('--current-observations', 'fresh'),
                          ('--normalize-final-caller-positions',)]:
            with self.subTest(arguments=arguments), \
                    patch('sys.argv', ['replay', '--check', '--gunner-migration-receipt',
                                       'receipt.json', *arguments]), \
                    contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                replay.main()


class GunnerMigrationFinalReceiptTests(unittest.TestCase):
    digest = staticmethod(HashCompositionReplayTests.digest)
    encode = staticmethod(HashCompositionReplayTests.encode)
    retain = HashCompositionReplayTests.retain
    save_receipt = HashCompositionReplayTests.save_receipt
    rewrite_json = HashCompositionReplayTests.rewrite_json

    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.path = self.root / 'receipt.json'
        fields = GunnerMigrationReplayTests()
        fields.setUp()
        self.receipt = {
            'schema_version': 1, 'kind': 'rust-gunner-state-migration', 'scope': 'test',
            'incoming_main': {'head': 'a' * 40, 'pins': {f: 71 for f in replay.GUNNER_MIGRATION_TESTS}},
            'runs': {}, 'inputs': [], 'comparisons': [], 'limits': [],
            'normalization': replay.GUNNER_MIGRATION_NORMALIZATION,
        }
        suite = b'test result: ok. 9494 passed; 0 failed; 227 ignored; finished in 1s\n'
        self.retain('suite.log', suite)
        self.retain('base.patch', b'diff --git a/docs/research/ghidra-workflow.md b/docs/research/ghidra-workflow.md\n')
        saved, final_data = {}, {}
        for mode, observations, exit_code in [('baseline', fields.baseline, 0),
                                               ('current', fields.current, 101),
                                               ('final', fields.current, 0)]:
            execution = {'command': ['/test', '--exact', *replay.GUNNER_MIGRATION_TESTS.values(),
                                     '--nocapture', '--test-threads=1'],
                         'cwd': '/checkout', 'exit_code': exit_code, 'seconds': 0.1,
                         'binary_sha256': self.digest(mode.encode()), 'binary_unchanged': True,
                         'source_base': ('d' if mode == 'final' else 'a') * 40, 'stage': mode}
            if mode == 'final':
                execution.update(full_suite_log_sha256=self.digest(suite), full_suite_exit_code=0)
            self.retain(f'{mode}.execution.json', self.encode(execution))
            log = '' if exit_code == 0 else ''.join(
                f"thread '{name}' (123) panicked at test.rs:1:1:\n  left: 81\n right: 71\n"
                for name in replay.GUNNER_MIGRATION_TESTS.values())
            log += ('test result: ok. 3 passed; 0 failed;' if exit_code == 0 else
                    'test result: FAILED. 0 passed; 3 failed;') + '\n'
            self.retain(f'{mode}.log', log.encode())
            self.receipt['runs'][mode] = {'execution_file': f'{mode}.execution.json',
                'log_file': f'{mode}.log', 'binary_sha256': execution['binary_sha256']}
            for fixture in replay.GUNNER_MIGRATION_TESTS:
                data = HashCompositionReplayTests.lines(observations)
                if mode == 'final':
                    data = data.replace(b'owner.rs:20:4', b'owner.rs:30:8')
                    final_data[fixture] = data
                else:
                    saved[mode, fixture] = observations
                self.retain(f'{mode}-{fixture}.jsonl', data, mode=mode, fixture=fixture)
        self.receipt['comparisons'] = [replay.compare_gunner_migration(
            fields.baseline, fields.current, f) for f in replay.GUNNER_MIGRATION_TESTS]
        self.receipt['final_validation'] = {
            'source_base': 'd' * 40, 'source_base_change_file': 'base.patch',
            'full_suite': {'log_file': 'suite.log', 'passed': 9494, 'ignored': 227},
            'observations': replay.compare_final_observations(saved, final_data, True),
            'normalization': ['draw.callers .rs line/column only'],
        }
        self.save_receipt()

    def check(self):
        with patch.object(replay, 'MAIN963_FIXTURES',
                          {f: (3, 71) for f in replay.GUNNER_MIGRATION_TESTS}):
            return replay.check_gunner_migration(self.path)

    def test_final_execution_retains_probe_fields_hashes_and_full_suite_identity(self):
        result = self.check()
        self.assertEqual(result['execution_exit_codes'], {'baseline': 0, 'current': 101, 'final': 0})
        self.assertEqual(result['final_validation']['full_suite'],
                         {'passed': 9494, 'failed': 0, 'ignored': 227})
        self.assertEqual([item['caller_positions_changed']
                          for item in result['final_validation']['observations']], [3, 3, 3])

    def test_final_comparison_never_repeats_the_field_or_hash_migration(self):
        original = (self.root / 'final-bridge.jsonl').read_bytes()
        for target, field, value in [('actor', 'current_weapon_number', 1),
                                     ('actor', 'current_turret_index', 0),
                                     ('actor', 'charge_turret_delay', 51),
                                     ('tick', 'state_hash', 82)]:
            with self.subTest(field=field):
                rows = replay.rows(original)
                (rows[1]['entities'][0] if target == 'actor' else rows[1]['tick_result'])[field] = value
                self.retain('final-bridge.jsonl', HashCompositionReplayTests.lines(rows))
                self.save_receipt()
                with self.assertRaisesRegex(ValueError, 'final ungated replay changed'):
                    self.check()

    def test_final_failure_source_scope_and_full_suite_claims_fail_closed(self):
        self.rewrite_json('final.execution.json', lambda row: row.update(exit_code=101))
        with self.assertRaisesRegex(ValueError, 'unexpected execution result'):
            self.check()
        self.rewrite_json('final.execution.json', lambda row: row.update(exit_code=0))
        self.retain('base.patch', b'diff --git a/src/sim/world.rs b/src/sim/world.rs\n')
        self.save_receipt()
        with self.assertRaisesRegex(ValueError, 'exceeds the recorded documentation update'):
            self.check()
        self.retain('base.patch', b'diff --git a/docs/research/ghidra-workflow.md b/docs/research/ghidra-workflow.md\n')
        self.receipt['final_validation']['full_suite']['passed'] = 9495
        self.save_receipt()
        with self.assertRaisesRegex(ValueError, 'full suite result differs'):
            self.check()


if __name__ == '__main__':
    unittest.main()
