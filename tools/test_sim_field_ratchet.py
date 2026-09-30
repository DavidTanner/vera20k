"""Checks for the src/sim crate-visible field ratchet.

python -m unittest tools.test_sim_field_ratchet -v
"""

from contextlib import redirect_stderr, redirect_stdout
import io
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch

from tools import sim_field_ratchet as ratchet


def wide(files):
    """Counted fields, with src/sim/mod.rs declaring `x` unless the files say otherwise."""
    files = {'src/sim/mod.rs': 'pub mod x;\n', **files}
    return sorted((f.path, f.struct, f.name) for f in ratchet.crate_visible_fields(files))


class BlankTests(unittest.TestCase):
    def test_comments_and_literals_are_blanked_in_place(self):
        source = ('a // pub x: u8\n/* pub /* nested */ y */ "pub z: \\" u8" r#"pub"q"# cr#"pub{"# '
                  "b'p' '\\n' '\\'' <'a> c")
        clean = ratchet.blank(source)
        self.assertEqual(len(clean), len(source))
        self.assertEqual(clean.index('\n'), source.index('\n'))
        self.assertNotIn('pub', clean)
        self.assertNotIn('{', clean)
        self.assertIn("<'a> c", clean)


class FieldTests(unittest.TestCase):
    def test_counts_fields_that_reach_all_of_sim(self):
        source = '''
pub struct A { pub a: u8, pub(crate) b: u16, pub(super) c: u8, d: u8, pub(in crate::sim) e: u8, pub r#type: u8 }
pub(crate) struct B(pub u8, pub (u8, u16), u32);
struct Private { pub hidden: u8 }
pub struct G<T: Fn(u8) -> u8, const N: usize> where T: Copy {
    #[serde(default)]
    pub list: Vec<Option<(u8, u16)>>,
    pub map: std::collections::HashMap<u8, [u8; N]>,
    pub shifted: [u8; 1 << 4], pub after_shift: u8,
    f: T,
}
pub struct Unit;
'''
        # In crate::sim::x, pub(super) reaches all of crate::sim; a private struct stays in x.
        self.assertEqual([(s, n) for _, s, n in wide({'src/sim/x.rs': source})], [
            ('A', 'a'), ('A', 'b'), ('A', 'c'), ('A', 'e'), ('A', 'r#type'),
            ('B', '0'), ('B', '1'), ('G', 'after_shift'), ('G', 'list'), ('G', 'map'), ('G', 'shifted')])

    def test_visibility_narrower_than_sim_is_not_counted(self):
        files = {
            'src/sim/x.rs': 'pub mod inner;\n',
            'src/sim/x/inner.rs': ('pub struct N { pub(super) a: u8, pub(in crate::sim::x) b: u8, pub c: u8 }\n'
                                   'pub(super) struct S { pub d: u8 }\nstruct P { pub e: u8 }\n'),
        }
        self.assertEqual([(s, n) for _, s, n in wide(files)], [('N', 'c')])

    def test_test_gated_items_and_fields_are_skipped(self):
        source = '''
pub struct Live { pub a: u8, #[cfg(test)] pub probe: u8 }
#[cfg(test)]
#[derive(Debug)]
pub struct Helper<A, B> { pub b: A, pub c: B }
#[cfg(test)]
fn build<A, B>() { pub struct Local { pub d: u8 } }
#[cfg(test)]
const LIMIT: u32 = 1 << 3;
#[cfg(test)]
mod tests {
    pub struct Inner { pub e: u8 }
}
pub struct After { pub f: u8 }
'''
        self.assertEqual([(s, n) for _, s, n in wide({'src/sim/x.rs': source})], [('After', 'f'), ('Live', 'a')])

    def test_module_tree_decides_what_is_test_code(self):
        files = {
            'src/sim/x.rs': ('#[path = "x_probe.rs"]\n#[cfg(test)]\nmod probe;\n'
                             '#[cfg(test)]\nmod harness;\nmod live;\npub struct A { pub a: u8 }\n'
                             'include!("x_shared.rs");\n#[cfg(test)]\nmod checks { include!("x_checks.rs"); }\n'),
            'src/sim/x_probe.rs': 'pub struct P { pub b: u8 }\n',
            'src/sim/x/harness.rs': 'mod deep;\npub struct H { pub c: u8 }\n',
            'src/sim/x/harness/deep.rs': 'pub struct D { pub d: u8 }\n',
            'src/sim/x/live.rs': 'pub struct L { pub e: u8 }\n',
            'src/sim/x_shared.rs': 'pub struct S { pub f: u8 }\n',
            'src/sim/x_checks.rs': 'pub struct C { pub g: u8 }\n',
            'src/sim/orphan.rs': 'pub struct O { pub h: u8 }\n',
        }
        self.assertEqual(wide(files), [('src/sim/x.rs', 'A', 'a'), ('src/sim/x/live.rs', 'L', 'e'),
                                       ('src/sim/x_shared.rs', 'S', 'f')])

    def test_path_files_resolve_children_like_mod_rs(self):
        files = {
            'src/sim/x.rs': '#[path = "other/y.rs"]\npub mod y;\n',
            'src/sim/other/y.rs': 'pub mod z;\n',
            'src/sim/other/z.rs': 'pub struct Z { pub a: u8, pub(super) b: u8 }\n',
        }
        # z is crate::sim::x::y::z, so its pub(super) stays inside crate::sim::x::y.
        self.assertEqual([(s, n) for _, s, n in wide(files)], [('Z', 'a')])

    def test_lines_point_at_the_field(self):
        fields = ratchet.crate_visible_fields(
            {'src/sim/mod.rs': '// c\npub struct A {\n    /// doc\n    pub a: u8,\n}\n'})
        self.assertEqual([(f.line, f.name, f.visibility) for f in fields], [(4, 'a', 'pub')])


