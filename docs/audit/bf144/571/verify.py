import gzip
import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parent
labels = ('double', 'single')


def raw(path):
    if path.exists():
        return path.read_bytes()
    return gzip.decompress(path.with_suffix('.json.gz').read_bytes())


def read(path):
    return json.loads(raw(path))


def digest(data):
    return dict(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())


for name in ('build-timer.report', 'timer.native.json.reader.report', 'compile-timer.report'):
    assert read(root / f'{name}.json')['success']
assert all(result == 'PASS' for result in read(root / 'watchdog-selftest.json').values())
source_path = root / 'timer.native.json'
cases = {case['name']: case['frames'] for case in read(source_path)['character_animation_probe']['cases']}
assert set(cases) == set(labels)
native, target = {}, {}
for label in labels:
    repeated = label == 'double'
    functional = read(root / f'{label}-functional.json')
    assert functional['pass'] and functional['images'] == [] and not functional['issues']
    run = root / f'runtime-{label}'
    assert read(run / 'process.report.json')['success']
    native[label], target[label] = [], []
    for frame in range(39, 76):
        row = cases[label][frame]
        before, after = (row[f'movement_{phase}_tick'] for phase in ('before', 'after'))
        assert row['frame'] == frame and after['movement_mode_name'] == 'MOVE_Walking'
        expected = 3 if 40 <= frame <= 56 else .5 if repeated and 57 <= frame <= 73 else 0
        expected_before = 3 if 41 <= frame <= 57 else .5 if repeated and 58 <= frame <= 74 else 0
        assert after['braking_factor'] == expected
        assert before['braking_factor'] == expected_before
        landed = frame == 40 or repeated and frame == 57
        if landed:
            hit, = row['landed_events']
            assert hit['component'] == 'AuditLandingDeck' and not hit['penetrating']
            assert before['movement_mode_name'] == 'MOVE_Falling'
            assert before['velocity_cm_s'] == dict(x=0, y=0, z=-500)
        elif frame >= 40:
            assert not row['landed_events'] and before['movement_mode_name'] == 'MOVE_Walking'
        state, = [state for state in row['source_state_after_tick'] if state['name'] == 'LocomotionState']
        fields = {field['name']: field for field in state['fields']}
        has_input = fields['bHasInput']['exported_text'] == 'True'
        assert has_input == (frame == 56)
        native[label].append(dict(frame=frame, after=after['braking_factor'], before=before['braking_factor'],
                                  landed=landed, has_input=has_input))

        start = read(run / f'timer-before-{frame}.json')
        end = read(run / f'timer-after-{frame}.json')
        assert start['frame'] == end['frame'] == frame
        assert start['delta'] == end['delta']
        if frame >= 40:
            assert start['delta'] == row['world_delta_seconds']
        if frame >= 41:
            assert start['delta'] == .03125
        if landed:
            assert start['mode'] == 'Air' and start['velocity'] == [0, -5, 0]
        elif frame >= 40:
            assert start['mode'] == 'Grounded'
        assert end['mode'] == 'Grounded'
        assert start['factor'] == expected_before and end['factor'] == expected
        assert start['retained_input'] == (frame == 57)
        assert end['input'] == (frame == 56)
        remaining = .5 - (frame - 40) * .03125 if 40 <= frame <= 56 else (
            .5 - (frame - 57) * .03125 if repeated and 57 <= frame <= 73 else 0)
        assert end['remaining_seconds'] == remaining
        target[label].append(dict(before=start, after=end))

print('Verified both engines preserve the new timer on the old expiration frame', flush=True)
failed_visual = root / 'full-input-visual.json'
assert not read(root / 'full-input-single-functional.json')['pass']
assert any('local_translation_spike:pelvis' in fault for fault in read(failed_visual)['faults'])
low_input_visual = root / 'low-input-visual.json'
assert not read(root / 'low-input-double-functional.json')['pass']
assert any('local_translation_spike:pelvis' in fault for fault in read(low_input_visual)['faults'])
next_frame_path = root / 'next-frame.native.json'
late = next(case['frames'] for case in read(next_frame_path)['character_animation_probe']['cases']
            if case['name'] == 'double')
assert late[57]['landed_events'] == [] and late[57]['movement_after_tick']['braking_factor'] == 0
assert late[58]['landed_events'][0]['component'] == 'AuditLandingDeck'
assert late[58]['movement_after_tick']['braking_factor'] == 3
late_target = read(root / 'next-frame-target.json')['double']
assert late_target['after-57']['mode'] == 'Air' and late_target['after-57']['factor'] == 0
assert late_target['after-58']['mode'] == 'Grounded' and late_target['after-58']['factor'] == 3
paths = [source_path, next_frame_path, root / 'full-input.native.json', root / 'low-input.native.json',
         failed_visual, low_input_visual]
paths.extend(root / f'runtime-{label}' / f'{name}.json'
             for label in labels for name in ('parameters', 'report'))
decompressed = {}
for path in paths:
    data = raw(path)
    archive = path.with_suffix('.json.gz')
    if path.exists():
        archive.write_bytes(gzip.compress(data, compresslevel=6, mtime=0))
    assert gzip.decompress(archive.read_bytes()) == data
    decompressed[str(archive.relative_to(root))] = digest(data)
    path.unlink(missing_ok=True)

external = []
for name in ('570/impact.request.json', '570/prepare_reader.py', '570/observer.py', '523/native_initial.inc',
             '523/observer.py', '523/deck.rs', '522/contact.rs'):
    path = root.parent / name
    external.append(dict(path=str(path.relative_to(Path.cwd())), **digest(path.read_bytes())))
summary = dict(gate='PASS_LANDING_TIMER_REPLACES_EXPIRING_HANDLE', findings=[],
               new_differences=0, total_differences=310, native=native, target=target,
               decompressed_files=decompressed, external_source_files=external)
(root / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print('Verified compressed evidence identities', flush=True)
