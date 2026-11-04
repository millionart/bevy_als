import gzip
import hashlib
import json
import math
import struct
from pathlib import Path

root = Path(__file__).resolve().parent
labels = ('stuck', 'positive')
contacts = ['AuditAirCornerWall', 'AuditAirCornerSecond', 'AuditAirCornerFloor']


def raw(path):
    if not path.exists():
        return gzip.decompress(path.with_suffix('.json.gz').read_bytes())
    return path.read_bytes()


def read(path):
    return json.loads(raw(path))


def load(label, name):
    return read(root / f'runtime-{label}' / f'{name}.json')


def f32(value):
    return struct.unpack('<f', struct.pack('<f', value))[0]


def digest(data):
    return dict(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())


def world(value):
    return [value['x'] / 100, 2 + value['z'] / 100, value['y'] / 100]


for name in ('build-third.report', 'third.native.json.reader.report', 'compile-third.report'):
    assert read(root / f'{name}.json')['success']
assert not load('active-mantle-failure', 'process.report')['success']
assert 'auto mantle begin' in (root / 'runtime-active-mantle-failure/diagnostic.txt').read_text()

source_file = root / 'third.native.json'
frames = {case['name']: case['frames'][49] for case in read(source_file)['character_animation_probe']['cases']}
assert set(frames) == set(labels)
native = {}
target = {}
for label in labels:
    stuck = label == 'stuck'
    row = frames[label]
    assert row['frame'] == 49 and row['world_delta_seconds'] == f32(.05)
    assert row['gravity_cm_s2'] == -981
    before, after = (row[f'movement_{phase}_tick'] for phase in ('before', 'after'))
    assert before['movement_mode_name'] == 'MOVE_Falling'
    assert before['velocity_cm_s'] == {'x': 400, 'y': 400, 'z': -100}
    one, two, three = row['blocking_hits']
    assert [one['component'], two['component'], three['component']] == contacts
    assert all(not hit['penetrating'] for hit in (one, two, three))
    assert 0 < one['time'] < 1 and 0 < two['time'] < 1
    assert not one['walkable_hit'] and not two['walkable_hit'] and three['walkable_hit']
    assert one['impact_normal'] == {'x': -1, 'y': 0, 'z': 0}
    assert two['impact_normal'] == {'x': 0, 'y': -1, 'z': 0}
    assert three['impact_normal'] == {'x': 0, 'y': 0, 'z': 1}
    assert one['move_stop_cm'] == two['trace_start_cm']
    assert two['move_stop_cm'] == three['trace_start_cm']
    assert three['trace_start_cm']['x'] == three['trace_end_cm']['x']
    assert three['trace_start_cm']['y'] == three['trace_end_cm']['y']
    assert three['trace_start_cm']['z'] > three['trace_end_cm']['z']
    assert (three['time'] == 0) == stuck
    final = world(row['actor_transform']['translation'])
    third_stop = world(three['move_stop_cm'])
    if stuck:
        assert after['movement_mode_name'] == 'MOVE_Falling' and row['mode_events'] == []
        assert abs(final[0] - third_stop[0] + .01) < 1e-12
        assert final[1:] == third_stop[1:]
        assert abs(after['velocity_cm_s']['z'] + 149.05) < .00001
    else:
        assert after['movement_mode_name'] == 'MOVE_Walking'
        event, = row['mode_events']
        assert [event['previous'], event['current']] == ['MOVE_Falling', 'MOVE_Walking']
        assert event['walking_floor']['component'] == 'AuditAirCornerFloor'
        assert final[::2] == third_stop[::2]
        assert after['velocity_cm_s'] == {'x': 0, 'y': 0, 'z': 0}
    native[label] = dict(hits=[one, two, three], end=final, mode=after['movement_mode_name'],
                         velocity_cm_s=after['velocity_cm_s'])

    functional = read(root / f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images'] == []
    assert load(label, 'process.report')['success']
    trace = load(label, 'third-contact')
    assert trace['initial'] == [4, -1, 4] and trace['step'] == row['world_delta_seconds']
    assert trace['gravity'] == [0, f32(-9.81), 0]
    assert [hit['name'] for hit in trace['hits']] == contacts
    assert abs(trace['hits'][0]['normal'][1]) < .001
    assert abs(trace['hits'][1]['normal'][1]) < .001
    assert trace['hits'][2]['normal'][1] > .999
    assert (trace['hits'][2]['travel'] == 0) == stuck
    assert trace['landing_candidate'] and trace['floor'][2] == 'AuditAirCornerFloor'
    assert trace['side_blocked'] is None
    assert abs(trace['side'][0] + .01) < 1e-6
    assert math.dist(trace['hits'][2]['stop'], third_stop) < .0003
    result = load(label, 'contact-after-49')
    assert result['mode'] == 'Grounded' and result['has_base']
    assert result['transitions'] == [['LocomotionState(Air)', 'LocomotionState(Grounded)']]
    assert result['published'][1] == 0
    transaction = load(label, 'stage-after')
    assert transaction['mode'] == 'Grounded'
    assert abs(result['position'][0] - trace['hits'][2]['stop'][0]) < .0001
    assert abs(result['position'][0] - transaction['position'][0]) < 1e-6
    assert transaction['position'][2] - trace['hits'][2]['stop'][2] > .1
    assert transaction['collision_velocity'][2] > 3
    if stuck:
        assert abs(result['position'][0] - final[0]) > .009
    else:
        assert math.dist(result['position'][:2], final[:2]) < .0003
    target[label] = dict(trace=trace, actual=result, transaction=transaction)

for left, right in zip(native['stuck']['hits'][:2], native['positive']['hits'][:2], strict=True):
    for key in ('time', 'trace_start_cm', 'trace_end_cm', 'normal', 'impact_normal', 'move_stop_cm'):
        assert left[key] == right[key]
print('Verified native side step after third zero-time hit and actual target landing', flush=True)

paths = [source_file]
paths.extend(root / f'runtime-{label}' / f'{name}.json' for label in labels for name in ('parameters', 'report'))
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
    '557/corner.request.json', '557/corner_probe.inc', '523/native_initial.inc',
    '523/observer.py', '553/prepare_reader.py')]
external = [dict(path=str(p.relative_to(Path.cwd())), **digest(p.read_bytes())) for p in external_paths]
summary = dict(gate='PASS_FALLING_THIRD_ZERO_TIME_TRIES_SIDE_STEP', findings=['BF144-D308'],
               new_differences=1, total_differences=305, ordinal=49, native=native, target=target,
               decompressed_files=decompressed, external_source_files=external)
(root / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print('Verified compressed evidence identities', flush=True)
