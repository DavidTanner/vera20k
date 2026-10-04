"""Check retained procedural-line captures against original native stores.

The existing map-observation owner validates capture receipts. Rally compares
native RGB565 stores on unobstructed floor and ArchiveTarget cleanup. Ordinary
Move compares opaque native stores, real input and matched-time controls. This
does not certify native Scenario initialization, revelation history or occlusion.
The historical module name remains the shared archive owner for rally and Move.
"""

from __future__ import annotations

import argparse
import copy
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


def action_cases():
    rows = document(HERE / 'action_lines.json')['production_cases']
    cases = {row['production']['run']: row for row in rows}
    check(len(cases) == len(rows) == 12, 'expected twelve distinct Move captures')
    return cases


def action_actor(frame, actor_id=1374):
    return next(actor for actor in frame['actors'] if actor['stable_id'] == actor_id)


def action_stores(row):
    x, y = row['production']['crop_origin']
    return {(x + px, y + py): word for px, py, word in row['pixels']}


def changed_points(a, b, width):
    check(len(a) == len(b), 'paired frame lengths differ')
    return [(i % width, i // width) for i in range(len(a) // 4)
            if a[i * 4:i * 4 + 4] != b[i * 4:i * 4 + 4]]


def compare_action(captures, capture_hashes):
    """Bind original prepared inputs to ordinary input/renderer observations.

    Pixel expectations come only from the original executable. The comparisons
    of matched-time controls below establish which production pixels changed;
    they do not synthesize native terrain, object drawing or background pixels.
    """
    cases = action_cases()
    check(len(captures) == len(capture_hashes) == len(cases), 'Move capture inventory differs')
    by_name = dict(zip(cases, captures, strict=True))
    results = {}
    for (name, row), (capture, pixels), capture_hash in zip(
            cases.items(), captures, capture_hashes, strict=True):
        proof, native = row['production'], row['input']
        profile = capture['profile']['request']
        frame = capture['observations']['frames'][-1]
        actor = action_actor(frame, proof['actor_id'])
        observed_input = frame['input']
        check(capture_hash == proof['capture_sha256'], f'{name}: native receipt binding differs')
        check(capture['profile']['sha256'] == proof['profile_sha256'], f'{name}: profile differs')
        check(digest(pixels) == capture['frame']['sha256'] == proof['frame_sha256'],
              f'{name}: native frame binding differs')
        check(capture['inputs']['executable']['sha256'] == proof['executable_sha256'],
              f'{name}: native executable binding differs')
        check([profile['width'], profile['height']] == [800, 600], f'{name}: unexpected extent')
        check(capture['map_source']['source_sha256'] ==
              '7a390de363f79743dd54897a49302869a795f839f3387ff03e8c0b70a519e17e',
              f'{name}: retail XMP03T4 bytes differ')
        check(native['map_size'] == [80, 85], f'{name}: native map Size differs')
        check(actor['type_id'] == 'MTNK' and actor['owner'] == profile['launch']['player_name']
              and actor['active'] and not actor['in_limbo'] and actor['health'] > 0,
              f'{name}: actor is not a live local MTNK')
        check(actor['target'] is None, f'{name}: attack route entered')
        check(actor['physical_leptons'] == native['source'], f'{name}: native source differs')
        check(actor['on_bridge'] == native.get('on_bridge', False), f'{name}: source bridge differs')
        selected = proof['actor_id'] in observed_input['selected_ids']
        check(not observed_input['selection_pending'] and selected == native['selected'],
              f'{name}: native selection differs')
        check((actor['nav'] is None) == native.get('nav_null', False),
              f'{name}: native NavCom presence differs')
        if actor['nav'] is not None:
            target = actor['nav']['Cell']
            check([target['rx'], target['ry']] == native['target_cell'],
                  f'{name}: native NavCom coordinate differs')
            terrain = next(cell for cell in frame['terrain'] if cell['cell'] == native['target_cell'])
            check(terrain['allocated'] and
                  [terrain['level'], terrain['slope'], terrain['raw_bridge_flags']] ==
                  [native['level'], native['slope'], native['flags']],
                  f'{name}: native target terrain differs')
        gestures = capture['observations']['gesture_input']
        check(gestures['tactical_extent'] == [632, 568], f'{name}: tactical extent differs')
        last = gestures['receipts'][-1]
        check(last['issued_binary_frame'] == native['timer_start'] and
              last['after']['target_line_remaining'] == 25,
              f'{name}: native restart input differs')
        check(frame['binary_frame'] == native['frame'] == profile['ticks'],
              f'{name}: native final frame differs')
        check(observed_input['target_line_remaining'] == max(0, 25 - (native['frame'] - native['timer_start'])),
              f'{name}: observed restart/expiry interval differs')
        camera = capture['render']['camera']
        origin = proof['crop_origin']
        check(camera['zoom'] == 1 and
              native['camera'] == [camera['top_left'][0] + origin[0],
                                   camera['top_left'][1] + origin[1] - 15],
              f'{name}: crop camera differs')
        stores = action_stores(row)
        check(bool(stores) == (selected and actor['nav'] is not None and
                               observed_input['target_line_active']),
              f'{name}: original draw admission differs')
        for (x, y), word in stores.items():
            check(0 <= x < 632 and 0 <= y < 568, f'{name}: store escaped tactical viewport')
            offset = (y * 800 + x) * 4
            check(packed565(pixels[offset:offset + 4]) == word,
                  f'{name}: original opaque store differs at{x},{y}')
        results[name] = dict(native_stores=len(stores), crop_origin=origin,
                             source=actor['physical_leptons'], nav=actor['nav'],
                             final_input=observed_input, frame=frame['binary_frame'],
                             frame_wall_mean_ms=capture['render']['frame_wall_mean_ms'])

    pairs = {}
    specifications = (
        ('move', 'unit-move-active-v1', 'unit-selected-only-v1'),
        ('stop', 'unit-move-before-stop-v1', 'unit-move-stopped-v1'),
        ('reselect', 'unit-move-reselect-v1', 'unit-move-expired-next-v1'),
        ('arrived', 'unit-move-arrived-reselect-v1', 'unit-move-arrived-control-v1'),
    )
    for kind, active_name, control_name in specifications:
        active, pixels = by_name[active_name]
        control, control_pixels = by_name[control_name]
        expected_profile = copy.deepcopy(active['profile']['request'])
        if kind == 'stop':
            expected_profile['commands'].append({
                'issue_after_step': 6, 'owner': 'VERA-OBSERVER',
                'payload': {'Stop': {'entity_id': 1374}}})
        else:
            expected_profile['gestures'].pop()
        check(control['profile']['request'] == expected_profile, f'{kind}: control profile differs')
        for key in ('camera', 'internal_extent', 'surface_extent', 'gpu', 'cursor_position'):
            check(active['render'][key] == control['render'][key], f'{kind}: render {key} differs')
        frames = active['observations']['frames']
        controls = control['observations']['frames']
        check(len(frames) == len(controls), f'{kind}: incomplete paired observations')
        for index, (observed, comparison) in enumerate(zip(frames, controls, strict=True)):
            expected = copy.deepcopy(observed)
            if index == len(frames) - 1:
                if kind == 'move':
                    check(action_actor(expected)['mission']['queued'] == 2, 'Move mission not queued')
                    action_actor(expected)['nav'] = None
                    action_actor(expected)['mission']['queued'] = -1
                    expected['input']['target_line_remaining'] = 23
                elif kind == 'stop':
                    action_actor(expected)['nav'] = None
                else:
                    expected['input']['target_line_active'] = False
                    expected['input']['target_line_remaining'] = 0
            check(expected == comparison, f'{kind}: unexplained state delta at step{index}')
        changed = changed_points(pixels, control_pixels, 800)
        stores = action_stores(cases[active_name])
        tactical = [point for point in changed if point[0] < 632 and point[1] < 568]
        if kind == 'arrived':
            check(not stores and not tactical, 'arrival/reselection changed tactical pixels')
            # The real click resets the neutral sidebar hover. Its tooltip is
            # later than Tactical (GScreen4F4593), outside this line comparison.
            check(all(706 <= x < 800 and 572 <= y < 592 for x, y in changed),
                  'arrival control changed outside the observed sidebar tooltip')
        else:
            check(bool(changed) and all(point in stores for point in changed),
                  f'{kind}: complete frame changed outside original line stores')
        pairs[kind] = dict(compared_frame_pixels=480000,
                           native_stores=len(stores), changed_pixels=len(changed),
                           changed_tactical_pixels=len(tactical),
                           exact_tactical_pixels=632 * 568 if kind == 'arrived' else None,
                           state_boundaries=len(frames))
    return dict(status='PASS', native_payload_sha256=digest((HERE / 'action_lines.json').read_bytes()),
                captures=results, paired_controls=pairs, limitations=[
                    'Original Tactical/Foot/primitive execution uses prepared captured inputs; this is not a native Scenario or whole native frame comparison.',
                    'Every native opaque line store matches RGB565. Three matched-time controls check all 480000 pixels; arrival/reselection matches all 358976 tactical pixels and differs only in the observed sidebar tooltip.',
                    'Captured nonshift fresh Move starts with an empty NavQueue and executes native clear_queue1; queue absence and flat-ground source equivalence use the traced production owners, not an extra diagnostic state copy.',
                    'The existing last60-frame wall mean includes simulation, observation and pacing. Short captures include startup; no GPU-duration or ordinary-play FPS claim.'])


def run_names(family, final_candidate):
    check(family in ('rally', 'unit-move'), 'unknown procedural comparison family')
    if final_candidate:
        check_run_name(final_candidate)
    if family == 'unit-move':
        runs = tuple(action_cases())
        return runs + (tuple(f'{final_candidate}/{name}' for name in runs) if final_candidate else ())
    return RUNS + ((final_candidate,) if final_candidate else ())


def compare_family(family, captures, capture_hashes):
    if family != 'unit-move':
        return compare(captures)
    count = len(action_cases())
    check(len(captures) == len(capture_hashes) and len(captures) in (count, count * 2),
          'Move capture inventory differs')
    result = compare_action(captures[:count], capture_hashes[:count])
    if len(captures) == count * 2:
        candidates = {}
        for name, reference, candidate, capture_hash in zip(
                action_cases(), captures[:count], captures[count:], capture_hashes[count:], strict=True):
            before, pixels_before = reference
            after, pixels_after = candidate
            for key in ('profile', 'map_source', 'initial', 'final', 'observations'):
                check(before[key] == after[key], f'{name}: candidate {key} differs')
            for key in ('camera', 'internal_extent', 'surface_extent', 'gpu', 'cursor_position'):
                check(before['render'][key] == after['render'][key], f'{name}: candidate render {key} differs')
            check(pixels_before == pixels_after, f'{name}: candidate frame differs')
            candidates[name] = dict(
                capture_sha256=capture_hash,
                executable_sha256=after['inputs']['executable']['sha256'],
                identical_frame_sha256=digest(pixels_after),
                compared_frame_pixels=len(pixels_after) // 4,
                state_boundaries=len(after['observations']['frames']))
        result['final_candidate'] = candidates
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


def record(root: Path, archive: Path, final_candidate: str | None = None, family='rally'):
    check(not archive.exists(), 'refusing to overwrite retained evidence')
    captures, capture_hashes, validations, files = [], [], {}, {}
    runs = run_names(family, final_candidate)
    for name in runs:
        run = (root / name).resolve()
        validated = validate_run(run)
        check(validated['status'] == 'VALID', f'{name}: {validated["errors"]}')
        validations[name] = validation_summary(validated)
        captures.append((document(run / 'child-output/capture.json'),
                         (run / 'child-output/frame.bgra').read_bytes()))
        capture_hashes.append(digest((run / 'child-output/capture.json').read_bytes()))
    result = compare_family(family, captures, capture_hashes)
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
    receipt = dict(comparison=result, family=family, final_candidate=bool(final_candidate),
                   final_candidate_run=final_candidate,
                   live_run_validations=validations, files=files)
    (archive / 'receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
    return result


def recheck(archive: Path):
    receipt = document(archive / 'receipt.json')
    family = receipt.get('family', 'rally')
    candidate = receipt.get('final_candidate_run', FINAL_RUN) if receipt.get('final_candidate', False) else None
    runs = run_names(family, candidate)
    expected_files = {f'{name}/{relative}.gz' for name in runs for relative in FILES}
    check(set(receipt['files']) == expected_files, 'archive file inventory differs')
    captures, capture_hashes = [], []
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
            capture_hashes.append(digest((run / 'child-output/capture.json').read_bytes()))
        result = compare_family(family, captures, capture_hashes)
        check(json.loads(json.dumps(result)) == receipt['comparison'], 'comparison result changed')
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('operation', choices=('record', 'check'))
    parser.add_argument('--archive', type=Path, required=True)
    parser.add_argument('--runs', type=Path)
    parser.add_argument('--family', choices=('rally', 'unit-move'), default='rally',
                        help='comparison to record; check reads the retained family')
    parser.add_argument('--final-candidate', nargs='?', const=FINAL_RUN, metavar='RUN_NAME',
                        help='additional rally clear run, or directory containing all twelve Move replays')
    args = parser.parse_args()
    if args.operation == 'record':
        if args.runs is None:
            parser.error('--runs is required for record')
        result = record(args.runs, args.archive, args.final_candidate, args.family)
    else:
        result = recheck(args.archive)
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
