"""Fail a change that raises the number of crate-visible struct fields under src/sim.

Simulation state has one owner that keeps it private (AGENTS.md/CLAUDE.md). A
struct field that code anywhere in `crate::sim` (or the whole crate) can reach,
through `pub`, `pub(crate)`, `pub(in crate::sim)` or a `pub(super)` that lands on
`crate::sim`, is counted when its struct is at least as visible. Private fields
and visibilities narrower than `crate::sim` are not. The count may only go down.

    python tools/sim_field_ratchet.py                      # count only
    python tools/sim_field_ratchet.py --base origin/main   # fail if it went up

`--base` compares with the merge base of that revision and the head, so a branch
is measured by its own changes. CI runs `--base HEAD^1` on every pull request
(the merge commit's first parent is the base branch) and every push to main.

The scan is lexical. It walks the module tree from `src/sim/mod.rs` through
`mod` declarations, `#[path]` and `include!`, skips `#[cfg(test)]` modules, items
and fields, blanks comments and literals, and reads struct bodies. It does not
see module privacy or re-exports (a field behind a private module still counts),
macro-generated structs or enum variant fields.
"""
from __future__ import annotations

import argparse
import bisect
from dataclasses import dataclass
import os
from pathlib import Path, PurePosixPath
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
SCOPE = 'src/sim'
SIM = ('crate', 'sim')

_VIS = r'pub\b(?:\s*\(\s*(?:crate|self|super|in\s[^)]*)\s*\))?'
VISIBILITY = re.compile(_VIS)
STRUCT = re.compile(rf'\b(?:({_VIS})\s*)?struct\s+([A-Za-z_]\w*)')
ITEM = re.compile(rf'(?:{_VIS}\s*)?(?:(?:unsafe|async|const|default|extern)\s+)*'
                  r'(?:fn|impl|struct|enum|trait|mod|union|const|static|type|use|extern|macro_rules)\b')
FIELD_NAME = re.compile(r'((?:r#)?[A-Za-z_]\w*)\s*:')
TOKEN = re.compile(r'//|/\*|(?<!\w)[bc]?r#*"|"|\'')
BLOCK_MARK = re.compile(r'/\*|\*/')
STRING_BODY = re.compile(r'(?:[^"\\]|\\.)*"', re.S)
NOT_NEWLINE = re.compile(r'[^\n]')
CHAR = re.compile(r"'(?:\\(?:x[0-9A-Fa-f]{2}|u\{[0-9A-Fa-f]{1,6}\}|.)|[^\\'\n])'")
CFG_TEST = re.compile(r'#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]')
INNER_CFG_TEST = re.compile(r'\A\s*(?:#\s*!\s*\[[^\]]*\]\s*)*#\s*!\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]')
ATTRIBUTE = re.compile(r'#\s*!?\s*\[')
PATH_ATTRIBUTE = re.compile(r'#\s*\[\s*path\s*=\s*"([^"]*)"\s*\]')
MOD_DECLARATION = re.compile(r'\bmod\s+([A-Za-z_]\w*)\s*;')
INLINE_MOD = re.compile(r'\bmod\s+([A-Za-z_]\w*)\s*\{')
INCLUDE = re.compile(r'\binclude!\s*\(\s*"([^"]+)"\s*\)')
BRACKETS = {pair[0]: re.compile(f'[{re.escape(pair)}]') for pair in ('()', '[]', '{}')}


@dataclass(frozen=True)
class Field:
    path: str
    line: int
    struct: str
    name: str
    visibility: str


@dataclass(frozen=True)
class Unit:
    """A source file as the compiler splices it into the module tree."""
    path: str
    module: tuple[str, ...]
    test: bool
    child_dir: PurePosixPath  # where `mod x;` looks for x.rs and x/mod.rs
    source_dir: PurePosixPath  # where `#[path]` and `include!` paths start


