"""Original Infantry Unlimbo frontend identities, comparison and preservation.

The passenger fixture owns the supplied Foot4D7170 success seam. This caller
never supplies Gate, membership, placement, floor, class or RNG decisions.
Source seal:68c0abc23dc667a0de7f7ad7538365d00b8beba32c4d7b44674b0b982ad1f828.
"""
import functools
import gzip
import hashlib
import json
from pathlib import Path
import sys
import traceback

from . import runtime as rt

HOST_KEYS = frozenset(('driver_sha256', 'imports'))


@functools.cache
def metadata():
    return json.loads((rt.HERE / 'gate-meta.json').read_text())


def normalize(receipt):
    # Only these two top-level source identities differ on promotion. Arrays,
    # native state, original calls, static spans and all RNG bytes stay literal.
    return {key: value for key, value in receipt.items() if key not in HOST_KEYS}


def read_pinned(name, *, decoded=True):
    identity = metadata()['retained_files'][name]
    path = rt.HERE / name
    raw = path.read_bytes()
    rt.require(len(raw) == identity['bytes'] and
               hashlib.sha256(raw).hexdigest() == identity['sha256'],
               'Retained Unlimbo Gate artifact changed: ' + name)
    if path.suffix == '.gz':
        raw = gzip.decompress(raw)
        rt.require(len(raw) == identity['uncompressed_bytes'] and
                   hashlib.sha256(raw).hexdigest() == identity['uncompressed_sha256'],
                   'Retained Unlimbo Gate decoded bytes changed: ' + name)
    return json.loads(raw) if decoded else raw


def accepted():
    return read_pinned('accepted/infantry-unlimbo-gate.native.json.gz')


def verify_source(*, candidate=False):
    meta = metadata()
    public = meta['public_caller']
    path = rt.REPO_ROOT / public['path']
    rt.require(path.is_file() and path.stat().st_size == public['bytes'] and
               rt.sha(path) == public['sha256'], 'Public Unlimbo Gate caller changed')
    for name, row in meta['caller_files'].items():
        path = rt.HERE / name
        rt.require(path.is_file() and path.stat().st_size == row['bytes'] and
                   rt.sha(path) == row['sha256'], 'Unlimbo Gate caller changed: ' + name)
    rt.verify_callers()
    shared = rt.verify_helpers()
    profiles = {'unregistered-gate-candidate': dict(files=meta['candidate_helper_files'],
                    status='candidate comparison only; not registered compatibility')} \
        if candidate else meta['helper_profiles']
    paths = set().union(*(set(row['files']) for row in profiles.values()))
    actual = {name: rt.sha(rt.REPO_ROOT / name) for name in paths}
    for name, row in profiles.items():
        if actual == row['files']:
            return dict(profile=name, imported_owner_files=len(actual),
                        profile_sha256=rt.canonical_sha(actual), status=row['status'],
                        shared_factory_profile=shared)
    differences = {name: {path: dict(expected=digest, observed=actual.get(path))
                    for path, digest in row['files'].items() if actual.get(path) != digest}
                   for name, row in profiles.items()}
    raise ValueError('Unsupported Unlimbo Gate helper versions: ' + str(differences))


def census(profile):
    files = metadata()['candidate_helper_files'] \
        if profile['profile'] == 'unregistered-gate-candidate' \
        else metadata()['helper_profiles'][profile['profile']]['files']
    # A caller may have already loaded the older factory/consumer checks in the
    # same Python process. Their source versions keep their existing guard.
    shared = profile['shared_factory_profile']['profile']
    pinned = set(files) | set(rt.metadata()['helper_profiles'][shared]['files']) | \
        set(rt.metadata()['inspection_owner_files'])
    extra = []
    for module in tuple(sys.modules.values()):
        file = getattr(module, '__file__', None)
        if not file:
            continue
        path = Path(file).resolve()
        if path.is_relative_to(rt.REPO_ROOT / 'tools'):
            name = str(path.relative_to(rt.REPO_ROOT))
            if name == 'tools/spatial_oracle/factory_infantry_output.py' or name.startswith(
                    'tools/spatial_oracle/_factory_infantry_output/'):
                continue
            if name not in pinned:
                extra.append(name)
    rt.require(not extra, 'Unlimbo Gate imported unpinned shared owner: ' + str(sorted(set(extra))))


