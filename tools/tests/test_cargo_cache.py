"""Retention safety contracts: disposable caches versus validation dependencies.

Synthetic binaries below exercise retention policy, not executable-format parity.
Dependency inspector tests are separate and do not mock the parser being tested.
"""
from contextlib import redirect_stderr, redirect_stdout
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import MagicMock, patch

from tools import _cargo_cache as cache
from tools import cargo_run


class CacheTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        subprocess.run(['git', 'init', '-q', str(self.root)], check=True)
        self.environment = patch.dict(os.environ)
        self.environment.start()
        self.addCleanup(self.environment.stop)
        os.environ.pop('CARGO_TARGET_DIR', None)
        self.store, self.namespace = cargo_run.build_store(self.root)
        self.target = self.root / 'target' / 'owned-worktrees' / self.namespace
        self.target.mkdir(parents=True)
        self.idle = patch.object(cargo_run, 'build_processes', return_value=[])
        self.idle.start()
        self.addCleanup(self.idle.stop)

    def write(self, relative, *, content=None, age=100):
        path = self.target / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content if content is not None else b'x' * 8192)
        os.utime(path, (age, age))
        return path

    def label(self, *, name='validation', profile='debug/deps'):
        directory = self.store / 'artifacts' / name
        binary = directory / '0' / 'libtest-hash'
        binary.parent.mkdir(parents=True)
        binary.write_bytes(b'\xcf\xfa\xed\xfe' + b'validation binary')
        manifest = {
            'schema': 1, 'checkout': str(self.root), 'target_dir': str(self.target),
            'artifacts': [{'source': str(self.target / profile / binary.name),
                           'file': '0/' + binary.name,
                           'sha256': hashlib.sha256(binary.read_bytes()).hexdigest()}],
        }
        manifest_path = directory / 'manifest.json'
        manifest_path.write_text(json.dumps(manifest))
        return binary, manifest_path

    def trim(self, policy=None, *, dry_run=False, dependencies=None):
        with patch.object(cache, '_dependencies', side_effect=dependencies or (lambda binary: set())):
            return cache.trim(self.root, policy or cache.CachePolicy(0, 0, 0), 0,
                              dry_run=dry_run)

    def assert_blocked(self, receipt, *paths):
        self.assertEqual(receipt['state'], 'blocked', receipt)
        self.assertTrue(receipt['errors'], receipt)
        self.assertEqual(receipt['removed_files'], [])
        self.assertEqual(receipt['removed_allocated_bytes'], 0)
        for path in paths:
            self.assertTrue(path.exists(), path)

    def test_dry_run_records_plan_but_does_not_delete_or_claim_reclamation(self):
        object_file = self.write('debug/deps/orphan.rcgu.o')
        original = object_file.read_bytes()
        result = self.trim(dry_run=True)
        self.assertEqual(result['state'], 'planned', result)
        self.assertEqual(result['selected_files'], [str(object_file)])
        self.assertGreater(result['projected_removed_allocated_bytes'], 0)
        self.assertEqual(result['removed_allocated_bytes'], 0)
        self.assertEqual(result['removed_files'], [])
        self.assertEqual(object_file.read_bytes(), original)
        receipt = json.loads(Path(result['receipt_path']).read_text())
        self.assertTrue(receipt['dry_run'])
        self.assertEqual(receipt['state'], 'planned')

    def test_labelled_libtest_preserves_required_object_and_all_other_inputs(self):
        binary, manifest = self.label()
        dependency = self.write('debug/deps/required.rcgu.o')
        orphan = self.write('debug/deps/orphan.rcgu.o')
        evidence = self.root / 'native-evidence.json'
        evidence.write_text('native observations')
        source = self.root / 'source.rs'
        source.write_text('fn main() {}')
        asset = self.root / 'retail.mix'
        asset.write_bytes(b'retail data')
        kept = {path: path.read_bytes() for path in (binary, manifest, dependency,
                                                    evidence, source, asset)}
        result = self.trim(dependencies=lambda executable: {dependency})
        self.assertEqual(result['state'], 'trimmed', result)
        self.assertEqual(result['removed_files'], [str(orphan)])
        self.assertEqual(result['removed_logical_bytes'], 8192)
        self.assertFalse(orphan.exists())
        for path, contents in kept.items():
            self.assertEqual(path.read_bytes(), contents, path)
        # Protecting lib tests must not turn them into host CLI executables.
        with self.assertRaisesRegex(ValueError, 'not a debug host bin'):
            cargo_run.resolve_labeled_binary(self.root, 'validation', 'libtest-hash', 'debug')

    def test_unlabelled_cached_executable_dependency_is_protected(self):
        binary = self.write('debug/examples/toy', content=b'\xcf\xfa\xed\xfeexample')
        dependency = self.write('debug/deps/required.rcgu.o')
        orphan = self.write('debug/deps/orphan.rcgu.o')
        seen = []
        def inspect(path):
            seen.append(path)
            return {dependency}
        result = self.trim(dependencies=inspect)
        self.assertEqual(result['state'], 'trimmed', result)
        self.assertIn(binary, seen)
        self.assertTrue(dependency.exists())
        self.assertFalse(orphan.exists())

    def test_total_budget_stops_after_oldest_sufficient_object(self):
        oldest = self.write('release/deps/old.rcgu.o', age=10)
        newest = self.write('release/deps/new.rcgu.o', age=20)
        allocated = cache._allocated(newest.stat())
        result = self.trim(cache.CachePolicy(allocated, 1 << 40, 0))
        self.assertEqual(result['removed_files'], [str(oldest)], result)
        self.assertTrue(newest.exists())
        self.assertEqual(result['unmet_targets']['cache_bytes'], 0)

    def test_free_space_target_trims_even_under_cache_budgets(self):
        orphan = self.write('debug/deps/orphan.rcgu.o')
        with patch.object(cache.shutil, 'disk_usage', return_value=SimpleNamespace(free=100)):
            result = self.trim(cache.CachePolicy(1 << 40, 1 << 40, 200))
        self.assertEqual(result['removed_files'], [str(orphan)], result)
        self.assertEqual(result['free_before'], result['free_after'])
        # Allocated bytes unlinked and the actual volume free-space observation
        # are separate facts; a clone/snapshot may prevent observed recovery.
        self.assertGreater(result['removed_allocated_bytes'], 0)
        self.assertEqual(set(result['unmet_targets']['free_bytes'].values()), {100})

    def test_actual_free_space_shortfall_uses_more_cold_objects_than_dry_plan(self):
        first = self.write('debug/deps/first.rcgu.o', age=10)
        second = self.write('debug/deps/second.rcgu.o', age=20)
        policy = cache.CachePolicy(1 << 40, 1 << 40, 200)
        with patch.object(cache.shutil, 'disk_usage', return_value=SimpleNamespace(free=100)):
            plan = self.trim(policy, dry_run=True)
            result = self.trim(policy)
        self.assertEqual(plan['selected_files'], [str(first)], plan)
        self.assertEqual(result['removed_files'], [str(first), str(second)], result)
        self.assertEqual(set(result['unmet_targets']['free_bytes'].values()), {100})

    def test_incremental_budget_deletes_whole_cold_session_and_keeps_hottest(self):
        cold = [self.write('debug/incremental/crate-123/s-old/object.o', age=10),
                self.write('debug/incremental/crate-123/s-old/dep-graph.bin', age=10)]
        hot = self.write('debug/incremental/crate-123/s-hot/object.o', age=20)
        outside = self.write('debug/deps/orphan.rcgu.o', age=1)
        result = self.trim(cache.CachePolicy(1 << 40, 0, 0))
        self.assertEqual(set(result['removed_files']), {str(path) for path in cold}, result)
        self.assertFalse(cold[0].parent.exists())
        self.assertTrue(hot.exists())
        self.assertTrue(outside.exists())
        self.assertGreater(result['unmet_targets']['incremental_bytes'], 0)

    def test_hardlinks_symlinks_unknown_source_and_debug_files_are_preserved(self):
        hard = self.write('debug/deps/shared.rcgu.o')
        alias = self.root / 'hardlink-backup'
        os.link(hard, alias)
        external = self.root / 'external-object'
        external.write_bytes(b'outside cache')
        linked = self.target / 'debug/deps/linked.rcgu.o'
        linked.symlink_to(external)
        cold = self.write('debug/incremental/crate-123/s-old/objects.o', age=1)
        source = self.write('debug/incremental/crate-123/s-old/source.rs', age=1)
        hot = self.write('debug/incremental/crate-123/s-hot/objects.o', age=20)
        sidecars = [self.write('debug/deps/' + name) for name in
                    ('symbols.pdb', 'symbols.dwo', 'symbols.dwp', 'archive.rlib', 'unknown.o')]
        outside_profile = self.write('research/deps/native.rcgu.o')
        result = self.trim()
        self.assertEqual(result['removed_files'], [], result)
        self.assertTrue(linked.is_symlink())
        for path in [hard, alias, external, cold, source, hot, outside_profile, *sidecars]:
            self.assertTrue(path.exists(), path)

    def test_link_inside_cold_session_prevents_partial_session_deletion(self):
        cold = self.write('debug/incremental/crate-123/s-old/object.o', age=1)
        hot = self.write('debug/incremental/crate-123/s-hot/object.o', age=20)
        link = cold.parent / 'unknown.bin'
        link.symlink_to(self.root / 'missing-input')
        result = self.trim(cache.CachePolicy(1 << 40, 0, 0))
        self.assertEqual(result['removed_files'], [], result)
        self.assertTrue(cold.exists())
        self.assertTrue(hot.exists())
        self.assertTrue(link.is_symlink())

    def test_directory_link_inside_cold_session_prevents_partial_session_deletion(self):
        cold = self.write('debug/incremental/crate-123/s-old/object.o', age=1)
        self.write('debug/incremental/crate-123/s-hot/object.o', age=20)
        external = self.root / 'source-directory'
        external.mkdir()
        source = external / 'evidence.json'
        source.write_text('native evidence')
        link = cold.parent / 'source'
        link.symlink_to(external, target_is_directory=True)
        result = self.trim(cache.CachePolicy(1 << 40, 0, 0))
        self.assertEqual(result['removed_files'], [], result)
        self.assertTrue(cold.exists())
        self.assertTrue(link.is_symlink())
        self.assertEqual(source.read_text(), 'native evidence')

    def test_unknown_native_evidence_binary_protects_whole_incremental_session(self):
        cold = self.write('debug/incremental/crate-123/s-old/object.o', age=1)
        evidence = self.write('debug/incremental/crate-123/s-old/native-evidence.bin',
                              content=b'captured native instructions', age=1)
        hot = self.write('debug/incremental/crate-123/s-hot/object.o', age=20)
        result = self.trim(cache.CachePolicy(1 << 40, 0, 0))
        self.assertEqual(result['removed_files'], [], result)
        self.assertTrue(cold.exists())
        self.assertTrue(hot.exists())
        self.assertEqual(evidence.read_bytes(), b'captured native instructions')

    def test_symlinked_owned_root_never_adopts_external_cache_for_deletion(self):
        self.target.rmdir()
        external = self.root / 'external-cache'
        external.mkdir()
        self.target.symlink_to(external, target_is_directory=True)
        orphan = self.write('debug/deps/orphan.rcgu.o')
        result = self.trim()
        self.assert_blocked(result, orphan)

    def test_missing_malformed_duplicate_or_corrupted_label_blocks_all_deletion(self):
        binary, manifest = self.label()
        orphan = self.write('debug/deps/orphan.rcgu.o')
        original_manifest = manifest.read_text()
        original_binary = binary.read_bytes()
        for failure in ('missing_manifest', 'malformed_manifest', 'duplicate_key',
                        'missing_binary', 'wrong_binary_hash'):
            with self.subTest(failure=failure):
                manifest.write_text(original_manifest)
                binary.write_bytes(original_binary)
                if failure == 'missing_manifest':
                    manifest.unlink()
                elif failure == 'malformed_manifest':
                    manifest.write_text('{broken')
                elif failure == 'duplicate_key':
                    manifest.write_text('{"schema": 1, "schema": 1}')
                elif failure == 'missing_binary':
                    binary.unlink()
                else:
                    binary.write_bytes(b'changed')
                self.assert_blocked(self.trim(), orphan)

    def test_dependency_inspection_failure_or_missing_dependency_blocks_every_unlink(self):
        self.label()
        orphan = self.write('debug/deps/orphan.rcgu.o')
        for inspector in (lambda path: (_ for _ in ()).throw(ValueError('unknown format')),
                          lambda path: {self.root / 'missing.rcgu.o'}):
            with self.subTest(inspector=inspector):
                self.assert_blocked(self.trim(dependencies=inspector), orphan)

    def test_stale_latest_record_protects_replacement_executable_dependencies(self):
        original = self.write('release/game', content=b'\xcf\xfa\xed\xfeold')
        cargo_run.publish_binaries(self.store, self.namespace, self.target, {original})
        original.write_bytes(b'\xcf\xfa\xed\xfechanged by failed compile')
        required = self.write('release/deps/replacement.rcgu.o')
        orphan = self.write('release/deps/orphan.rcgu.o')
        latest = self.store / 'latest' / f'{self.namespace}.json'
        recorded = latest.read_bytes()
        seen = []
        def inspect(binary):
            seen.append(binary)
            return {required}
        result = self.trim(dependencies=inspect)
        self.assertEqual(result['removed_files'], [str(orphan)], result)
        self.assertIn(original, seen)
        self.assertTrue(required.exists())
        self.assertEqual(latest.read_bytes(), recorded)
        self.assertTrue(result['notes'])
        self.assertEqual(cargo_run.resolve_binary(self.root, 'game'), (None, None))

    def test_candidate_mutation_during_dependency_inspection_blocks_deletion(self):
        self.label()
        orphan = self.write('debug/deps/orphan.rcgu.o')
        def inspect(binary):
            orphan.write_bytes(b'changed after inventory')
            return set()
        self.assert_blocked(self.trim(dependencies=inspect), orphan)

    def test_new_validation_publication_during_inspection_blocks_deletion(self):
        self.label()
        orphan = self.write('debug/deps/orphan.rcgu.o')
        def inspect(binary):
            self.label(name='new-validation')
            return set()
        self.assert_blocked(self.trim(dependencies=inspect), orphan)

    def test_unlabelled_binary_published_at_end_of_inventory_blocks_deletion(self):
        required = self.write('debug/deps/new-test-dependency.rcgu.o')
        original_files = cache._files
        published = []
        def inventory_then_publish(root, *args, **kwargs):
            # Deterministic inventory/snapshot gap: the new binary is created
            # after the walker emits its last entry, before inventory completes.
            yield from original_files(root, *args, **kwargs)
            published.append(self.write('debug/deps/new-libtest',
                                        content=b'\xcf\xfa\xed\xfefresh libtest'))
        def inspect(binary):
            return {required} if binary in published else set()
        with patch.object(cache, '_files', side_effect=inventory_then_publish):
            result = self.trim(dependencies=inspect)
        self.assert_blocked(result, required, *published)

    def test_nested_native_evidence_paths_are_not_compiler_cache_candidates(self):
        base = 'debug/build/native-oracle/out/debug'
        evidence = self.write(base + '/deps/native-evidence.rcgu.o')
        cold = self.write(base + '/incremental/capture-123/s-old/object.o', age=1)
        metadata = self.write(base + '/incremental/capture-123/s-old/dep-graph.bin', age=1)
        hot = self.write(base + '/incremental/capture-123/s-hot/object.o', age=20)
        disposable = self.write('debug/deps/orphan.rcgu.o')
        kept = {path: path.read_bytes() for path in (evidence, cold, metadata, hot)}
        result = self.trim()
        self.assertEqual(result['removed_files'], [str(disposable)], result)
        for path, contents in kept.items():
            self.assertEqual(path.read_bytes(), contents, path)

    def test_cross_target_cache_requires_recorded_cargo_executable_profile(self):
        profile = 'aarch64-unknown-linux-gnu/debug'
        cross_object = self.write(profile + '/deps/orphan.rcgu.o')
        cold = self.write(profile + '/incremental/crate-123/s-old/object.o', age=1)
        hot = self.write(profile + '/incremental/crate-123/s-hot/object.o', age=20)
        binary = self.write(profile + '/toy', content=b'\xcf\xfa\xed\xfefixture')
        host_object = self.write('debug/deps/orphan.rcgu.o')
        guessed = self.trim()
        self.assertEqual(guessed['removed_files'], [str(host_object)], guessed)
        self.assertTrue(cross_object.exists())
        self.assertTrue(cold.exists())
        # This is the owner's post-Cargo registration API: an exact emitted
        # executable proves the cross profile; a directory name alone does not.
        with cargo_run.build_lock(self.store / 'cargo.lock', 0):
            cache.register_locked(self.root, self.store, self.target, artifacts={binary})
        recorded = self.trim()
        self.assertEqual(set(recorded['removed_files']), {str(cross_object), str(cold)}, recorded)
        self.assertTrue(hot.exists())
        self.assertTrue(binary.exists())

    def test_preserved_label_original_source_proves_cross_target_profile(self):
        profile = 'x86_64-unknown-linux-gnu/release'
        self.label(profile=profile + '/deps')
        orphan = self.write(profile + '/deps/orphan.rcgu.o')
        result = self.trim()
        self.assertEqual(result['removed_files'], [str(orphan)], result)

    def test_inspector_mutation_of_previously_verified_binary_blocks_deletion(self):
        first, _ = self.label(name='first')
        self.label(name='second')
        orphan = self.write('debug/deps/orphan.rcgu.o')
        def inspect(binary):
            if binary != first:
                first.write_bytes(b'changed after verification')
            return set()
        self.assert_blocked(self.trim(dependencies=inspect), orphan)

    def test_active_unwrapped_build_blocks_deletion(self):
        orphan = self.write('debug/deps/orphan.rcgu.o')
        # Call the already-locked owner to model a compiler appearing after
        # lock acquisition; the public lock gate is covered by cargo_run tests.
        with patch.object(cargo_run, 'build_processes', return_value=['123 rustc']):
            result = cache.trim_locked(self.root, self.store, cache.CachePolicy(0, 0, 0))
        self.assert_blocked(result, orphan)

    def test_partial_unlink_failure_stops_and_records_actual_reclamation(self):
        first = self.write('debug/deps/first.rcgu.o', age=10)
        second = self.write('debug/deps/second.rcgu.o', age=20)
        third = self.write('debug/deps/third.rcgu.o', age=30)
        unlink = Path.unlink
        def fail_one(path, *args, **kwargs):
            if path == second:
                raise OSError('simulated disk failure')
            return unlink(path, *args, **kwargs)
        with patch.object(Path, 'unlink', new=fail_one):
            result = self.trim()
        self.assertEqual(result['state'], 'partial', result)
        self.assertEqual(result['removed_files'], [str(first)])
        self.assertEqual(result['removed_logical_bytes'], 8192)
        self.assertTrue(result['errors'])
        self.assertFalse(first.exists())
        self.assertTrue(second.exists())
        self.assertTrue(third.exists())
        saved = json.loads(Path(result['receipt_path']).read_text())
        self.assertEqual(saved['removed_files'], [str(first)])
        self.assertEqual(saved['state'], 'partial')

    def test_final_free_space_failure_saves_partial_receipt_without_escaping(self):
        orphan = self.write('debug/deps/orphan.rcgu.o')
        def sample(root):
            if not orphan.exists():
                raise OSError('final free-space observation unavailable')
            return SimpleNamespace(free=100)
        with patch.object(cache.shutil, 'disk_usage', side_effect=sample):
            result = self.trim()
        self.assertEqual(result['state'], 'partial', result)
        self.assertEqual(result['removed_files'], [str(orphan)])
        self.assertEqual(result['removed_logical_bytes'], 8192)
        self.assertTrue(result['errors'])
        self.assertTrue(any('free-space' in error for error in result['errors']))
        saved = json.loads(Path(result['receipt_path']).read_text())
        self.assertEqual(saved['state'], 'partial')
        self.assertEqual(saved['removed_files'], [str(orphan)])

    def test_initial_free_space_failure_saves_blocked_receipt(self):
        orphan = self.write('debug/deps/orphan.rcgu.o')
        with patch.object(cache.shutil, 'disk_usage', side_effect=OSError('volume unavailable')):
            result = self.trim()
        self.assert_blocked(result, orphan)
        saved = json.loads(Path(result['receipt_path']).read_text())
        self.assertEqual(saved['state'], 'blocked')
        self.assertTrue(saved['errors'])

    def test_later_dependency_change_stops_remaining_deletions(self):
        self.label()
        dependency = self.write('debug/deps/required.rcgu.o')
        first = self.write('debug/deps/first.rcgu.o', age=10)
        second = self.write('debug/deps/second.rcgu.o', age=20)
        original_unlink = Path.unlink
        def mutate_after_first(path, *args, **kwargs):
            result = original_unlink(path, *args, **kwargs)
            if path == first:
                dependency.write_bytes(b'changed during deletion')
            return result
        # Advance the periodic guard's clock at every read to exercise the
        # promised repeated checks, without making this a wall-clock wait test.
        with patch.object(Path, 'unlink', new=mutate_after_first), \
             patch.object(cache.time, 'monotonic', side_effect=iter(range(100))):
            result = self.trim(dependencies=lambda executable: {dependency})
        self.assertEqual(result['state'], 'partial', result)
        self.assertEqual(result['removed_files'], [str(first)])
        self.assertTrue(second.exists())
        self.assertTrue(result['errors'])

    def test_registry_tracks_custom_failed_and_unlabelled_build_without_publishing(self):
        custom = self.root / 'custom-target'
        os.environ['CARGO_TARGET_DIR'] = str(custom)
        real_popen = subprocess.Popen
        for status in (7, 0):
            child = MagicMock()
            child.__enter__.return_value = child
            child.stdout = io.StringIO('')
            child.wait.return_value = status
            def start(command, **kwargs):
                return child if command[0] == 'cargo' else real_popen(command, **kwargs)
            with patch.object(cargo_run, 'source_identity', return_value={'same': True}), \
                 patch.object(cargo_run.subprocess, 'Popen', side_effect=start), \
                 patch.object(cache, 'automatic_locked') as automatic, \
                 redirect_stderr(io.StringIO()), redirect_stdout(io.StringIO()):
                result = cargo_run.run(self.root, ['check'], None, 0,
                                       policy=cache.CachePolicy(1 << 40, 1 << 40, 0))
            self.assertEqual(result, status)
            registry = json.loads((self.store / 'cache-roots.json').read_text())
            owned = custom / 'owned-worktrees' / self.namespace
            self.assertEqual(registry['roots'][str(owned)]['checkout'], str(self.root))
            self.assertEqual(automatic.call_count, 2)
            self.assertFalse((self.store / 'artifacts').exists())

    def test_retention_free_space_failure_does_not_abort_ordinary_cargo(self):
        real_popen = subprocess.Popen
        launched = []
        child = MagicMock()
        child.__enter__.return_value = child
        child.stdout = io.StringIO('')
        child.wait.return_value = 0
        def start(command, **kwargs):
            if command[0] == 'cargo':
                launched.append(command)
                return child
            return real_popen(command, **kwargs)
        with patch.object(cache.shutil, 'disk_usage', side_effect=OSError('volume unavailable')), \
             patch.object(cargo_run, 'source_identity', return_value={'same': True}), \
             patch.object(cargo_run.subprocess, 'Popen', side_effect=start), \
             redirect_stderr(io.StringIO()), redirect_stdout(io.StringIO()):
            result = cargo_run.run(self.root, ['check'], None, 0,
                                   policy=cache.CachePolicy(0, 0, 0))
        self.assertEqual(result, 0)
        self.assertEqual(len(launched), 1)
        receipts = [json.loads(path.read_text()) for path in
                    (self.store / 'retention').glob('*.json')]
        self.assertEqual(len(receipts), 2)
        self.assertTrue(all(receipt['state'] == 'blocked' for receipt in receipts))
        self.assertTrue(all(receipt['removed_files'] == [] for receipt in receipts))

    def test_under_budget_never_invokes_expensive_dependency_inspection(self):
        self.label()
        orphan = self.write('debug/deps/orphan.rcgu.o')
        with patch.object(cache, '_dependencies', side_effect=AssertionError('unnecessary inspection')):
            result = cache.trim(self.root, cache.CachePolicy(1 << 40, 1 << 40, 0), 0)
        self.assertEqual(result['state'], 'under_budget', result)
        self.assertTrue(orphan.exists())

    def test_standalone_cli_dry_run_reports_plan_and_never_dispatches_cargo(self):
        orphan = self.write('debug/deps/orphan.rcgu.o')
        stdout, stderr = io.StringIO(), io.StringIO()
        with patch.object(Path, 'cwd', return_value=self.root), \
             patch.object(cargo_run, 'run') as build, \
             redirect_stdout(stdout), redirect_stderr(stderr):
            result = cargo_run.main(['--trim-cache', '--dry-run', '--cache-gib', '0',
                                     '--incremental-gib', '0', '--min-free-gib', '0',
                                     '--wait-seconds', '0'])
        self.assertEqual(result, 0, stderr.getvalue())
        build.assert_not_called()
        receipt = json.loads(stdout.getvalue())
        self.assertEqual(receipt['state'], 'planned')
        self.assertEqual(receipt['selected_files'], [str(orphan)])
        self.assertTrue(orphan.exists())

    def test_cli_rejects_ambiguous_retention_and_build_or_resolve_requests(self):
        for arguments in (
            ['--dry-run', '--', 'build'],
            ['--trim-cache', '--label', 'validation'],
            ['--trim-cache', '--', 'build'],
            ['--resolve', 'game', '--profile', 'release', '--cache-gib', '0'],
        ):
            with self.subTest(arguments=arguments), \
                 patch.object(cargo_run, 'run') as build, \
                 redirect_stderr(io.StringIO()), self.assertRaises(SystemExit) as failure:
                cargo_run.main(arguments)
            self.assertEqual(failure.exception.code, 2)
            build.assert_not_called()


