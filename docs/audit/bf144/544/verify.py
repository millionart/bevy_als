import gzip
import hashlib
import json
import math
import struct
from pathlib import Path

root = Path(__file__).resolve().parent
labels = ('one-wall', 'two-walls')


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


def add(left, right):
    return [a + b for a, b in zip(left, right, strict=True)]


def dot(left, right):
    return sum(a * b for a, b in zip(left, right, strict=True))


def sweep(hit):
    return delta(xyz(hit['trace_start_cm']), xyz(hit['trace_end_cm']))


def slide(delta, hit):
    normal = xyz(hit['normal'])
    assert normal[2] == 0 and not hit['walkable_hit']
    projected = [v - n * dot(delta, normal) for v, n in zip(delta, normal, strict=True)]
    remain = struct.unpack('<f', struct.pack('<f', 1 - hit['time']))[0]
    return [v * remain for v in projected]


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


for name in ('build-corner.report', 'corner.native.json.reader.report', 'compile-corner.report'):
    assert read(root / f'{name}.json')['success']

request = read(root / 'corner.request.json')
source_file = root / 'corner.native.json'
frames = {case['name']: case['frames'][40] for case in read(source_file)['character_animation_probe']['cases']}
assert set(frames) == set(labels)
native = {}
target = {}
for label, case in zip(labels, request['cases'], strict=True):
    assert case['name'] == label
    two = label == 'two-walls'
    assert case['step_corner_second'] == two and case['step_fin']
    frame = frames[label]
    assert frame['frame'] == 40 and frame['world_delta_seconds'] == .05000000074505806
    before, after = (frame[f'movement_{phase}_tick'] for phase in ('before', 'after'))
    assert before['movement_mode_name'] == after['movement_mode_name'] == 'MOVE_Walking'
    assert before['velocity_cm_s'] == {'x': 650, 'y': 0, 'z': 0}
    assert before['walkable_floor'] and after['walkable_floor'] and frame['mode_events'] == []
    first, forward, *between, down = frame['blocking_hits']
    assert first['component'] == down['component'] == 'AuditStepFin'
    assert not first['walkable_hit'] and down['walkable_hit']
    assert all(not hit['penetrating'] and 0 < hit['time'] < 1 for hit in frame['blocking_hits'])
    assert forward['component'] == 'AuditStepWallFirst'
    assert abs(sweep(forward)[0] - 25.6574753769855) < 1e-9
    assert first['move_stop_cm']['x'] == forward['trace_start_cm']['x']
    assert 42.8 < forward['trace_start_cm']['z'] - first['move_stop_cm']['z'] < 42.9
    first_slide = slide(sweep(forward), forward)
    end = world(frame['actor_transform']['translation'])
    down_start = world(down['trace_start_cm'])
    assert math.dist(sweep(down), [0, 0, -49.79999923706055]) < 1e-9
    assert xz(down_start) == xz(end)
    if two:
        second, = between
        assert second['component'] == 'AuditStepWallSecond'
        assert second['trace_start_cm'] == forward['move_stop_cm']
        assert math.dist(first_slide, sweep(second)) < 1e-9
        assert 0 < dot(xyz(forward['normal']), xyz(second['normal'])) < .99
        second_slide = slide(first_slide, second)
        assert dot(second_slide, sweep(forward)) > 0
        continued = delta(xyz(second['move_stop_cm']), xyz(down['trace_start_cm']))
        assert math.dist(continued, second_slide) < 1e-9
    else:
        assert between == []
        assert math.dist(first_slide, delta(xyz(forward['move_stop_cm']), xyz(down['trace_start_cm']))) < 1e-9
        second_slide = [0, 0, 0]
    native[label] = dict(hits=frame['blocking_hits'], position=end, mode=after['movement_mode_name'],
                         down_start=down_start, second_slide_cm=second_slide)

    functional = read(root / f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images'] == []
    assert load(label, 'process.report')['success']
    fixture = load(label, 'fixture')
    assert fixture['half_width_cm'] == case['floor_half_extent_x_cm'] == 200
    walls = load(label, 'step-walls')
    assert len(walls) == (2 if two else 1)
    for wall, threshold, yaw in zip(walls, [1.13, 1.16][:len(walls)], [45, 20][:len(walls)], strict=True):
        n = [-math.cos(math.radians(yaw)), 0, math.sin(math.radians(yaw))]
        assert math.dist(wall['front_normal'], n) < 1e-6
        center = [-15 + threshold - .32 * n[0], 3.5, -15 - .32 * n[2]]
        assert math.dist(wall['center'], center) < 2e-6

    stage = load(label, 'stage-before')
    assert stage['mode'] == 'Grounded' and stage['substeps'] == 1 and stage['delta'] == frame['world_delta_seconds']
    assert stage['cached_world_velocity'] == [6.5, 0, 0]
    assert math.dist(stage['position'], world(first['trace_start_cm'])) < 1e-6
    assert math.dist(stage['planned_move'], [.3125, 0, 0]) < 1e-7
    replay = load(label, 'step-down-replay')
    actual, control = (replay['variants'][key] for key in ('actual', 'with-second-slide'))
    assert replay['start'] == stage['position'] and replay['requested'] == stage['planned_move']
    prefix = ('forward', 'lift', 'step_forward', 'step_slide')
    for kind in prefix:
        assert event(actual, kind) == event(control, kind)
    assert event(actual, 'forward')['hit']['entity'] == 'AuditStepFin'
    assert event(actual, 'lift')['hit'] is None
    assert event(actual, 'step_forward')['hit']['entity'] == 'AuditStepWallFirst'
    observed_slide = event(actual, 'step_slide')
    observed_down = event(actual, 'down_request')['start']
    controlled_down = event(control, 'down_request')['start']
    assert math.dist(add(observed_slide['start'], observed_slide['advance']), observed_down) < 2e-6
    result = actual['result']
    assert event(actual, 'landing_check')['valid'] and event(control, 'landing_check')['valid']
    txn = load(label, 'walking-transaction')
    assert txn['position'] == [result['advanced'][0], result['next_y'], result['advanced'][2]]
    after = load(label, 'contact-after-40')
    assert after['mode'] == txn['mode'] == 'Grounded' and after['transitions'] == []
    assert math.dist(xz(after['position']), xz(observed_down)) < 2e-6
    if two:
        assert observed_slide['hit']['entity'] == 'AuditStepWallSecond'
        assert math.dist(xz(observed_down), xz(world(second['move_stop_cm']))) < .00015
        extra = event(control, 'step_second_slide')
        assert extra['start'] == observed_down and extra['hit'] is None
        assert math.dist(add(extra['start'], extra['advance']), controlled_down) < 2e-6
        assert .064 < math.dist(xz(observed_down), xz(down_start)) < .065
        assert math.dist(xz(controlled_down), xz(down_start)) < .00003
        assert control['result']['next_y'] - result['next_y'] > .04
    else:
        assert observed_slide['hit'] is None and actual == control
        assert math.dist(xz(observed_down), xz(down_start)) < .00006
    target[label] = dict(actual=actual, read_only_second_slide=control,
                         pre_physics=txn['position'], after=after)

assert native['one-wall']['hits'][:2] == native['two-walls']['hits'][:2]
assert load('one-wall', 'stage-before') == load('two-walls', 'stage-before')
print('Verified native StepDown origin, production first-sweep stop and read-only second-sweep counterfactual', flush=True)

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
summary = dict(gate='PASS_STEPUP_INTERNAL_TWO_WALL_CONTINUATION_WITNESS', findings=['BF144-D300'],
               new_differences=1, total_differences=297, ordinal=40, native=native, target=target,
               decompressed_files=decompressed, external_source_files=external)
(root / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print('Verified compressed evidence identities', flush=True)