def selected_fixture(receipt=None, source_hash=None):
    from .fixture import infantry_unlimbo_gate
    if receipt is None:
        receipt = accepted()
        source_hash = metadata()['source_receipt_sha256']
    rt.require(source_hash is not None, 'Provide the observed Gate source receipt identity')
    return infantry_unlimbo_gate(receipt, source_hash)


def compare(actual):
    from tools import native_oracle as native
    meta = metadata()
    prior = accepted()
    rt.require(actual['fault'] is None, 'Original Unlimbo Gate control did not complete')
    difference = native.first_difference(normalize(prior), normalize(actual))
    rt.require(difference is None, 'Complete original Unlimbo Gate observations differ: ' + str(difference))
    digest = rt.canonical_sha(normalize(actual))
    rt.require(digest == meta['complete_normalized_original_sha256'],
               'Complete original Unlimbo Gate observation hash changed')
    return dict(complete_original_observations_equal=True,
                complete_normalized_original_sha256=digest,
                historical_receipt_sha256=meta['source_receipt_sha256'],
                host_exclusions=sorted(HOST_KEYS))


def replay(*, candidate=False):
    rt.require(sys.flags.optimize == 0, 'Whole original Unlimbo Gate emulation requires normal Python')
    profile = verify_source(candidate=candidate)
    import unicorn
    rt.require(unicorn.__version__ == rt.metadata()['native_runtime']['unicorn'],
               'Unsupported Unlimbo Gate Unicorn version: ' + unicorn.__version__)
    from tools import native_oracle as native
    rt.require(native.NATIVE_SHA256 == metadata()['native_sha256'], 'Active retail image differs')
    # Check the retained production-query input before the original cached-INI
    # reader uses it. Its historical source pathname is never opened.
    read_pinned('inputs/infantry-unlimbo-gate-retail.json')
    from . import gate
    actual = None
    try:
        actual = gate.generate()
        comparison = compare(actual)
        census(profile)
        verify_source(candidate=candidate)
        raw = (json.dumps(actual, indent=2) + '\n').encode()
        projection = selected_fixture(actual, hashlib.sha256(raw).hexdigest())
        expected = selected_fixture()
        # The selector reports each actual raw receipt SHA; only that top-level
        # source identity changes. Every selected native/prior field must match.
        rt.require({k: v for k, v in projection.items() if k != 'source_receipt_sha256'} ==
                   {k: v for k, v in expected.items() if k != 'source_receipt_sha256'},
                   'Mechanically selected original Unlimbo Gate values differ')
        return dict(schema=1, status='PASS', control='infantry_unlimbo_gate',
            native_sha256=native.NATIVE_SHA256, gate_helpers=profile,
            comparison=comparison, full_original_gate_control=actual,
            selected_native_projection=projection,
            original_json_sha256=hashlib.sha256(raw).hexdigest(),
            whole_object_completion_claimed=False, limits=metadata()['bounds'])
    except Exception as exc:
        return dict(schema=1, status='FAIL', control='infantry_unlimbo_gate',
            native_sha256=native.NATIVE_SHA256, gate_helpers=profile,
            failure=dict(type=type(exc).__name__, message=str(exc), traceback=traceback.format_exc()),
            full_original_gate_control=actual, whole_object_completion_claimed=False,
            limits=metadata()['bounds'])