def blank(text: str) -> str:
    """Blank comments and string/char literals, keeping offsets and newlines."""
    out, position, n = [], 0, len(text)
    while (token := TOKEN.search(text, position)):
        start, kind = token.start(), token.group(0)
        if kind == '//':
            end = text.find('\n', start)
            end = n if end < 0 else end
        elif kind == '/*':
            depth, end = 1, n
            for mark in BLOCK_MARK.finditer(text, start + 2):
                depth += 1 if mark.group(0) == '/*' else -1
                if depth == 0:
                    end = mark.end()
                    break
        elif kind == '"':
            body = STRING_BODY.match(text, start + 1)
            end = body.end() if body else n
        elif kind == "'":
            char = CHAR.match(text, start)
            if not char:
                out.append(text[position:start + 1])
                position = start + 1
                continue
            end = char.end()
        else:
            hashes = kind.count('#')
            close = text.find('"' + '#' * hashes, token.end())
            end = n if close < 0 else close + 1 + hashes
        out.append(text[position:start])
        out.append(NOT_NEWLINE.sub(' ', text[start:end]))
        position = end
    out.append(text[position:])
    return ''.join(out)


def normalize(visibility: str | None) -> str:
    return re.sub(r'\s+', '', visibility or '').replace('pub(in', 'pub(in ')


def reach(visibility: str, module: tuple[str, ...]) -> tuple[str, ...]:
    """The module whose whole subtree can see an item declared in `module`."""
    if visibility in ('', 'pub(self)'):
        return module
    if visibility in ('pub', 'pub(crate)'):
        return ('crate',)
    if visibility == 'pub(super)':
        return module[:-1]
    path = visibility[len('pub(in '):-1].split('::')
    current = module if path[0] in ('self', 'super') else ()
    for segment in path:
        current = current[:-1] if segment == 'super' else current if segment == 'self' else current + (segment,)
    return current


def matching(text: str, i: int, open_: str, close: str) -> int:
    """Index just past the bracket that closes the one at `i`."""
    depth = 0
    for mark in BRACKETS[open_].finditer(text, i):
        depth += 1 if mark.group(0) == open_ else -1
        if depth == 0:
            return mark.end()
    return len(text)


def skip_attributes(text: str, i: int) -> int:
    while True:
        while i < len(text) and text[i].isspace():
            i += 1
        attribute = ATTRIBUTE.match(text, i)
        if not attribute:
            return i
        i = matching(text, attribute.end() - 1, '[', ']')


def item_end(text: str, i: int) -> int:
    """End of the item, field or variant that starts at `i`.

    An item ends with its braced body or `;`; a field or variant also ends at a
    top-level `,`. Angle brackets are not counted (a `<` may be a comparison).
    """
    ends_at_comma = not ITEM.match(text, i)
    depth = 0
    for j in range(i, len(text)):
        c = text[j]
        if c in '([{':
            if c == '{' and depth == 0:
                return matching(text, j, '{', '}')
            depth += 1
        elif c in ')]}':
            if depth == 0:
                return j
            depth -= 1
        elif depth == 0 and (c == ';' or (c == ',' and ends_at_comma)):
            return j + 1
    return len(text)


def test_regions(clean: str) -> list[tuple[int, int]]:
    """Spans of items and fields gated by `#[cfg(test)]`."""
    return [(a.start(), item_end(clean, skip_attributes(clean, a.end()))) for a in CFG_TEST.finditer(clean)]


def inside(position: int, spans: list[tuple[int, int]]) -> bool:
    return any(a <= position < b for a, b in spans)


def inline_modules(clean: str) -> list[tuple[int, int, str]]:
    return [(m.start(), matching(clean, m.end() - 1, '{', '}'), m.group(1)) for m in INLINE_MOD.finditer(clean)]


def enclosing(position: int, modules: list[tuple[int, int, str]]) -> tuple[str, ...]:
    return tuple(name for start, end, name in modules if start < position < end)


