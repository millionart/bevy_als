import gzip
import hashlib
import json
import math
import struct
from pathlib import Path

root = Path(__file__).resolve().parent
labels = ('ramp-no-step', 'ramp-step', 'steep-no-step')


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


def delta(hit):
    return [b - a for a, b in zip(xyz(hit['trace_start_cm']), xyz(hit['trace_end_cm']), strict=True)]


def world(value):
    x, y, z = xyz(value)
    return [x / 100, 2 + z / 100, y / 100]


def f32(value):
    return struct.unpack('<f', struct.pack('<f', value))[0]


def digest(data):
    return dict(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())


for name in ('build-ramp.report', 'ramp.native.json.reader.report', 'compile-ramp.report'):
    assert read(root / f'{name}.json')['success']

request = read(root / 'ramp.request.json')
source_file = root / 'ramp.native.json'
frames = {case['name']: case['frames'][40] for case in read(source_file)['character_animation_probe']['cases']}
assert set(frames) == set(labels)
native = {}
target = {}
for label, case in zip(labels, request['cases'], strict=True):
    assert label == case['name']
    allowed = label == 'ramp-step'
    steep = label == 'steep-no-step'
    assert case['ramp_step'] == allowed and case['ramp_pitch'] == (60 if steep else 40)
    frame = frames[label]
    assert frame['frame'] == 40 and frame['world_delta_seconds'] == f32(.05)
    before, after = (frame[f'movement_{phase}_tick'] for phase in ('before', 'after'))
    assert before['movement_mode_name'] == after['movement_mode_name'] == 'MOVE_Walking'
    assert before['walkable_floor'] and frame['mode_events'] == []
    assert before['velocity_cm_s'] == {'x': 650, 'y': 0, 'z': 0}
    hits = frame['blocking_hits']
    first = hits[0]
    assert first['component'] == 'AuditTransitionRamp' and not first['penetrating']
    assert 0 < first['time'] < 1 and first['normal']['z'] > 0
    assert first['walkable_hit'] == (not steep)
    assert abs(first['impact_normal']['z'] - math.cos(math.radians(case['ramp_pitch']))) < 1e-7
    first_delta = delta(first)
    assert math.dist(first_delta, [31.25, 0, 0]) < 1e-6
    end = world(frame['actor_transform']['translation'])
    if steep:
        assert len(hits) == 1 and abs(end[0] - world(first['move_stop_cm'])[0]) < 1e-6
        native[label] = dict(hits=hits, position=end, mode=after['movement_mode_name'])
    else:
        witness = hits[1]
        assert witness['component'] == 'AuditRampWitness' and not witness['penetrating']
        assert not witness['walkable_hit'] and 0 < witness['time'] < 1
        assert first['move_stop_cm'] == witness['trace_start_cm']
        remain = f32(1 - first['time'])
        x, y, _ = (v * remain for v in first_delta)
        nx, ny, nz = xyz(first['impact_normal'])
        z = -f32(nx * x + ny * y) / nz
        second_delta = delta(witness)
        assert math.dist([x, y, z], second_delta) < 1e-8
        assert abs(end[0] - world(witness['move_stop_cm'])[0]) < 1e-6
        native[label] = dict(hits=hits, position=end, mode=after['movement_mode_name'],
                             second_request_cm=second_delta, first_remaining_time=remain)

    functional = read(root / f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images'] == []
    assert load(label, 'process.report')['success']
    fixture = load(label, 'fixture')
    ramp = load(label, 'ramp')
    assert fixture['half_width_cm'] == case['floor_half_extent_x_cm'] == 200
    assert ramp['pitch'] == case['ramp_pitch'] and ramp['allows_step'] == allowed
    assert math.dist(ramp['witness_center'], [-13.35, 3.5, -15]) < 2e-6
    angle = math.radians(ramp['pitch'])
    center = [-13.8 + .6 * math.cos(angle) + .05 * math.sin(angle),
              2 + .6 * math.sin(angle) - .05 * math.cos(angle), -15]
    assert math.dist(ramp['ramp_center'], center) < 2e-6

    stage = load(label, 'stage-before')
    walking = load(label, 'walking-step')
    assert stage['mode'] == 'Grounded' and stage['substeps'] == 1 and stage['delta'] == frame['world_delta_seconds']
    assert stage['cached_world_velocity'] == [6.5, 0, 0]
    assert walking['start_floor']['variants']['none']['result']['entity'] == 'AuditWalkOffPlatform'
    assert math.dist(stage['position'], world(first['trace_start_cm'])) < 1e-6
    assert math.dist(stage['planned_move'], [.3125, 0, 0]) < 1e-7
    replay = load(label, 'step-down-replay')['variants']['actual']
    forward = replay['events'][0]['hit']
    result = replay['result']
    assert forward['entity'] == 'AuditTransitionRamp'
    assert math.dist(forward['normal'], ramp['normal']) < 1e-4
    assert abs(result['advanced'][0] - world(first['move_stop_cm'])[0]) < .001
    txn = load(label, 'walking-transaction')
    actual = load(label, 'contact-after-40')
    assert actual['mode'] == txn['mode'] == 'Grounded'
    assert actual['transitions'] == []
    if allowed:
        down, = [event for event in replay['events'] if event['kind'] == 'down_request']
        assert down['start'][1] - down['end'][1] > .49
        assert result['next_y'] is not None and result['remaining'][0] > .17
        assert txn['position'] == [result['advanced'][0], result['next_y'], result['advanced'][2]]
        assert actual['position'][0] - txn['position'][0] > .17
    else:
        assert len(replay['events']) == 1 and result['next_y'] is None
        assert abs(result['remaining'][0]) < 1e-6
        assert txn['position'] == result['advanced']
        assert actual['position'][0] == txn['position'][0]
    target[label] = dict(first_hit=forward, replay=replay, pre_physics=txn['position'], after=actual)

assert native['ramp-no-step'] == native['ramp-step']
assert target['ramp-no-step']['first_hit'] == target['ramp-step']['first_hit']
assert load('ramp-no-step', 'stage-before') == load('ramp-step', 'stage-before')
assert native['ramp-no-step']['position'][0] - target['ramp-no-step']['after']['position'][0] > .17
assert abs(native['steep-no-step']['position'][0] - target['steep-no-step']['after']['position'][0]) < .001
print('Verified actual second ramp sweep before step permission and 60-degree control', flush=True)

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
    '537/wall.request.json', '536/prepare_reader.py', '536/step_down_floor.inc',
    '536/observer.py', '536/step_probe.rs')]
external = [dict(path=str(p.relative_to(Path.cwd())), **digest(p.read_bytes())) for p in external_paths]
summary = dict(gate='PASS_WALKABLE_RAMP_SWEEP_PRECEDES_STEP_PERMISSION',
               new_differences=0, total_differences=296, supplements=['BF144-D13'],
               ordinal=40, native=native, target=target, decompressed_files=decompressed,
               external_source_files=external)
(root / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print('Verified compressed evidence identities', flush=True)
