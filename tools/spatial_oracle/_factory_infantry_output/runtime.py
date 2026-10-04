"""Portable identity, I/O and comparison support; no native gameplay implementation."""
from pathlib import Path
import gzip
import functools
import contextlib
import hashlib
import importlib.util
import json
import os
import sys

HERE = Path(__file__).resolve().parent
REPO_ROOT = HERE.parents[2]
_ASSETS = None
# These are host/driver identities, checked separately. Every other receipt
# field, including all PCs, calls, timer words, code guards and RNG bytes, is
# retained in the comparison. No gameplay field is removed by key or value.
HOST_KEYS = frozenset(('driver_sha256', 'derive_ast_sha256', 'queue_ast_sha256',
    'derived_queue_ast_sha256', 'setup_ast_sha256', 'derived_caller_ast_sha256',
    'derived_observer_ast_sha256', 'immutable_drivers', 'construction_driver_sha256',
    'parent_driver_sha256', 'frozen_owner_files', 'source_frozen', 'assets_reference',
    'native_path'))


def require(ok, message):
    if not ok:
        raise ValueError(message)


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def canonical_sha(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':')).encode()).hexdigest()


@functools.cache
def metadata():
    return json.loads((HERE / 'meta.json').read_text())


def caller_sha(name):
    return metadata()['caller_files'][name]['sha256']


def verify_callers():
    for name, row in metadata()['caller_files'].items():
        path = HERE / name
        require(path.is_file() and sha(path) == row['sha256'],
                'Portable caller changed: ' + name + '; refresh after original native comparison')


def load_module(name, path):
    path = Path(path).resolve()
    require(path.parent == HERE, 'Caller outside factory_infantry_output package: ' + str(path))
    verify_callers()
    # A package-qualified module name preserves relative imports in the six
    # retained caller adapters without inserting any machine-local sys.path.
    spec = importlib.util.spec_from_file_location(__package__ + '.' + name, path)
    require(spec is not None and spec.loader is not None, 'Unable to load original caller')
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def configure_assets(path=None):
    global _ASSETS
    value = path or os.environ.get('VERA20K_FACTORY_INFANTRY_OUTPUT_ASSETS')
    require(value, 'Provide --assets or VERA20K_FACTORY_INFANTRY_OUTPUT_ASSETS; no machine-local fallback')
    root = Path(value).expanduser().resolve()
    require(root.is_dir(), 'Physical input directory missing: ' + str(root))
    actual = {p.name: p for p in root.iterdir()}
    required = metadata()['physical_inputs']
    present = {name for name, row in required.items() if not row.get('absent')}
    require(set(actual) == present,
            'Physical input root differs; the shared Reader reads every root entry. Missing=' +
            str(sorted(present-set(actual))) + '; extra=' + str(sorted(set(actual)-present)))
    require(len({name.upper() for name in actual}) == len(actual), 'Case-colliding physical inputs')
    for name, row in required.items():
        if row.get('absent'):
            require(name.upper() not in {n.upper() for n in actual}, 'Recorded absent input appeared: ' + name)
        else:
            p = actual[name]
            require(p.is_file() and p.stat().st_size == row['bytes'] and sha(p) == row['sha256'],
                    'Physical input changed: ' + name)
    _ASSETS = root
    return root


def assets_root():
    require(_ASSETS is not None, 'Use the public factory_infantry_output replay owner to configure physical inputs')
    return _ASSETS


def receipt_path(control):
    require(control in ('no_rally', 'rally'), 'Unknown original control')
    return HERE / 'accepted' / (control + '.receipt.json.gz')


def read_pinned(relative):
    row = metadata()['retained_files'][relative]
    path = HERE / relative
    raw = path.read_bytes()
    require(len(raw) == row['bytes'] and hashlib.sha256(raw).hexdigest() == row['sha256'],
            'Retained native artifact changed: ' + relative)
    plain = gzip.decompress(raw) if path.suffix == '.gz' else raw
    if 'uncompressed_sha256' in row:
        require(len(plain) == row['uncompressed_bytes'] and
                hashlib.sha256(plain).hexdigest() == row['uncompressed_sha256'],
                'Decompressed native artifact changed: ' + relative)
    return json.loads(plain)


def read_accepted(control):
    return read_pinned('accepted/' + control + '.receipt.json.gz')


def normalize(value, path=()):
    """Remove only declared host metadata and normalize the physical-root path."""
    if isinstance(value, dict):
        return {k: normalize(v, path+(k,)) for k, v in value.items()
                if k not in HOST_KEYS and
                not (k == 'runtime' and path[-1:] in (('queue',), ('environment',), ('native_environment',))) and
                not (k == 'primary_comparison' and not path) and
                not (k == 'source' and path[-1:] == ('original_infantry_vtable',))}
    if isinstance(value, list):
        return [normalize(v, path+(i,)) for i, v in enumerate(value)]
    if isinstance(value, str):
        roots = [metadata()['historical_physical_root']]
        if _ASSETS is not None:
            roots.append(str(_ASSETS))
        for root in roots:
            if value.startswith(root + '/'):
                return '<physical-input-root>' + value[len(root):]
    return value


