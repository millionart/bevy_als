import gzip
import hashlib
import json
import math
import struct
from pathlib import Path

root = Path(__file__).resolve().parent
labels = ('zero', 'micro', 'advance')
epsilon = 2 ** -23


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


for name in ('build-stop.report', 'stop.native.json.reader.report', 'compile-stop.report'):
    assert read(root / f'{name}.json')['success']

request = read(root / 'stop.request.json')
source_file = root / 'stop.native.json'
frames = {case['name']: case['frames'][40] for case in read(source_file)['character_animation_probe']['cases']}
assert set(frames) == set(labels)
native = {}
target = {}
for label, case in zip(labels, request['cases'], strict=True):
    assert case['name'] == label
    zero = label == 'zero'
    micro = label == 'micro'
    frame = frames[label]
    assert frame['frame'] == 40 and frame['world_delta_seconds'] == f32(.05)
    before, after = (frame[f'movement_{phase}_tick'] for phase in ('before', 'after'))
    assert before['movement_mode_name'] == after['movement_mode_name'] == 'MOVE_Walking'
    assert before['velocity_cm_s'] == {'x': 650, 'y': 0, 'z': 0}
    assert before['floor_blocking'] and before['walkable_floor'] and frame['mode_events'] == []
    hits = frame['blocking_hits']
    first = hits[0]
    assert first['component'] == 'AuditStepFin' and not first['walkable_hit']
    first_stop = first['move_stop_cm']
    first_request = first['trace_end_cm']['x'] - first['trace_start_cm']['x']
    distance = first_request * f32(1 - first['time'])
    pullback = f32(f32(1 / f32(distance)) + f32(.001))
    geometry_raw_x = -1500 + case['step_stop_front_cm'] - 30
    measured_advance = max(0, (geometry_raw_x - first_stop['x']) / distance - pullback) * distance
    position = world(frame['actor_transform']['translation'])
    down_start = None
    if zero:
        assert measured_advance == 0
        assert hits[1]['component'] == 'AuditStepStop'
        assert hits[1]['trace_start_cm'] == first_stop
        assert all(hit['trace_start_cm']['z'] < 100 for hit in hits)
        assert position[2] < -15.04
    else:
        _, forward, down = hits
        assert forward['component'] == 'AuditStepStop' and down['component'] == 'AuditStepFin'
        assert not forward['penetrating'] and 0 < forward['time'] < .01
        assert xyz(forward['normal']) == xyz(forward['impact_normal']) == [-1, 0, 0]
        assert abs(forward['location_cm']['x'] - geometry_raw_x) < 1e-5
        assert forward['trace_start_cm']['x'] == first_stop['x']
        assert 42.8 < forward['trace_start_cm']['z'] - first_stop['z'] < 42.9
        observed_advance = forward['move_stop_cm']['x'] - forward['trace_start_cm']['x']
        assert abs(observed_advance - measured_advance) < .00001
        assert observed_advance > 0
        assert (observed_advance / 100 < math.sqrt(epsilon)) == micro
        assert down['trace_start_cm'] == forward['move_stop_cm']
        assert down['walkable_hit'] and not down['penetrating']
        down_start = world(down['trace_start_cm'])
        assert position[0] == down_start[0] and position[2] == down_start[2]
    native[label] = dict(hits=hits, position=position, down_start=down_start, geometry_safe_advance_cm=measured_advance)

    functional = read(root / f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images'] == []
    assert load(label, 'process.report')['success']
    stage = load(label, 'stage-before')
    assert stage['mode'] == 'Grounded' and stage['substeps'] == 1 and stage['delta'] == frame['world_delta_seconds']
    assert stage['cached_world_velocity'] == [6.5, 0, 0]
    assert math.dist(stage['position'], world(first['trace_start_cm'])) < 1e-6
    assert math.dist(stage['planned_move'], [.3125, 0, 0]) < 1e-7
    fixture = load(label, 'step-stop')
    assert abs(fixture['front'] - case['step_stop_front_cm'] / 100) < 1e-6
    replay = load(label, 'step-down-replay')['variants']
    actual, control = (replay[k] for k in ('actual', 'with-axis-progress'))
    for kind in ('forward', 'lift', 'step_forward'):
        assert event(actual, kind) == event(control, kind)
    assert event(actual, 'forward')['hit']['entity'] == 'AuditStepFin'
    assert event(actual, 'lift')['hit'] is None
    approach = event(actual, 'step_forward')
    hit = approach['hit']
    assert hit['entity'] == 'AuditStepStop' and hit['distance_m'] > 0
    assert math.dist(hit['normal'], [-1, 0, 0]) < .00005
    measured, selected = (event(row, 'step_progress') for row in (actual, control))
    assert {k: v for k, v in measured.items() if k != 'rejected'} == {
        k: v for k, v in selected.items() if k != 'rejected'}
    assert math.dist(measured['advance'], measured['total']) < .00002
    assert abs(measured['advance'][0] - measured_advance / 100) < .00002
    assert measured['rejected'] == (zero or micro)
    assert selected['rejected'] == zero
    assert (measured['length_sq'] <= epsilon) == (zero or micro)
    assert (measured['advance'][0] == 0) == zero
    if not zero:
        controlled_down = event(control, 'down_request')['start']
        assert math.dist(controlled_down, down_start) < .0003
        assert event(control, 'landing_check')['valid'] and control['result']['next_y'] is not None
    if micro:
        assert actual['result']['next_y'] is None
        assert not any(e['kind'] == 'down_request' for e in actual['events'])
    else:
        assert actual == control
    result = actual['result']
    txn = load(label, 'walking-transaction')
    expected = result['advanced'].copy()
    if result['next_y'] is not None:
        expected[1] = result['next_y']
    assert txn['position'] == expected
    after_target = load(label, 'contact-after-40')
    assert after_target['mode'] == txn['mode'] == 'Grounded' and after_target['transitions'] == []
    target[label] = dict(variants=replay, pre_physics=txn['position'], after=after_target)

assert native['zero']['hits'][0] == native['micro']['hits'][0] == native['advance']['hits'][0]
assert load('zero', 'stage-before') == load('micro', 'stage-before') == load('advance', 'stage-before')
print('Verified the zero and above-threshold controls and the omitted positive StepDown', flush=True)

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
    '547/edge.request.json', '545/observer.py', '537/native_fin.inc', '537/target_fin.inc', '537/observer.py',
    '536/prepare_reader.py', '536/step_down_floor.inc', '536/observer.py', '536/step_probe.rs')]
external = [dict(path=str(p.relative_to(Path.cwd())), **digest(p.read_bytes())) for p in external_paths]
summary = dict(gate='PASS_STEPUP_POSITIVE_SUB_EPSILON_FORWARD_WITNESS', findings=['BF144-D303'],
               new_differences=1, total_differences=300, ordinal=40, native=native, target=target,
               decompressed_files=decompressed, external_source_files=external)
(root / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print('Verified compressed evidence identities', flush=True)
