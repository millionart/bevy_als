import gzip
import hashlib
import json
import math
import struct
from pathlib import Path

root = Path(__file__).resolve().parent
labels = ('stalled', 'moving')


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


def digest(data):
    return dict(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())


for name in ('build-stall.report', 'stall.native.json.reader.report', 'compile-stall.report'):
    assert read(root / f'{name}.json')['success']

request = read(root / 'stall.request.json')
source_file = root / 'stall.native.json'
frames = {case['name']: case['frames'][40] for case in read(source_file)['character_animation_probe']['cases']}
assert set(frames) == set(labels)
native = {}
target = {}
for label, case in zip(labels, request['cases'], strict=True):
    assert case['name'] == label
    stalled = label == 'stalled'
    frame = frames[label]
    assert frame['frame'] == 40 and frame['world_delta_seconds'] == f32(.06)
    before, after = (frame[f'movement_{phase}_tick'] for phase in ('before', 'after'))
    assert before['movement_mode_name'] == after['movement_mode_name'] == 'MOVE_Walking'
    assert before['velocity_cm_s'] == {'x': 650, 'y': 0, 'z': 0}
    assert before['floor_blocking'] and before['walkable_floor'] and frame['mode_events'] == []
    assert abs(before['floor_distance_cm'] - after['floor_distance_cm']) < 1e-6
    hits = frame['blocking_hits']
    assert len(hits) == (1 if stalled else 2)
    first = hits[0]
    assert first['trace_start_cm'] == {'x': -1398, 'y': -1500, 'z': 92.15}
    first_request = first['trace_end_cm']['x'] - first['trace_start_cm']['x']
    assert abs(first_request - 19.05) < 1e-6
    for hit in hits:
        assert hit['component'] == 'AuditStallWall' and not hit['penetrating']
        assert xyz(hit['normal']) == xyz(hit['impact_normal']) == [-1, 0, 0]
        assert abs(hit['location_cm']['x'] - (-1500 + case['stall_front_cm'] - 30)) < 1e-5
        assert hit['trace_start_cm']['z'] == hit['move_stop_cm']['z'] == 92.15
    final = frame['actor_transform']['translation']
    assert final == hits[-1]['move_stop_cm']
    assert (first['time'] == 0) == stalled
    if stalled:
        assert final == first['trace_start_cm']
        assert xyz(after['velocity_cm_s']) == [0, 0, 0]
    else:
        assert hits[1]['trace_start_cm'] == first['move_stop_cm']
        assert all(0 < hit['time'] < 1 for hit in hits)
        assert final['x'] > first['move_stop_cm']['x']
    native[label] = dict(hits=hits, position=world(final), velocity_cm_s=after['velocity_cm_s'])

    functional = read(root / f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images'] == []
    assert load(label, 'process.report')['success']
    stage = load(label, 'stage-before')
    assert stage['mode'] == 'Grounded' and stage['substeps'] == 2 and stage['delta'] == frame['world_delta_seconds']
    assert stage['cached_world_velocity'] == [6.5, 0, 0]
    assert math.dist(stage['position'], world(first['trace_start_cm'])) < 1e-6
    fixture = load(label, 'stall-wall')
    assert abs(fixture['front'] - case['stall_front_cm'] / 100) < 1e-6
    replay = load(label, 'stall-substeps')
    step = replay['step_s']
    assert step == f32(.03)
    one, two = replay['rows']
    assert one['start'] == stage['position'] and two['start'] == one['end']
    assert abs(one['request'][0] - first_request / 100) < 1e-7
    for row in replay['rows']:
        assert row['floor'] == 'AuditWalkOffPlatform' and .019 <= row['floor_gap_m'] <= .024
        assert row['hit']['entity'] == 'AuditStallWall' and row['hit']['distance_m'] > 0
        assert math.hypot(*row['slide']) < 1e-6
        requested_distance = math.hypot(*row['request'])
        pullback = min(.01, max(.001, requested_distance * .1)) + requested_distance * .001
        assert abs(row['travel'] - max(0, row['hit']['distance_m'] - pullback)) < 1e-8
    if stalled:
        assert one['start'] == one['end'] and one['applied'] == [0, 0, 0] and one['travel'] == 0
        assert one['hit']['distance_m'] == two['hit']['distance_m']
        assert .0014 < two['applied'][0] < .0015
        assert .0067 < two['request'][0] < .0068
        assert math.dist(one['end'], world(final)) < 1e-6
        assert .0014 < math.dist(two['end'], world(final)) < .0015
    else:
        assert one['applied'][0] > .029 and two['applied'][0] > .006

    txn = load(label, 'walking-transaction')
    observed = load(label, 'stage-after')
    after_target = load(label, 'contact-after-40')
    assert txn['position'] == observed['position'] == after_target['position'] == two['end']
    assert after_target['mode'] == txn['mode'] == 'Grounded' and after_target['transitions'] == []
    assert observed['walking_last_input'] == two['input_velocity']
    expected_velocity = [f32(v / step) for v in two['applied']]
    assert observed['collision_velocity'] == expected_velocity
    target[label] = dict(replay=replay, pre_physics=txn['position'], after=after_target)

assert load('stalled', 'stage-before') == load('moving', 'stage-before')
print('Verified source zero-position exit and actual target second-step displacement', flush=True)

paths = [source_file]
paths.extend(root / f'runtime-{label}' / f'{name}.json' for label in labels for name in (
    'parameters', 'report', 'walking-step', 'walking-transaction'))
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
    '550/roof.request.json', '533/observer.py', '533/walking_probe.rs',
    '536/prepare_reader.py', '536/step_down_floor.inc')]
external = [dict(path=str(p.relative_to(Path.cwd())), **digest(p.read_bytes())) for p in external_paths]
summary = dict(gate='PASS_WALKING_ZERO_POSITION_TERMINATES_SOURCE_SUBSTEPS', findings=['BF144-D305'],
               new_differences=1, total_differences=302, ordinal=40, native=native, target=target,
               decompressed_files=decompressed, external_source_files=external)
(root / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print('Verified compressed evidence identities', flush=True)