def check_saved(*, candidate=False):
    profile = verify_source(candidate=candidate)
    from tools import native_oracle as native
    meta = metadata()
    image = native.image_bytes()
    rt.require(hashlib.sha256(image).hexdigest() == meta['native_sha256'] == native.NATIVE_SHA256,
               'Original Unlimbo Gate image changed')
    receipt = accepted()
    rt.require(sorted(HOST_KEYS) == meta['host_comparison_exclusions'], 'Gate exclusion census differs')
    for span in receipt['original_instructions']:
        start = int(span['start'], 16)
        end = int(span['end_exclusive'], 16)
        offset, raw = native.file_span(image, start, end - start)
        rt.require(offset == span['file_offset'] and raw.hex() == span['bytes'] and
                   hashlib.sha256(raw).hexdigest() == span['sha256'],
                   'Original Unlimbo Gate byte span changed: ' + span['name'])
    read_pinned('inputs/infantry-unlimbo-gate-retail.json')
    compare(receipt)
    rt.require(receipt['gate_reader']['ctor_default'] == 0 and
               [row['after'] for row in receipt['gate_reader']['rows']] == [1, 1, 1, 1],
               'Native Gate constructor/default/layer inheritance differs')
    fixture = rt.REPO_ROOT / meta['rust_fixture']['path']
    raw = fixture.read_bytes()
    rt.require(len(raw) == meta['rust_fixture']['bytes'] and
               hashlib.sha256(raw).hexdigest() == meta['rust_fixture']['sha256'],
               'Unlimbo Gate Rust fixture changed')
    selected = selected_fixture()
    rt.require(json.loads(raw) == selected, 'Unlimbo Gate fixture differs from its single native selector')
    rt.require(len(receipt['rows']) == meta['native_control_rows'], 'Unlimbo Gate control inventory changed')
    for row in receipt['rows']:
        rt.require(row['original_code_unchanged'], 'Original class control changed executable bytes')
        rt.require(row['active_cell_list_byte'] == 1 and row['map_fields'] == [16, -16, -16, 64, 64],
                   'Prepared map/cell-list priors differ')
        rt.require(row['gate_prior']['primary_vtable'] == '007E3EBC' and
                   row['infantry_prior']['primary_vtable'] == '007EB058', 'Native vtables differ')
        for boundary in ('rng_before', 'rng_after'):
            rt.require(set(row[boundary]) == {'main', 'scenario', 'mapgen'} and
                       all(len(bytes.fromhex(value)) == 1012 for value in row[boundary].values()),
                       'Unlimbo Gate complete three-stream boundary missing')
        rt.require(all(row['rng_before'][name] == row['rng_after'][name] for name in ('main', 'mapgen')),
                   'Unexpected Main/MapGen draw')
        requests = [event for event in row['events'] if event['name'].startswith('random_')]
        rt.require(all(event['name'] == 'random_ranged' and event['stream'] == 'scenario' and
                       event['range'] == [0, 3] and event['return_pc'] == '0048139F'
                       for event in requests), 'Unexpected original Gate random caller/stream/range')
        if requests:
            rt.require(requests[0]['rng_before'] == row['rng_before'] and
                       requests[-1]['rng_after'] == row['rng_after'], 'Original RNG request boundaries differ')
        else:
            rt.require(row['rng_before'] == row['rng_after'], 'Unobserved original RNG advance')
        foot = [event for event in row['events'] if event['name'] == 'foot_handoff']
        rt.require(bool(foot) == bool(row['result']) and
                   all(event['return_pc'] == '0051E0DB' and event['facing'] == 128 and
                       event['return_eax'] == 1 for event in foot), 'Inherited supplied Foot handoff differs')
        rt.require(row['water_sentinel'] == (2 if foot else 0), 'Class water sentinel boundary differs')
    census(profile)
    return dict(schema=1, status='PASS', control='infantry_unlimbo_gate',
        native_sha256=native.NATIVE_SHA256, gate_helpers=profile,
        native_control_rows=len(receipt['rows']),
        original_byte_spans=len(receipt['original_instructions']),
        source_receipt_sha256=meta['source_receipt_sha256'],
        complete_normalized_original_sha256=meta['complete_normalized_original_sha256'],
        rust_fixture_sha256=meta['rust_fixture']['sha256'],
        whole_object_completion_claimed=False, limits=meta['bounds'])