class DebugDependencyTests(unittest.TestCase):
    def parse(self, text):
        return cache._debug_map(io.StringIO(text))

    def test_actual_llvm_yaml_subset_includes_quoted_objects_and_archive_container(self):
        text = """---
triple:          'arm64-apple-darwin'
binary-path:     '/tmp/game'
objects:
  - filename:        '/tmp/objects/has space.rcgu.o'
    timestamp:       1730301197
    symbols:
      - { sym: '_main', objAddr: 0x0, binAddr: 0x100003FB0, size: 0x20 }
  - filename:        '/tmp/libs/has (parenthesis).rlib(member.rcgu.o)'
    timestamp:       1730301198
    symbols:         []
...
"""
        self.assertEqual(self.parse(text), {Path('/tmp/objects/has space.rcgu.o'),
                                           Path('/tmp/libs/has (parenthesis).rlib')})

    def test_multiple_architectures_and_empty_object_document(self):
        text = "---\ntriple: 'arm64-apple-darwin'\nbinary-path: '/tmp/game'\nobjects: []\n...\n"
        self.assertEqual(self.parse(text + text.replace('arm64', 'x86_64')), set())

    def test_llvm_empty_debug_map_may_omit_objects_key(self):
        # Actual dsymutil --dump-debug-map output on a binary without linker
        # object references omits objects rather than serializing an empty list.
        self.assertEqual(self.parse(
            "---\ntriple: 'arm64-apple-darwin'\nbinary-path: '/tmp/game'\n...\n"), set())

    def test_unrecognized_relative_truncated_or_malformed_map_is_not_accepted(self):
        good = "---\ntriple: 'arm64-apple-darwin'\nbinary-path: '/tmp/game'\nobjects: []\n...\n"
        bad_maps = [
            '', good[:-4], 'warning: missing object\n' + good,
            good.replace("binary-path: '/tmp/game'", "binary-path: '/tmp/game'\ntriple: duplicate"),
            good.replace('objects: []', 'unknown: []'),
            good.replace('objects: []', "objects:\n  - filename: 'relative.rcgu.o'\n    symbols: []"),
            good.replace('objects: []', "objects:\n  - filename: '/tmp/../objects/x.o'\n    symbols: []"),
            good.replace('objects: []', "objects:\n  - filename: '/tmp/x.o'"),
            good.replace('objects: []', "objects:\n  - filename: '/tmp/lib.rlib()'\n    symbols: []"),
            good.replace('objects: []', "objects:\n  - filename: '/tmp/x.o'\n    symbols:\n      - { sym: '_main' }"),
        ]
        for text in bad_maps:
            with self.subTest(text=text), self.assertRaises(ValueError):
                self.parse(text)

    def test_inspector_zero_status_with_warning_still_fails_closed(self):
        good = "---\ntriple: 'arm64-apple-darwin'\nbinary-path: '/tmp/game'\nobjects: []\n...\n"
        for status, diagnostic in ((0, 'warning: missing object\n'), (1, ''), (0, '')):
            def start(command, **kwargs):
                kwargs['stderr'].write(diagnostic)
                child = MagicMock()
                child.__enter__.return_value = child
                child.stdout = io.StringIO(good)
                child.wait.return_value = status
                return child
            with self.subTest(status=status, diagnostic=diagnostic), \
                 patch.object(cache.subprocess, 'Popen', side_effect=start):
                if status or diagnostic:
                    with self.assertRaisesRegex(ValueError, 'inspection failed'):
                        cache._stream(['inspector'], cache._debug_map)
                else:
                    self.assertEqual(cache._stream(['inspector'], cache._debug_map), set())

    def test_non_native_or_pe_format_blocks_cleanup_instead_of_guessing_dependencies(self):
        with tempfile.TemporaryDirectory() as directory:
            binary = Path(directory) / 'program'
            for content in (b'text fixture', b'MZ\x90\x00unknown PE'):
                binary.write_bytes(content)
                with self.subTest(content=content), self.assertRaisesRegex(ValueError, 'Unsupported'):
                    cache._dependencies(binary)


class PolicyTests(unittest.TestCase):
    def test_decimal_environment_and_cli_overrides_have_exact_integer_byte_values(self):
        with patch.dict(os.environ, {'VERA20K_CACHE_GIB': '1.5',
                                     'VERA20K_INCREMENTAL_GIB': '0',
                                     'VERA20K_MIN_FREE_GIB': '2'}):
            policy = cache.CachePolicy.from_env()
            self.assertEqual(policy, cache.CachePolicy(3 * cache.GIB // 2, 0, 2 * cache.GIB))
            self.assertEqual(cache.CachePolicy.from_env('0.25', free='0'),
                             cache.CachePolicy(cache.GIB // 4, 0, 0))

    def test_invalid_nonfinite_negative_or_fractional_byte_limits_are_rejected(self):
        for value in ('nan', 'Infinity', '-1', 'not-a-size'):
            with self.subTest(value=value), self.assertRaises(ValueError):
                cache.gib_bytes(value)
        for value in (-1, True, 1.5):
            with self.subTest(value=value), self.assertRaises(ValueError):
                cache.CachePolicy(value, 0, 0)


if __name__ == '__main__':
    unittest.main()
