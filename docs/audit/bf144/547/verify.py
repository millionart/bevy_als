import gzip
import hashlib
import json
import math
from pathlib import Path

root = Path(__file__).resolve().parent
labels = ('upper', 'lower')
walkable_y = math.cos(math.radians(44.765083))


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


def delta(start, end):
    return [b - a for a, b in zip(start, end, strict=True)]


def world(value):
    x, y, z = xyz(value)
    return [x / 100, 2 + z / 100, y / 100]


def event(case, kind):
    match, = [item for item in case['events'] if item['kind'] == kind]
    return match


def digest(data):
    return dict(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())


for name in ('build-edge.report', 'edge.native.json.reader.report', 'compile-edge.report'):
    assert read(root / f'{name}.json')['success']

request = read(root / 'edge.request.json')
source_file = root / 'edge.native.json'
frames = {case['name']: case['frames'][40] for case in read(source_file)['character_animation_probe']['cases']}
assert set(frames) == set(labels)
native = {}
target = {}
for label, case in zip(labels, request['cases'], strict=True):
    assert case['name'] == label
    upper = label == 'upper'
    assert case['step_ceiling_bottom_cm'] == (207 if upper else 204)
    frame = frames[label]
    assert frame['frame'] == 40 and frame['world_delta_seconds'] == .05000000074505806
    before, after = (frame[f'movement_{phase}_tick'] for phase in ('before', 'after'))
    assert before['movement_mode_name'] == after['movement_mode_name'] == 'MOVE_Walking'
    assert before['velocity_cm_s'] == {'x': 650, 'y': 0, 'z': 0}
    assert before['floor_blocking'] and before['walkable_floor'] and frame['mode_events'] == []
    assert before['floor_normal'] == {'x': 0, 'y': 0, 'z': 1}
    first, lift, edge, down = frame['blocking_hits']
    assert [hit['component'] for hit in (first, lift, edge, down)] == [
        'AuditStepFin', 'AuditStepCeiling', 'AuditStepEdge', 'AuditStepEdge']
    assert all(not hit['penetrating'] for hit in frame['blocking_hits'])
    assert all(0 < hit['time'] < 1 for hit in (first, lift, edge)) and 0 <= down['time'] < .01
    assert first['move_stop_cm'] == lift['trace_start_cm']
    assert lift['move_stop_cm'] == edge['trace_start_cm']
    assert edge['move_stop_cm'] == down['trace_start_cm']
    assert 30 < lift['move_stop_cm']['z'] - lift['trace_start_cm']['z'] < 35
    assert edge['normal']['z'] > 0 and (edge['normal']['z'] > walkable_y) == upper
    assert not edge['walkable_hit'] and xyz(edge['impact_normal']) == [-1, 0, 0]
    assert down['walkable_hit'] and xyz(down['impact_normal']) == [0, 0, 1]
    assert abs(down['impact_point_cm']['z'] - 43) < .0001
    assert math.dist(delta(xyz(down['trace_start_cm']), xyz(down['trace_end_cm'])),
                     [0, 0, -49.79999923706055]) < 1e-9
    down_start = world(down['trace_start_cm'])
    position = world(frame['actor_transform']['translation'])
    assert position[0] == down_start[0] and position[2] == down_start[2]
    native[label] = dict(hits=frame['blocking_hits'], down_start=down_start, position=position)

    functional = read(root / f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images'] == []
    assert load(label, 'process.report')['success']
    stage = load(label, 'stage-before')
    assert stage['mode'] == 'Grounded' and stage['substeps'] == 1 and stage['delta'] == frame['world_delta_seconds']
    assert stage['cached_world_velocity'] == [6.5, 0, 0]
    assert math.dist(stage['position'], world(first['trace_start_cm'])) < 1e-6
    assert math.dist(stage['planned_move'], [.3125, 0, 0]) < 1e-7
    fixture = load(label, 'step-edge')
    assert math.dist(fixture['wall'], [-13.55, 2.215, -15]) < 1e-6
    assert abs(fixture['ceiling_bottom'] - (2 + case['step_ceiling_bottom_cm'] / 100)) < 1e-6
    replay = load(label, 'step-down-replay')['variants']
    actual, control = (replay[k] for k in ('actual', 'with-edge-face'))
    for kind in ('forward', 'lift', 'step_forward'):
        assert event(actual, kind) == event(control, kind)
    assert event(actual, 'forward')['hit']['entity'] == 'AuditStepFin'
    assert event(actual, 'lift')['hit']['entity'] == 'AuditStepCeiling'
    approach = event(actual, 'step_forward')
    hit = approach['hit']
    assert hit['entity'] == 'AuditStepEdge'
    assert abs(hit['point'][0] + 13.59) < 1e-6 and abs(hit['point'][1] - 2.43) < 1e-6
    assert abs(hit['normal'][1] - edge['normal']['z']) < .004
    assert hit['normal'][1] > 0 and (hit['normal'][1] > walkable_y) == upper
    slide, guarded = (event(row, 'step_slide') for row in (actual, control))
    assert slide['start'] == guarded['start'] and slide['hit'] is guarded['hit'] is None
    assert guarded['normal_used'] == [-1, 0, 0] and math.hypot(*guarded['advance']) < 1e-7
    observed_down = event(actual, 'down_request')['start']
    controlled_down = event(control, 'down_request')['start']
    assert math.dist(controlled_down, down_start) < .00002
    assert event(actual, 'landing_check')['valid'] and event(control, 'landing_check')['valid']
    result = actual['result']
    txn = load(label, 'walking-transaction')
    assert txn['position'] == [result['advanced'][0], result['next_y'], result['advanced'][2]]
    after_target = load(label, 'contact-after-40')
    assert after_target['mode'] == txn['mode'] == 'Grounded' and after_target['transitions'] == []
    assert math.dist(after_target['position'][::2], observed_down[::2]) < 2e-6
    if upper:
        assert slide['normal_used'] == hit['normal']
        assert .074 < observed_down[0] - down_start[0] < .075
        assert .060 < observed_down[1] - down_start[1] < .061
    else:
        assert actual == control
    target[label] = dict(variants=replay, pre_physics=txn['position'], after=after_target,
                         actual_origin_error_m=math.dist(observed_down, down_start),
                         controlled_origin_error_m=math.dist(controlled_down, down_start))

assert native['upper']['hits'][0] == native['lower']['hits'][0]
assert load('upper', 'stage-before') == load('lower', 'stage-before')
print('Verified source side versus top face on the same edge and both target qualifications', flush=True)

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
    '545/overhang.request.json', '545/observer.py', '537/native_fin.inc', '537/target_fin.inc', '537/observer.py',
    '536/prepare_reader.py', '536/step_down_floor.inc', '536/observer.py', '536/step_probe.rs')]
external = [dict(path=str(p.relative_to(Path.cwd())), **digest(p.read_bytes())) for p in external_paths]
summary = dict(gate='PASS_STEPUP_GEOMETRIC_EDGE_FACE_QUALIFIES_POSITIVE_SLIDE', supplements=['BF144-D10'],
               new_differences=0, total_differences=299, ordinal=40, native=native, target=target,
               decompressed_files=decompressed, external_source_files=external)
(root / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print('Verified compressed evidence identities', flush=True)