class CompareTests(unittest.TestCase):
    def test_new_fields_ignore_a_moved_struct(self):
        base = ratchet.crate_visible_fields({'src/sim/mod.rs': 'pub struct S { pub x: u8 }'})
        moved = ratchet.crate_visible_fields({'src/sim/mod.rs': 'mod b;', 'src/sim/b.rs': 'pub struct S { pub x: u8 }'})
        grown = ratchet.crate_visible_fields(
            {'src/sim/mod.rs': 'mod b;', 'src/sim/b.rs': 'pub struct S { pub x: u8, pub y: u8 }'})
        self.assertEqual(ratchet.new_fields(base, moved), [])
        self.assertEqual([(f.struct, f.name) for f in ratchet.new_fields(base, grown)], [('S', 'y')])


@unittest.skipUnless(shutil.which('git'), 'git is required')
class RevisionTests(unittest.TestCase):
    def test_branch_is_measured_from_its_merge_base(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            sim = root / 'src' / 'sim'
            sim.mkdir(parents=True)

            def run(*args):
                return subprocess.run(['git', '-c', 'user.name=t', '-c', 'user.email=t@example.com', *args],
                                      cwd=root, check=True, capture_output=True, text=True).stdout.strip()

            def commit(**sources):
                for name, text in sources.items():
                    (sim / name).write_text(text)
                run('add', '.')
                run('commit', '-q', '-m', 'step')

            run('init', '-q')
            commit(**{'mod.rs': 'pub mod a;\npub mod b;\n', 'a.rs': 'pub struct S { pub x: u8, pub y: u8 }\n',
                      'b.rs': 'pub struct T { pub z: u8 }\n'})
            fork = run('rev-parse', 'HEAD')
            commit(**{'a.rs': 'pub struct S { x: u8, y: u8 }\n'})
            main = run('rev-parse', 'HEAD')
            run('checkout', '-q', fork)
            commit(**{'b.rs': 'pub struct T { pub z: u8, w: u8 }\n'})
            private_branch = run('rev-parse', 'HEAD')
            commit(**{'b.rs': 'pub struct T { pub z: u8, w: u8, pub v: u8 }\n'})
            output, errors = io.StringIO(), io.StringIO()
            with patch.object(ratchet, 'ROOT', root), redirect_stdout(output), redirect_stderr(errors):
                # Behind main, which dropped two fields: measured from the fork, not main's tip.
                self.assertEqual(ratchet.main(['--base', main, '--head', private_branch]), 0)
                self.assertEqual(ratchet.main(['--base', main, '--head', 'HEAD']), 1)
            self.assertIn('src/sim/b.rs:1 T.v (pub)', errors.getvalue())
            self.assertIn('up from 3', errors.getvalue())


if __name__ == '__main__':
    unittest.main()