def module_units(files: dict[str, str], cleaned: dict[str, str]) -> list[Unit]:
    """Walk the module tree from src/sim/mod.rs; unreached files are not compiled."""
    root = PurePosixPath(SCOPE)
    queue = [Unit(f'{SCOPE}/mod.rs', SIM, False, root, root)]
    units: dict[str, Unit] = {}
    while queue:
        unit = queue.pop()
        if unit.path in units or unit.path not in files:
            continue
        text, clean = files[unit.path], cleaned[unit.path]
        unit = Unit(unit.path, unit.module, unit.test or bool(INNER_CFG_TEST.match(clean)),
                    unit.child_dir, unit.source_dir)
        units[unit.path] = unit
        tests, modules = test_regions(clean), inline_modules(clean)
        test_modules = [(a, b, n) for a, b, n in modules if inside(a, tests)]
        for declaration in MOD_DECLARATION.finditer(clean):
            position, name = declaration.start(), declaration.group(1)
            nested = enclosing(position, modules)
            window = max(clean.rfind(ch, 0, position) for ch in ';{}') + 1
            test = (unit.test or bool(CFG_TEST.search(clean, window, position))
                    or bool(enclosing(position, test_modules)))
            explicit = PATH_ATTRIBUTE.search(text, window, position)
            if explicit:
                base = unit.child_dir.joinpath(*nested) if nested else unit.source_dir
                target = PurePosixPath(os.path.normpath(str(base / explicit.group(1))).replace(os.sep, '/'))
                child_dir = target.parent  # rustc treats every #[path] file as a mod.rs
            else:
                base = unit.child_dir.joinpath(*nested)
                target = next((c for c in (base / f'{name}.rs', base / name / 'mod.rs') if str(c) in files),
                              base / f'{name}.rs')
                child_dir = base / name
            queue.append(Unit(str(target), unit.module + nested + (name,), test, child_dir, target.parent))
        for include in INCLUDE.finditer(text):
            if clean[include.start():include.start() + 8] != 'include!':
                continue
            nested = enclosing(include.start(), modules)
            target = os.path.normpath(str(PurePosixPath(unit.path).parent / include.group(1))).replace(os.sep, '/')
            test = unit.test or inside(include.start(), tests) or bool(enclosing(include.start(), test_modules))
            queue.append(Unit(target, unit.module + nested, test, unit.child_dir.joinpath(*nested), unit.source_dir))
    return list(units.values())


def split_top_level(body: str) -> list[tuple[int, str]]:
    """Comma-separated entries of a struct body, with their offsets.

    Angle brackets count only outside other brackets, where a field type's
    generics are (`[u8; 1 << 4]` keeps its shift inside the brackets).
    """
    entries, depth, angle, start = [], 0, 0, 0
    for j, c in enumerate(body):
        if c in '([{':
            depth += 1
        elif c in ')]}':
            depth -= 1
        elif depth == 0 and c == '<':
            angle += 1
        elif depth == 0 and c == '>' and (j == 0 or body[j - 1] != '-'):
            angle -= 1
        elif c == ',' and depth == 0 and angle == 0:
            entries.append((start, body[start:j]))
            start = j + 1
    entries.append((start, body[start:]))
    return entries


def struct_fields(unit: Unit, clean: str) -> list[Field]:
    """Every struct field outside `#[cfg(test)]` items; `(narrow)` marks those that stay inside a sim submodule."""
    excluded = test_regions(clean)
    modules = inline_modules(clean)
    line_starts = [0] + [m.end() for m in re.finditer('\n', clean)]
    fields = []
    for struct in STRUCT.finditer(clean):
        if inside(struct.start(), excluded):
            continue
        module = unit.module + enclosing(struct.start(), modules)
        struct_reach = reach(normalize(struct.group(1)), module)
        i = struct.end()
        while i < len(clean) and clean[i].isspace():
            i += 1
        if i < len(clean) and clean[i] == '<':
            depth = 0
            while i < len(clean):
                if clean[i] == '<':
                    depth += 1
                elif clean[i] == '>' and clean[i - 1] != '-':
                    depth -= 1
                    if depth == 0:
                        i += 1
                        break
                i += 1
        while i < len(clean) and clean[i].isspace():
            i += 1
        if i < len(clean) and clean[i] == '(':
            close, tuple_struct = matching(clean, i, '(', ')'), True
        else:
            ends = [k for k in (clean.find('{', i), clean.find(';', i)) if k >= 0]
            if not ends or clean[min(ends)] != '{':
                continue
            i = min(ends)
            close, tuple_struct = matching(clean, i, '{', '}'), False
        for index, (offset, entry) in enumerate(split_top_level(clean[i + 1:close - 1])):
            start = skip_attributes(entry, 0)
            absolute = i + 1 + offset + start
            if inside(absolute, excluded):
                continue
            visibility = VISIBILITY.match(entry, start)
            rest = entry[visibility.end() if visibility else start:].strip()
            if not rest:
                continue
            if tuple_struct:
                name = str(index)
            else:
                named = FIELD_NAME.match(rest)
                if not named:
                    continue
                name = named.group(1)
            spelled = normalize(visibility.group(0) if visibility else '')
            field_reach = reach(spelled, module) if spelled else module + ('<private>',)
            effective = max(struct_reach, field_reach, key=len)
            fields.append(Field(unit.path, bisect.bisect_right(line_starts, absolute), struct.group(2), name,
                                spelled if len(effective) <= len(SIM) else f'{spelled or "private"} (narrow)'))
    return fields


