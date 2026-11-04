import gzip
import hashlib
import json
import math
import struct
from pathlib import Path

root = Path(__file__).resolve().parent
labels = ('tight', 'open')


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


def project(move, normal, amount):
    component = dot(move, normal)
    return [(value - n * component) * amount for value, n in zip(move, normal, strict=True)]


def world(value):
    x, y, z = xyz(value)
    return [x / 100, 2 + z / 100, y / 100]


def xz(vec):
    return vec[0], vec[2]


def event(case, kind):
    match, = [item for item in case['events'] if item['kind'] == kind]
    return match


def digest(data):
    return dict(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())


for name in ('build-overhang.report', 'overhang.native.json.reader.report', 'compile-overhang.report'):
    assert read(root / f'{name}.json')['success']
assert read(root / 'build-shadow-failure.report.json')['process_exit_code'] == 6

request = read(root / 'overhang.request.json')
source_file = root / 'overhang.native.json'
frames = {case['name']: case['frames'][40] for case in read(source_file)['character_animation_probe']['cases']}
assert set(frames) == set(labels)
native = {}
target = {}
for label, case in zip(labels, request['cases'], strict=True):
    assert case['name'] == label
    tight = label == 'tight'
    frame = frames[label]
    assert frame['frame'] == 40 and frame['world_delta_seconds'] == .05000000074505806
    before, after = (frame[f'movement_{phase}_tick'] for phase in ('before', 'after'))
    assert before['movement_mode_name'] == after['movement_mode_name'] == 'MOVE_Walking'
    assert before['velocity_cm_s'] == {'x': 650, 'y': 0, 'z': 0}
    assert before['walkable_floor'] and before['floor_blocking'] and frame['mode_events'] == []
    assert before['floor_normal'] == {'x': 0, 'y': 0, 'z': 1}
    assert abs(before['floor_distance_cm'] - (1.8 if tight else 2.15)) < .001
    assert (before['floor_distance_cm'] < 1.9) == tight
    first, forward, down = frame['blocking_hits']
    assert first['component'] == down['component'] == 'AuditStepFin'
    assert forward['component'] == 'AuditStepOverhang' and not forward['walkable_hit']
    assert all(not hit['penetrating'] and 0 < hit['time'] < 1 for hit in frame['blocking_hits'])
    assert first['move_stop_cm']['x'] == forward['trace_start_cm']['x']
    assert math.dist(sweep(down), [0, 0, -49.79999923706055]) < 1e-9
    raw_normal = xyz(forward['normal'])
    assert abs(raw_normal[2] + .5) < 1e-6
    planar_length = math.hypot(*raw_normal[:2])
    flat = [n / planar_length for n in raw_normal[:2]] + [0]
    fraction = struct.unpack('<f', struct.pack('<f', 1 - forward['time']))[0]
    requested = sweep(forward)
    selected = project(requested, flat if tight else raw_normal, fraction)
    observed = delta(xyz(forward['move_stop_cm']), xyz(down['trace_start_cm']))
    assert math.dist(selected, observed) < 1e-9
    assert (observed[2] == 0) if tight else (-6.6 < observed[2] < -6.4)
    down_start = world(down['trace_start_cm'])
    position = world(frame['actor_transform']['translation'])
    assert xz(down_start) == xz(position)
    native[label] = dict(floor_before=before, hits=frame['blocking_hits'],
                         slide_cm=observed, down_start=down_start, position=position)

    functional = read(root / f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images'] == []
    assert load(label, 'process.report')['success']
    stage = load(label, 'stage-before')
    assert stage['mode'] == 'Grounded' and stage['substeps'] == 1 and stage['delta'] == frame['world_delta_seconds']
    assert stage['cached_world_velocity'] == [6.5, 0, 0]
    assert math.dist(stage['position'], world(first['trace_start_cm'])) < 1e-6
    assert math.dist(stage['planned_move'], [.3125, 0, 0]) < 1e-7
    floor = load(label, 'walking-step')['start_floor']['variants']['none']['result']
    assert floor['entity'] == 'AuditWalkOffPlatform'
    replay = load(label, 'step-down-replay')['variants']
    actual, control = (replay[k] for k in ('actual', 'with-near-floor-guard'))
    for kind in ('forward', 'lift', 'step_forward'):
        assert event(actual, kind) == event(control, kind)
    assert event(actual, 'forward')['hit']['entity'] == 'AuditStepFin'
    assert event(actual, 'lift')['hit'] is None
    approach = event(actual, 'step_forward')
    assert (approach['floor_gap_m'] < .019) == tight
    assert abs(approach['floor_gap_m'] - before['floor_distance_cm'] / 100) < .0003
    assert approach['hit']['entity'] == 'AuditStepOverhang'
    slide = event(actual, 'step_slide')
    guarded = event(control, 'step_slide')
    assert slide['start'] == guarded['start'] and slide['hit'] is guarded['hit'] is None
    assert math.dist(slide['normal_used'], [raw_normal[0], raw_normal[2], raw_normal[1]]) < .0002
    assert -.066 < slide['advance'][1] < -.064
    observed_down = event(actual, 'down_request')['start']
    controlled_down = event(control, 'down_request')['start']
    assert math.dist(controlled_down, down_start) < .001
    assert math.dist(xz(controlled_down), xz(down_start)) < .0003
    assert event(actual, 'landing_check')['valid'] and event(control, 'landing_check')['valid']
    result = actual['result']
    txn = load(label, 'walking-transaction')
    assert txn['position'] == [result['advanced'][0], result['next_y'], result['advanced'][2]]
    after_target = load(label, 'contact-after-40')
    assert after_target['mode'] == txn['mode'] == 'Grounded' and after_target['transitions'] == []
    assert math.dist(xz(after_target['position']), xz(observed_down)) < 2e-6
    if tight:
        assert guarded['normal_used'][1] == guarded['advance'][1] == 0
        assert .037 < math.dist(xz(observed_down), xz(down_start)) < .039
        assert .064 < down_start[1] - observed_down[1] < .066
    else:
        assert actual == control
    target[label] = dict(actual=actual, read_only_near_floor_guard=control,
                         down_origin_xz_error_m=math.dist(xz(observed_down), xz(down_start)),
                         controlled_origin_xz_error_m=math.dist(xz(controlled_down), xz(down_start)),
                         pre_physics=txn['position'], after=after_target)

assert load('tight', 'step-overhang') == load('open', 'step-overhang')
print('Verified source tight-floor normal guard and its target StepDown consumer', flush=True)

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
    '537/wall.request.json', '537/native_fin.inc', '537/target_fin.inc', '537/observer.py',
    '536/prepare_reader.py', '536/step_down_floor.inc', '536/observer.py', '536/step_probe.rs')]
external = [dict(path=str(p.relative_to(Path.cwd())), **digest(p.read_bytes())) for p in external_paths]
summary = dict(gate='PASS_STEPUP_NEGATIVE_NORMAL_TIGHT_FLOOR_WITNESS', findings=['BF144-D301'],
               new_differences=1, total_differences=298, ordinal=40, native=native, target=target,
               decompressed_files=decompressed, external_source_files=external)
(root / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print('Verified compressed evidence identities', flush=True)
