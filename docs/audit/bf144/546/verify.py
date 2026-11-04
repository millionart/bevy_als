import gzip
import hashlib
import json
import math
import struct
from pathlib import Path

root = Path(__file__).resolve().parent
labels = ('opposed', 'following')


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


def dot(left, right):
    return sum(a * b for a, b in zip(left, right, strict=True))


def sweep(hit):
    return delta(xyz(hit['trace_start_cm']), xyz(hit['trace_end_cm']))


def f32(value):
    return struct.unpack('<f', struct.pack('<f', value))[0]


def project(move, normal, amount=1):
    component = dot(move, normal)
    return [(value - n * component) * amount for value, n in zip(move, normal, strict=True)]


def flatten_ue(normal):
    length = math.hypot(*normal[:2])
    return [normal[0] / length, normal[1] / length, 0.0]


def bevy_direction(v):
    return [v[0], v[2], v[1]]


def world(value):
    x, y, z = bevy_direction(xyz(value))
    return [x / 100, 2 + y / 100, z / 100]


def xz(vec):
    return vec[0], vec[2]


def event(case, kind):
    match, = [item for item in case['events'] if item['kind'] == kind]
    return match


def digest(data):
    return dict(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())


for name in ('build-floor.report', 'floor.native.json.reader.report', 'compile-floor.report'):
    assert read(root / f'{name}.json')['success']