def crate_visible_fields(files: dict[str, str]) -> list[Field]:
    cleaned = {path: blank(text) for path, text in files.items()}
    result = []
    for unit in sorted(module_units(files, cleaned), key=lambda u: u.path):
        if not unit.test:
            result.extend(f for f in struct_fields(unit, cleaned[unit.path]) if not f.visibility.endswith('(narrow)'))
    return result


def worktree_files(root: Path) -> dict[str, str]:
    return {path.relative_to(root).as_posix(): path.read_text(encoding='utf-8', errors='replace')
            for path in sorted((root / SCOPE).rglob('*.rs'))}


def git(root: Path, *args: str, stdin: bytes | None = None) -> bytes:
    return subprocess.run(['git', *args], cwd=root, capture_output=True, check=True, input=stdin).stdout


def revision_files(root: Path, revision: str) -> dict[str, str]:
    paths = [p for p in git(root, 'ls-tree', '-r', '-z', '--name-only', revision, '--', SCOPE).decode().split('\0')
             if p.endswith('.rs')]
    batch = git(root, 'cat-file', '--batch', stdin=''.join(f'{revision}:{p}\n' for p in paths).encode())
    files, offset = {}, 0
    for path in paths:
        header_end = batch.index(b'\n', offset)
        size = int(batch[offset:header_end].split()[2])
        files[path] = batch[header_end + 1:header_end + 1 + size].decode('utf-8', errors='replace')
        offset = header_end + 1 + size + 1
    return files


def new_fields(base: list[Field], head: list[Field]) -> list[Field]:
    """Head fields whose struct and name the base does not have (a moved struct keeps its key)."""
    known = {(f.struct, f.name) for f in base}
    return [f for f in head if (f.struct, f.name) not in known]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    parser.add_argument('--base', help='compare with the merge base of this revision, e.g. origin/main or HEAD^1')
    parser.add_argument('--head', help='revision to check instead of the working tree')
    args = parser.parse_args(argv)
    try:
        head = crate_visible_fields(revision_files(ROOT, args.head) if args.head else worktree_files(ROOT))
        if not args.base:
            print(f'{len(head)} crate-visible struct fields in {SCOPE}.')
            return 0
        merge_base = git(ROOT, 'merge-base', args.base, args.head or 'HEAD').decode().strip()
        base = crate_visible_fields(revision_files(ROOT, merge_base))
    except subprocess.CalledProcessError as error:
        print(f'git failed: {error.stderr.decode(errors="replace").strip()}', file=sys.stderr)
        return 2
    against = f'{args.base} (merge base {merge_base[:10]})'
    if len(head) <= len(base):
        print(f'{len(head)} crate-visible struct fields in {SCOPE}; {len(base)} at {against}.')
        return 0
    added = new_fields(base, head)
    print(f'{SCOPE} has {len(head)} crate-visible struct fields, up from {len(base)} at {against}.\n'
          'New simulation state must be private to its owning module, with changes going through '
          'that owner (AGENTS.md/CLAUDE.md: "one authoritative owner"). Fields added here:', file=sys.stderr)
    for field in added[:40]:
        print(f'  {field.path}:{field.line} {field.struct}.{field.name} ({field.visibility})', file=sys.stderr)
    if len(added) > 40:
        print(f'  ... and {len(added) - 40} more', file=sys.stderr)
    return 1


if __name__ == '__main__':
    sys.exit(main())
