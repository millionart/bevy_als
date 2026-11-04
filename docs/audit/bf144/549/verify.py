import gzip
import hashlib
import json
import math
import struct
from pathlib import Path

root = Path(__file__).resolve().parent
labels = ('lower', 'near-limit', 'tight-limit')


def raw(path):
    if not path.exists():
        return gzip.decompress(path.with_suffix('.json.gz').read_bytes())
    return path.read_bytes()


def read(path):
    return json.loads(raw(path))


def load(label, name):
    return read(root / f'runtime-{label}' / f'{name}.json')


def xyz(value):
    return [value[k] for k in 'xyz']


def world(value):
    x, y, z = xyz(value)
    return [x / 100, 2 + z / 100, y / 100]


def f32(value):
    return struct.unpack('<f', struct.pack('<f', value))[0]


def event(case, kind):
    match, = [item for item in case['events'] if item['kind'] == kind]
    return match


def digest(data):
    return dict(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())


for name in ('build-height.report', 'height.native.json.reader.report', 'compile-height.report'):
    assert read(root / f'{name}.json')['success']

request = read(root / 'height.request.json')
source_file = root / 'height.native.json'
frames = {case['name']: case['frames'][40] for case in read(source_file)['character_animation_probe']['cases']}
assert set(frames) == set(labels)
native = {}
target = {}
for label, case in zip(labels, request['cases'], strict=True):
    assert case['name'] == label
    tight = label == 'tight-limit'
    lower = label == 'lower'
    frame = frames[label]
    assert frame['frame'] == 40 and frame['world_delta_seconds'] == f32(.05)
    before, after = (frame[f'movement_{phase}_tick'] for phase in ('before', 'after'))
    assert before['movement_mode_name'] == after['movement_mode_name'] == 'MOVE_Walking'
    assert before['velocity_cm_s'] == {'x': 650, 'y': 0, 'z': 0}
    assert before['floor_blocking'] and before['walkable_floor'] and frame['mode_events'] == []
    assert before['floor_normal'] == {'x': 0, 'y': 0, 'z': 1}
    assert abs(before['floor_distance_cm'] - (.3 if tight else 2.15)) < .0001
    first, down = frame['blocking_hits']
    assert first['component'] == down['component'] == 'AuditStepLimit'
    assert not first['penetrating'] and not down['penetrating']
    assert xyz(first['impact_normal']) == [-1, 0, 0] and not first['walkable_hit']
    assert xyz(down['impact_normal']) == [0, 0, 1] and down['walkable_hit']
    old_center = first['trace_start_cm']['z']
    old_base = old_center - 90 - before['floor_distance_cm']
    impact_height = down['impact_point_cm']['z'] - old_base
    assert abs(old_base) < .0001 and abs(impact_height - case['step_limit_height_cm']) < .0001
    assert impact_height < 45
    assert abs(down['trace_start_cm']['z'] - 135) < .0001
    assert abs(down['trace_start_cm']['x'] - first['trace_end_cm']['x']) < 1e-6
    assert down['time'] == 0 and down['trace_start_cm'] == down['move_stop_cm']
    down_center_delta = down['move_stop_cm']['z'] - old_center
    final_center_delta = frame['actor_transform']['translation']['z'] - old_center
    assert down_center_delta < 45
    assert (final_center_delta > 45) == tight
    assert frame['actor_transform']['translation']['x'] == down['move_stop_cm']['x']
    native[label] = dict(hits=frame['blocking_hits'], old_floor=before,
                         impact_height_cm=impact_height, down_center_delta_cm=down_center_delta,
                         final_center_delta_cm=final_center_delta,
                         position=world(frame['actor_transform']['translation']))

    functional = read(root / f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images'] == []
    assert load(label, 'process.report')['success']
    stage = load(label, 'stage-before')
    assert stage['mode'] == 'Grounded' and stage['substeps'] == 1 and stage['delta'] == frame['world_delta_seconds']
    assert stage['cached_world_velocity'] == [6.5, 0, 0]
    assert math.dist(stage['position'], world(first['trace_start_cm'])) < 1e-6
    assert math.dist(stage['planned_move'], [.3125, 0, 0]) < 1e-7
    fixture = load(label, 'step-limit')
    assert abs(fixture['height'] - case['step_limit_height_cm'] / 100) < 1e-6
    assert abs(fixture['center'][0] + 13.4) < 1e-6
    replay = load(label, 'step-down-replay')['variants']
    actual, control = (replay[k] for k in ('actual', 'without-center-height'))
    assert actual['events'][:-1] == control['events'][:-1]
    assert event(actual, 'forward')['hit']['entity'] == 'AuditStepLimit'
    assert event(actual, 'lift')['hit'] is None
    assert not any(e['kind'] == 'step_forward' for e in actual['events'])
    observed_down = event(actual, 'down_request')['start']
    assert math.dist(observed_down, world(down['trace_start_cm'])) < .0003
    landing = event(actual, 'landing_check')
    assert landing['valid'] and landing['down_hit']['entity'] == 'AuditStepLimit'
    measured, selected = (event(row, 'height_guard') for row in (actual, control))
    assert {k: v for k, v in measured.items() if k != 'rejected'} == {
        k: v for k, v in selected.items() if k != 'rejected'}
    assert abs(measured['impact_delta'] - impact_height / 100) < 1e-6
    assert not measured['impact_exceeds'] and not selected['rejected']
    assert measured['center_exceeds'] == measured['rejected'] == (not lower)
    assert abs(measured['center_delta'] - (landing['position'][1] - stage['position'][1])) < 1e-7
    assert control['result']['next_y'] == landing['position'][1]
    if tight:
        assert measured['center_delta'] - .0025 > .45
    if lower:
        assert actual == control
    else:
        assert actual['result']['next_y'] is None
    result = actual['result']
    txn = load(label, 'walking-transaction')
    expected = result['advanced'].copy()
    if result['next_y'] is not None:
        expected[1] = result['next_y']
    assert txn['position'] == expected
    after_target = load(label, 'contact-after-40')
    assert after_target['mode'] == txn['mode'] == 'Grounded' and after_target['transitions'] == []
    if not lower:
        assert native[label]['position'][0] - after_target['position'][0] > .29
    target[label] = dict(variants=replay, pre_physics=txn['position'], after=after_target)

assert load('lower', 'stage-before') == load('near-limit', 'stage-before')
print('Verified source impact height, actual pulled-back and final heights, and target center guard control', flush=True)

paths = [source_file]
paths.extend(root / f'runtime-{label}' / f'{name}.json' for label in labels for name in (
    'parameters', 'report', 'walking-step', 'walking-transaction', 'step-down-replay'))
decompressed = {}
for path in paths:
    data = raw(path)
    archive = path.with_suffix('.json.gz')
    if path.exists():
        archive.write_bytes(gzip.compress(data, compresslevel=6, mtime=0))
    assert gzip.decompress(archive.read_bytes()) == data
    decompressed[str(archive.relative_to(root))] = digest(data)
    path.unlink(missing_ok=True)

external_paths = [root.parent / name for name in (
    '548/stop.request.json', '545/observer.py', '537/observer.py',
    '536/prepare_reader.py', '536/step_down_floor.inc', '536/observer.py', '536/step_probe.rs')]
external = [dict(path=str(p.relative_to(Path.cwd())), **digest(p.read_bytes())) for p in external_paths]
summary = dict(gate='PASS_STEPUP_IMPACT_HEIGHT_SEPARATE_FROM_CENTER_HEIGHT', findings=['BF144-D304'],
               new_differences=1, total_differences=301, ordinal=40, native=native, target=target,
               decompressed_files=decompressed, external_source_files=external)
(root / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print('Verified compressed evidence identities', flush=True)