request = read(root / 'floor.request.json')
source_file = root / 'floor.native.json'
frames = {case['name']: case['frames'][40] for case in read(source_file)['character_animation_probe']['cases']}
assert set(frames) == set(labels)
native = {}
target = {}
for label, case in zip(labels, request['cases'], strict=True):
    assert case['name'] == label
    opposed = label == 'opposed'
    assert case['step_floor_pitch'] == (4 if opposed else -4)
    frame = frames[label]
    assert frame['frame'] == 40 and frame['world_delta_seconds'] == f32(.05)
    before, after = (frame[f'movement_{phase}_tick'] for phase in ('before', 'after'))
    assert before['movement_mode_name'] == after['movement_mode_name'] == 'MOVE_Walking'
    assert before['velocity_cm_s'] == {'x': 650, 'y': 0, 'z': 0}
    assert before['walkable_floor'] and before['floor_blocking'] and frame['mode_events'] == []
    assert abs(before['floor_distance_cm'] - 1.8) < .0001 and before['floor_distance_cm'] < 1.9
    floor_normal = xyz(before['floor_normal'])
    angle = math.radians(case['step_floor_pitch'])
    yaw = math.radians(25)
    floor_geometry = [-math.sin(angle) * math.cos(yaw), math.sin(angle) * math.sin(yaw), math.cos(angle)]
    assert math.dist(floor_normal, floor_geometry) < 1e-7
    assert floor_normal[2] < 1 - 1e-5
    first, forward, down = frame['blocking_hits']
    assert first['component'] == down['component'] == 'AuditStepFin'
    assert forward['component'] == 'AuditStepOverhang' and not forward['walkable_hit']
    assert all(not hit['penetrating'] and 0 < hit['time'] < 1 for hit in frame['blocking_hits'])
    assert first['move_stop_cm']['x'] == forward['trace_start_cm']['x']
    assert math.dist(sweep(down), [0, 0, -49.79999923706055]) < 1e-9
    first_request = sweep(first)
    assert abs(first_request[0] - 31.25) < 1e-6
    assert abs(first_request[2] + f32(floor_normal[0] * first_request[0]) / floor_normal[2]) < 1e-5
    wall_normal = xyz(forward['normal'])
    assert abs(wall_normal[2] + .5) < 1e-6
    requested = sweep(forward)
    assert (dot(requested, floor_normal) < 0) == opposed
    fraction = f32(1 - forward['time'])
    wall_only = project(requested, flatten_ue(wall_normal), fraction)
    selected = flatten_ue(floor_normal if opposed else wall_normal)
    observed = delta(xyz(forward['move_stop_cm']), xyz(down['trace_start_cm']))
    assert math.dist(project(requested, selected, fraction), observed) < 1e-9
    if opposed:
        assert 7.3 < math.dist(observed, wall_only) < 7.5
    else:
        assert math.dist(observed, wall_only) < 1e-9
    down_start = world(down['trace_start_cm'])
    position = world(frame['actor_transform']['translation'])
    assert xz(down_start) == xz(position)
    native[label] = dict(floor_before=before, hits=frame['blocking_hits'], first_request_cm=first_request,
                         slide_cm=observed, wall_only_slide_cm=wall_only, down_start=down_start, position=position)

    functional = read(root / f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images'] == []
    assert load(label, 'process.report')['success']
    stage = load(label, 'stage-before')
    assert stage['mode'] == 'Grounded' and stage['substeps'] == 1 and stage['delta'] == frame['world_delta_seconds']
    assert stage['cached_world_velocity'] == [6.5, 0, 0]
    assert math.dist(stage['position'], world(first['trace_start_cm'])) < 1e-6
    assert math.dist(stage['planned_move'], [.3125, 0, 0]) < 1e-7
    fixture_floor = load(label, 'old-floor')
    assert fixture_floor['pitch'] == case['step_floor_pitch']
    assert math.dist(fixture_floor['normal'], bevy_direction(floor_normal)) < 1e-6
    floor = load(label, 'walking-step')['start_floor']['variants']['none']['result']
    assert floor['entity'] == 'AuditWalkOffPlatform'
    replay = load(label, 'step-down-replay')['variants']
    actual, wall, control = (replay[k] for k in ('actual', 'wall-only', 'floor-aware'))
    for kind in ('forward', 'lift', 'step_forward'):
        assert event(actual, kind) == event(wall, kind) == event(control, kind)
    assert event(actual, 'forward')['hit']['entity'] == 'AuditStepFin'
    assert event(actual, 'lift')['hit'] is None
    approach = event(actual, 'step_forward')
    assert .0175 < approach['floor_gap_m'] < .019
    assert approach['hit']['entity'] == 'AuditStepOverhang'
    old_floor = approach['old_floor']
    assert old_floor['entity'] == floor['entity']
    assert math.dist(old_floor['normal'], bevy_direction(floor_normal)) < .0002
    assert (dot(approach['requested'], old_floor['normal']) < 0) == opposed
    slide, wall_slide, guarded = (event(row, 'step_slide') for row in (actual, wall, control))
    assert slide['start'] == wall_slide['start'] == guarded['start']
    assert slide['hit'] is wall_slide['hit'] is guarded['hit'] is None
    assert -.066 < slide['advance'][1] < -.064
    assert wall_slide['advance'][1] == guarded['advance'][1] == 0
    assert not wall_slide['floor_selected'] and guarded['floor_selected'] == opposed
    target_tail = delta(approach['advance'], approach['requested'])
    same_tail = project(target_tail, bevy_direction(selected))
    assert math.dist(guarded['advance'], same_tail) < .00003
    if opposed:
        assert .073 < math.dist(wall_slide['advance'], guarded['advance']) < .074
    else:
        assert wall == control
    observed_down = event(actual, 'down_request')['start']
    controlled_down = event(control, 'down_request')['start']
    wall_down = event(wall, 'down_request')['start']
    assert math.dist(xz(controlled_down), xz(down_start)) < .003
    assert math.dist(controlled_down, down_start) < .005
    assert all(event(row, 'landing_check')['valid'] for row in (actual, wall, control))
    result = actual['result']
    txn = load(label, 'walking-transaction')
    assert txn['position'] == [result['advanced'][0], result['next_y'], result['advanced'][2]]
    after_target = load(label, 'contact-after-40')
    assert after_target['mode'] == txn['mode'] == 'Grounded' and after_target['transitions'] == []
    assert math.dist(xz(after_target['position']), xz(observed_down)) < 2e-6
    target[label] = dict(variants=replay, pre_physics=txn['position'], after=after_target,
                         normalized_source_projection_error_m=math.dist(guarded['advance'], same_tail),
                         actual_origin_xz_error_m=math.dist(xz(observed_down), xz(down_start)),
                         wall_only_origin_xz_error_m=math.dist(xz(wall_down), xz(down_start)),
                         floor_aware_origin_xz_error_m=math.dist(xz(controlled_down), xz(down_start)))

assert load('opposed', 'step-overhang') == load('following', 'step-overhang')
print('Verified source old-floor substitution and both same-contact counterfactuals', flush=True)

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
    '545/overhang.request.json', '545/native_overhang.inc', '545/target_overhang.inc', '545/observer.py',
    '537/native_fin.inc', '537/target_fin.inc', '537/observer.py',
    '536/prepare_reader.py', '536/step_down_floor.inc', '536/observer.py', '536/step_probe.rs')]
external = [dict(path=str(p.relative_to(Path.cwd())), **digest(p.read_bytes())) for p in external_paths]
summary = dict(gate='PASS_STEPUP_OPPOSING_FLOOR_NORMAL_SUBSTITUTION_WITNESS', findings=['BF144-D302'],
               new_differences=1, total_differences=299, ordinal=40, native=native, target=target,
               decompressed_files=decompressed, external_source_files=external)
(root / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print('Verified compressed evidence identities', flush=True)
