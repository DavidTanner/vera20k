"""Check the retained production rally/Stop pairs against original native stores.

The existing map-observation owner validates capture receipts. This comparison
adds only the bounded rally relation: native RGB565 stores on unobstructed floor,
all changed frame pixels, and the observed ArchiveTarget cleanup. It does not
certify native Scenario initialization, revelation history or object occlusion.
"""

from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import json
from pathlib import Path
import tempfile

from tools.map_observation import validate_capture, validate_run


HERE = Path(__file__).resolve().parent
RUNS = ('rally-clear-v2', 'rally-stop-v2',
        'rally-shroud-v2', 'rally-shroud-stop-v2')
FINAL_RUN = 'rally-clear-final'
FILES = ('run.json', 'profile.json', 'contract.json', 'config.toml',
         'stdout.log', 'stderr.log', 'child-output/capture.json',
         'child-output/frame.bgra')
# Chosen by inspecting the terrain below the barracks, before pixel comparison.
# The whole 160x120 native crop includes the building; only its lower floor is
# used for an unobstructed store assertion. All frame deltas are checked too.
FLOOR_CROP_RECT = (0, 52, 160, 68)


def digest(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def document(path: Path):
    return json.loads(path.read_bytes())


def check(condition, message):
    if not condition:
        raise ValueError(message)


def packed565(bgra: bytes) -> int:
    # Native RGB565 layout, independently fixed in the native surface fixture.
    # Compare packed stores; exact BGRA values are reported without asserting
    # a universal DirectDraw display expansion codebook.
    b, g, r, a = bgra
    check(a == 255, 'rally output must be opaque')
    return ((r >> 3) << 11) | ((g >> 2) << 5) | (b >> 3)


def compare_pair(visible, stopped, row, no_target):
    a, pixels = visible
    b, stop_pixels = stopped
    profile = a['profile']['request']
    stop_profile = b['profile']['request']
    expected_stop = json.loads(json.dumps(profile))
    expected_stop['commands'].append({
        'issue_after_step': 1490, 'owner': 'VERA-OBSERVER',
        'payload': {'Stop': {'entity_id': 1477}}})
    check(stop_profile == expected_stop, 'Stop pair differs by more than the late Stop command')
    for key in ('camera', 'internal_extent', 'surface_extent', 'gpu', 'cursor_position'):
        check(a['render'][key] == b['render'][key], f'pair render {key} differs')
    check(a['render']['camera']['zoom'] == 1, 'native crop requires zoom1')
    check(profile['ticks'] == row['input']['frame'] == 1500, 'native frame differs')
    width, height = profile['width'], profile['height']
    check((width, height) == (800, 600), 'unexpected production extent')
    camera = a['render']['camera']['top_left']
    check(all(value == int(value) for value in camera), 'crop requires integer camera')
    origin = [row['camera'][0] - int(camera[0]),
              row['camera'][1] - int(camera[1]) + 15]
    check(origin == [415, 296], 'production crop moved')
    check(all(not result['pixels'] for result in no_target['passes']),
          'original targetless producer drew pixels')
    af, bf = a['observations']['frames'], b['observations']['frames']
    check(len(af) == len(bf) == 1501, 'incomplete frame observations')
    for frame_a, frame_b in zip(af, bf):
        expected = json.loads(json.dumps(frame_a))
        if expected['completed_steps'] > 1490:
            factory = next(actor for actor in expected['actors'] if actor['stable_id'] == 1477)
            check(factory['archive'] == {'Cell': [35, 89]}, 'source rally was lost')
            factory['archive'] = None
        check(expected == frame_b, f'unexpected state delta at step{frame_a["completed_steps"]}')
    factory = next(actor for actor in af[-1]['actors'] if actor['stable_id'] == 1477)
    check(factory['type_id'] == 'GAPILE' and factory['active'] and not factory['in_limbo'],
          'factory is not a live placed GAPILE')
    check(factory['physical_leptons'] == row['input']['source'], 'native source differs')
    target = next(cell for cell in af[-1]['terrain'] if cell['cell'] == row['input']['target_cell'])
    check([target['level'], target['slope'], target['raw_bridge_flags']] ==
          [row['input']['level'], row['input']['slope'], row['input']['flags']],
          'native target terrain differs')
    native = {(origin[0] + x, origin[1] + y): word
              for x, y, word in row['passes'][-1]['pixels']}
    check(len(native) == 106, 'original production crop has changed')
    pixel = lambda raw, point: raw[(point[1] * width + point[0]) * 4:
                                  (point[1] * width + point[0]) * 4 + 4]
    changed = [(i % width, i // width) for i in range(width * height)
               if pixels[i * 4:i * 4 + 4] != stop_pixels[i * 4:i * 4 + 4]]
    check(all(point in native for point in changed), 'frame changed outside native rally stores')
    check(all(packed565(pixel(pixels, point)) == native[point] for point in changed),
          'a changed rally pixel differs from the native packed store')
    x, y, w, h = FLOOR_CROP_RECT
    floor = [point for point in native
             if origin[0] + x <= point[0] < origin[0] + x + w
             and origin[1] + y <= point[1] < origin[1] + y + h]
    check(len(floor) == 62, 'declared floor no longer covers62 native stores')
    check(all(packed565(pixel(pixels, point)) == native[point] for point in floor),
          'unobstructed floor differs from a native packed store')
    check(len(changed) == 76, 'retained scene no longer has76 visible rally stores')
    return dict(status='PASS', compared_frame_pixels=width * height,
                native_stores=len(native), visible_native_stores=len(changed),
                unchanged_covered_stores=len(native) - len(changed),
                crop_origin=origin, crop_extent=row['size'],
                floor_crop_rect=FLOOR_CROP_RECT, floor_native_stores=len(floor),
                observed_bgra=sorted({tuple(pixel(pixels, point)) for point in changed}),
                observed_state_delta='ArchiveTarget only, steps1491..1500',
                capture_frame_wall_mean_ms=[a['render']['frame_wall_mean_ms'],
                                            b['render']['frame_wall_mean_ms']])


def compare(captures):
    native = document(HERE / 'rally.json')
    rows = native['production_crop_cases']
    row = next(row for row in rows if not row['input'].get('no_target', False))
    no_target = next(row for row in rows if row['input'].get('no_target', False))
    profiles = [capture[0]['profile']['request'] for capture in captures]
    shrouded = json.loads(json.dumps(profiles[0]))
    shrouded['launch']['options']['shroud'] = True
    check(profiles[2] == shrouded, 'shroud pair changed other profile inputs')
    result = dict(status='PASS', native_payload_sha256=digest((HERE / 'rally.json').read_bytes()),
                clear=compare_pair(*captures[:2], row, no_target),
                shrouded=compare_pair(*captures[2:4], row, no_target),
                limitations=[
                    'Native original producer pixels on prepared captured inputs; no whole native Scenario comparison.',
                    'Floor stores and all changed frame pixels match RGB565. Covered pixels retain the Stop image; native object occlusion itself is not certified.',
                    'Shrouded pair checks production rendering and cleanup, not native revelation history.',
                    'Cadence is the existing last60 frame wall mean including simulation, observation and pacing, not GPU duration or ordinary play FPS.'])
    if len(captures) == 5:
        check(captures[4][1] == captures[0][1], 'final candidate changed the complete clear frame')
        result['final_candidate'] = compare_pair(captures[4], captures[1], row, no_target)
        result['final_candidate']['complete_frame_identical_to_clear'] = True
    return result


def validation_summary(validated):
    # Full observations and presentation-clock draws already live in the
    # compressed run/capture receipts. Retain their identities, not four more
    # uncompressed copies of the same trajectories.
    summary = {key: value for key, value in validated.items()
               if key not in ('capture', 'presentation_clock')}
    summary['capture'] = {key: value for key, value in validated['capture'].items()
                          if key not in ('observations', 'presentation_clock')}
    return summary


def check_run_name(name):
    check(isinstance(name, str) and bool(name) and Path(name).name == name
          and not any(char in name for char in ('/', '\\', ':'))
          and name not in ('.', '..') and name not in RUNS,
          'final candidate must name one distinct capture directory')


def record(root: Path, archive: Path, final_candidate: str | None = None):
    check(not archive.exists(), 'refusing to overwrite retained evidence')
    captures, validations, files = [], {}, {}
    if final_candidate:
        check_run_name(final_candidate)
    runs = RUNS + ((final_candidate,) if final_candidate else ())
    for name in runs:
        run = (root / name).resolve()
        validated = validate_run(run)
        check(validated['status'] == 'VALID', f'{name}: {validated["errors"]}')
        validations[name] = validation_summary(validated)
        captures.append((document(run / 'child-output/capture.json'),
                         (run / 'child-output/frame.bgra').read_bytes()))
    result = compare(captures)
    archive.mkdir(parents=True)
    for name in runs:
        for relative in FILES:
            raw = (root / name / relative).read_bytes()
            target = archive / name / (relative + '.gz')
            target.parent.mkdir(parents=True, exist_ok=True)
            compressed = io.BytesIO()
            with gzip.GzipFile(filename='', fileobj=compressed, mode='wb', mtime=0) as output:
                output.write(raw)
            target.write_bytes(compressed.getvalue())
            files[str(target.relative_to(archive))] = dict(
                bytes=len(raw), sha256=digest(raw), gzip_sha256=digest(compressed.getvalue()))
    receipt = dict(comparison=result, final_candidate=bool(final_candidate),
                   final_candidate_run=final_candidate,
                   live_run_validations=validations, files=files)
    (archive / 'receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
    return result


def recheck(archive: Path):
    receipt = document(archive / 'receipt.json')
    candidate = receipt.get('final_candidate_run', FINAL_RUN)
    if receipt.get('final_candidate', False):
        check_run_name(candidate)
    runs = RUNS + ((candidate,) if receipt.get('final_candidate', False) else ())
    expected_files = {f'{name}/{relative}.gz' for name in runs for relative in FILES}
    check(set(receipt['files']) == expected_files, 'archive file inventory differs')
    captures = []
    with tempfile.TemporaryDirectory(prefix='vera-rally-evidence-') as temporary:
        root = Path(temporary).resolve()
        for relative, identity in receipt['files'].items():
            raw_gzip = (archive / relative).read_bytes()
            check(digest(raw_gzip) == identity['gzip_sha256'], f'{relative}: gzip identity')
            raw = gzip.decompress(raw_gzip)
            check(len(raw) == identity['bytes'] and digest(raw) == identity['sha256'],
                  f'{relative}: byte identity')
            target = root / relative.removesuffix('.gz')
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(raw)
        for name in runs:
            run = root / name
            wrapper = document(run / 'run.json')
            profile = document(run / 'profile.json')
            # Reuse strict capture-byte validation. The original executable was
            # checked at record time and is identified in the retained receipt;
            # archive rechecks deliberately make no live executable claim.
            checked = validate_capture(run / 'child-output', profile, wrapper['inputs'])
            expected = dict(wrapper['capture'])
            for key in ('manifest', 'frame'):
                # Extraction changes only the snapshot's absolute path. Keep
                # the original byte length and digest in this comparison.
                expected[key] = dict(expected[key], path=checked.evidence[key]['path'])
            check(checked.evidence == expected, f'{name}: wrapper evidence differs')
            captures.append((document(run / 'child-output/capture.json'), checked.frame.raw))
        result = compare(captures)
        check(json.loads(json.dumps(result)) == receipt['comparison'], 'comparison result changed')
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('operation', choices=('record', 'check'))
    parser.add_argument('--archive', type=Path, required=True)
    parser.add_argument('--runs', type=Path)
    parser.add_argument('--final-candidate', nargs='?', const=FINAL_RUN, metavar='RUN_NAME',
                        help='record an additional clear capture; defaults to rally-clear-final')
    args = parser.parse_args()
    if args.operation == 'record':
        if args.runs is None:
            parser.error('--runs is required for record')
        result = record(args.runs, args.archive, args.final_candidate)
    else:
        result = recheck(args.archive)
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
