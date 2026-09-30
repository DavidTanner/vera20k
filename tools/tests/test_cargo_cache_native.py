"""Opt-in compiled debug-closure test, serialized with real project builds.

VERA20K_CACHE_NATIVE_TEST=1 python -m unittest tools.tests.test_cargo_cache_native
Requires clang and dsymutil/dwarfdump (macOS) or GNU readelf (Linux).
"""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

from tools import cargo_run
from tools._cargo_cache import CachePolicy, register_locked, trim, _dependencies


@unittest.skipUnless(os.environ.get('VERA20K_CACHE_NATIVE_TEST') == '1'
                     and sys.platform in {'darwin', 'linux'},
                     'opt-in native macOS/Linux compiler/debug test')
class NativeCacheRetentionTests(unittest.TestCase):
    def test_compiled_label_and_required_debug_input_survive_real_trim(self):
        project = Path(__file__).resolve().parents[2]
        project_store, _ = cargo_run.build_store(project)
        with cargo_run.build_lock(project_store / 'cargo.lock', 3600), tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            subprocess.run(['git', 'init', '-q', str(root)], check=True)
            store, namespace = cargo_run.build_store(root)
            target = root / 'target/owned-worktrees' / namespace
            deps = target / 'debug/deps'
            deps.mkdir(parents=True)
            source = root / 'fixture.c'
            source.write_text('#include <stdio.h>\n'
                              '__attribute__((noinline)) int required_answer(void) { return 42; }\n'
                              'int main(void) { printf("%d\\n", required_answer()); return 0; }\n')
            needed, orphan = deps / 'needed.rcgu.o', deps / 'orphan.rcgu.o'
            extra_source = root / 'extra.c'
            extra_source.write_text('int other_answer(void) { return 7; }\n')
            extra = deps / 'extra.rcgu.o'
            binary = deps / 'validation-test'
            subprocess.run(['clang', '-g', '-c', str(source), '-o', str(needed)], check=True)
            subprocess.run(['clang', '-g', '-c', str(extra_source), '-o', str(extra)], check=True)
            subprocess.run(['clang', '-g', str(needed), str(extra), '-o', str(binary)], check=True)
            shutil.copy2(needed, orphan)
            label = store / 'artifacts/native-test'
            preserved = label / '0/validation-test'
            preserved.parent.mkdir(parents=True)
            shutil.copy2(binary, preserved)
            manifest = {'schema': 1, 'checkout': str(root), 'target_dir': str(target),
                        'artifacts': [{'source': str(binary), 'file': '0/validation-test',
                                       'sha256': hashlib.sha256(preserved.read_bytes()).hexdigest()}]}
            (label / 'manifest.json').write_text(json.dumps(manifest))
            with cargo_run.build_lock(store / 'cargo.lock', 0):
                register_locked(root, store, target)
            references = _dependencies(preserved)
            if sys.platform == 'darwin':
                self.assertIn(needed, references)
            before = {str(path): hashlib.sha256(path.read_bytes()).hexdigest()
                      for path in [source, preserved, binary, *references]}
            policy = CachePolicy(cache_bytes=0, incremental_bytes=0, min_free_bytes=0)
            dry = trim(root, policy, 0, dry_run=True)
            self.assertEqual(dry['state'], 'planned', dry)
            self.assertTrue(orphan.exists())
            receipt = trim(root, policy, 0)
            self.assertEqual(receipt['state'], 'trimmed', receipt)
            self.assertFalse(orphan.exists())
            self.assertGreater(receipt['removed_allocated_bytes'], 0)
            for path, digest in before.items():
                self.assertEqual(hashlib.sha256(Path(path).read_bytes()).hexdigest(), digest)
            self.assertEqual(subprocess.check_output([str(preserved)], text=True), '42\n')
            self.assertEqual(_dependencies(preserved), references)
            if sys.platform == 'darwin':
                # Build an actual dSYM AFTER trimming: proves that the preserved
                # object's debug info is still linkable, beyond simple execution.
                dsym = root / 'validation-test.dSYM'
                result = subprocess.run(['xcrun', 'dsymutil', str(preserved), '-o', str(dsym)],
                                        capture_output=True, text=True, check=True)
                self.assertEqual(result.stderr, '')
                dwarf = subprocess.check_output(['xcrun', 'dwarfdump', '--debug-info', str(dsym)], text=True)
                self.assertIn('required_answer', dwarf)
                self.assertIn('fixture.c', dwarf)
            # A historical missing object must not erase surviving protection.
            # The executable retains both references after one input disappears.
            degraded = None
            if sys.platform == 'darwin':
                extra.unlink()
                shutil.copy2(needed, orphan)
                degraded = trim(root, policy, 0)
                self.assertEqual(degraded['state'], 'trimmed', degraded)
                self.assertEqual(degraded['degraded_debug_inputs'][str(preserved)], [str(extra)])
                self.assertFalse(orphan.exists())
                self.assertEqual(_dependencies(preserved), references)
                self.assertEqual(hashlib.sha256(needed.read_bytes()).hexdigest(), before[str(needed)])
                self.assertEqual(hashlib.sha256(preserved.read_bytes()).hexdigest(), before[str(preserved)])
                self.assertEqual(subprocess.check_output([str(preserved)], text=True), '42\n')
            print(json.dumps({'native_cache_validation': {
                'platform': sys.platform, 'clang': subprocess.check_output(['clang', '--version'], text=True),
                'before_sha256': before, 'required_debug_inputs': sorted(map(str, references)),
                'dry_run': dry, 'apply': receipt, 'missing_object_apply': degraded,
                'execution_after': '42', 'debug_dependency_recheck': 'passed'}}, indent=2))


if __name__ == '__main__':
    unittest.main()
