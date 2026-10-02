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


if __name__ == '__main__':
    unittest.main()