def compare_primary(control, actual):
    from tools import native_oracle as native
    prior = read_accepted(control)
    expected, observed = normalize(prior), normalize(actual)
    difference = native.first_difference(expected, observed)
    require(difference is None, 'Original native primary differs: ' + str(difference))
    joined = actual['joined'] if control == 'rally' else actual
    return dict(raw_receipt_equal=actual == prior, native_observations_equal=True,
        canonical_sha256=canonical_sha(actual), prior_canonical_sha256=canonical_sha(prior),
        native_observations_sha256=canonical_sha(observed),
        prior_file_sha256=metadata()['controls'][control]['primary_uncompressed_sha256'],
        prior_path='accepted/' + control + '.receipt.json.gz',
        final_frame=joined['admitted_queue']['queue']['final']['frame'],
        exclusions=sorted(HOST_KEYS)+['queue.runtime', 'environment.runtime',
            'native_environment.runtime', '$.primary_comparison',
            'original_infantry_vtable.source', 'physical-root pathname prefix'])


def verify_helpers():
    inspections = {}
    for name, row in metadata()['inspection_owner_files'].items():
        path = REPO_ROOT / name
        observed = sha(path) if path.is_file() else None
        require(observed == row['sha256'] and path.stat().st_size == row['bytes'],
                'Unsupported inspection-owner version: ' + name +
                ' observed=' + str(observed) + ' expected=' + row['sha256'])
        inspections[name] = observed
    profiles = metadata()['helper_profiles']
    paths = set().union(*(set(p['files']) for p in profiles.values()))
    actual = {name: sha(REPO_ROOT/name) if (REPO_ROOT/name).is_file() else None for name in paths}
    for name, profile in profiles.items():
        if actual == profile['files']:
            return dict(profile=name, imported_owner_files=len(actual),
                profile_sha256=canonical_sha(actual), status=profile['status'],
                inspection_owner_files=len(inspections),
                inspection_owner_sha256=canonical_sha(inspections))
    candidates = []
    for name, profile in profiles.items():
        changes = [(path, actual[path], profile['files'].get(path)) for path in sorted(paths)
                   if actual[path] != profile['files'].get(path)]
        candidates.append((len(changes), name, changes))
    _, name, changes = min(candidates)
    require(False, 'Unsupported shared-helper version (nearest profile ' + name + '): ' +
        '; '.join(path + ' observed=' + str(observed) + ' expected=' + str(expected)
                  for path, observed, expected in changes) +
        '. Re-execute the original native controls before registering a new compatibility profile.')


@contextlib.contextmanager
def candidate_helper_profile():
    """Explicit comparison-only helper map; never register compatibility here.

    The unchanged verification/census owners still check every imported byte.
    Complete original primary/private comparisons must pass before an on-disk
    compatibility profile may refer to this map. Default callers cannot use it.
    """
    meta = metadata()
    name = 'unregistered-initialized-candidate'
    require(name not in meta['helper_profiles'], 'Candidate helper context already active')
    files = meta.get('initialized_candidate_helper_files')
    require(files and all(sha(REPO_ROOT/path) == digest for path, digest in files.items()),
            'Explicit initialized candidate helper bytes changed')
    callers = meta.get('initialized_candidate_caller_files', {})
    require(set(callers) <= {'runtime.py', 'initialized.py'}, 'Unsupported candidate caller')
    original_callers = {path: meta['caller_files'][path] for path in callers}
    for path, row in callers.items():
        source = HERE/path
        require(source.is_file() and source.stat().st_size == row['bytes'] and sha(source) == row['sha256'],
                'Explicit initialized candidate caller bytes changed: '+path)
    for path, row in callers.items():
        meta['caller_files'][path] = dict(original_callers[path], **row)
    meta['helper_profiles'][name] = dict(files=files,
        status='candidate native comparison only; not registered compatibility')
    try:
        yield verify_helpers()
    finally:
        del meta['helper_profiles'][name]
        meta['caller_files'].update(original_callers)


def write_new(path, value):
    path = Path(path)
    require(not path.exists(), 'Refuse to overwrite saved evidence: ' + str(path))
    raw = (json.dumps(value, indent=2) + '\n').encode()
    # Exclusive create also prevents a race after the existence check.
    with path.open('xb') as output:
        output.write(gzip.compress(raw, mtime=0) if path.suffix == '.gz' else raw)
