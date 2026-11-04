import gzip
import hashlib
import json
import math
import struct
from pathlib import Path

root = Path(__file__).resolve().parent
labels = ('blocked', 'escape')
contacts = ['AuditAirCornerWall', 'AuditAirCornerSecond', 'AuditAirCornerSlope']


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


for name in ('build-side.report', 'side.native.json.reader.report', 'compile-side.report'):
    assert read(root / f'{name}.json')['success']
initial_file = root / 'initial-geometry/side.native.json'
for case in read(initial_file)['character_animation_probe']['cases']:
    first = case['frames'][49]['blocking_hits'][0]
    assert first['trace_start_cm'] != {'x': -1300, 'y': -1500, 'z': 200}
    assert case['frames'][49]['blocking_hits'][2]['time'] > 0

equal_file = root / 'equal-walls/side.native.json'
equal_native = read(equal_file)['character_animation_probe']['cases'][0]['frames'][49]
equal_target = read(root / 'equal-walls/blocked-third-contact.json')
assert equal_native['blocking_hits'][0]['component'] == contacts[0]
assert equal_target['hits'][0]['name'] == contacts[1]

source_file = root / 'side.native.json'
frames = {case['name']: case['frames'][49] for case in read(source_file)['character_animation_probe']['cases']}
assert set(frames) == set(labels)
native = {}
target = {}
for label in labels:
    blocked = label == 'blocked'
    row = frames[label]
    assert row['frame'] == 49 and row['world_delta_seconds'] == f32(.05)
    assert row['gravity_cm_s2'] == -981
    before, after = (row[f'movement_{phase}_tick'] for phase in ('before', 'after'))
    assert before['movement_mode_name'] == 'MOVE_Falling'
    assert before['velocity_cm_s'] == {'x': 400, 'y': 400, 'z': 0}
    one, two, third, side = row['blocking_hits']
    assert [h['component'] for h in (one, two, third, side)] == contacts + [contacts[2]]
    assert all(not h['penetrating'] and not h['walkable_hit'] for h in (one, two, third, side))
    assert one['impact_normal'] == {'x': -1, 'y': 0, 'z': 0}
    assert two['impact_normal'] == {'x': 0, 'y': -1, 'z': 0}
    origin = {'x': -1300, 'y': -1500, 'z': 200}
    assert one['trace_start_cm'] == origin
    for hit in (one, two, third):
        assert hit['time'] == 0 and hit['move_stop_cm'] == origin
    assert side['trace_start_cm'] == origin
    assert side['trace_end_cm'] == {'x': -1301, 'y': -1500, 'z': 200}
    assert abs(third['impact_normal']['z'] - .5) < 1e-6
    assert abs(side['impact_normal']['z'] - .5) < 1e-6
    assert (side['time'] == 0) == blocked
    final = row['actor_transform']['translation']
    assert final == side['move_stop_cm']
    if blocked:
        assert final == origin
        assert after['movement_mode_name'] == 'MOVE_Walking' and not after['walkable_floor']
        event, = row['mode_events']
        assert [event['previous'], event['current']] == ['MOVE_Falling', 'MOVE_Walking']
        assert not event['walking_floor']['walkable'] and not event['walking_floor']['valid_landing_spot']
        assert event['location_cm'] == final and event['velocity_cm_s'] == {'x': 0, 'y': 0, 'z': 0}
    else:
        assert after['movement_mode_name'] == 'MOVE_Falling' and row['mode_events'] == []
        assert final['x'] < -1300
    native[label] = dict(hits=row['blocking_hits'], position=world(final), mode=after['movement_mode_name'],
                         mode_events=row['mode_events'])

    functional = read(root / f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images'] == []
    assert load(label, 'process.report')['success']
    trace = load(label, 'third-contact')
    assert trace['initial'] == [4, 0, 4] and trace['step'] == row['world_delta_seconds']
    assert trace['gravity'] == [0, f32(-9.81), 0]
    assert [hit['name'] for hit in trace['hits']] == contacts
    assert all(hit['travel'] == 0 for hit in trace['hits'])
    assert abs(trace['hits'][2]['normal'][1] - .5) < .001
    assert not trace['landing_candidate'] and trace['floor'] is None
    assert math.dist(trace['side'], [-.01, 0, 0]) < .00001
    assert trace['side_hit']['name'] == 'AuditAirCornerSlope'
    assert (trace['side_hit']['travel'] == 0) == blocked
    result = load(label, 'contact-after-49')
    transaction = load(label, 'stage-after')
    assert result['mode'] == transaction['mode'] == 'Air' and result['transitions'] == []
    assert not result['has_base']
    assert result['published'] == transaction['collision_velocity'] == trace['after_third_velocity']
    assert result['position'] == transaction['position'] == trace['hits'][2]['stop']
    target[label] = dict(trace=trace, transaction=transaction, actual=result)

print('Verified native forced landing after a blocked side step without a walkable floor', flush=True)

paths = [source_file, initial_file, equal_file]
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
    '559/third.request.json', '559/observer.py', '559/corner_probe.inc',
    '523/native_initial.inc', '523/observer.py', '553/prepare_reader.py')]
external = [dict(path=str(p.relative_to(Path.cwd())), **digest(p.read_bytes())) for p in external_paths]
summary = dict(gate='PASS_BLOCKED_FALLING_SIDE_STEP_FORCES_LANDING', supplements=['BF144-D308'],
               new_differences=0, total_differences=307, ordinal=49, native=native, target=target,
               decompressed_files=decompressed, external_source_files=external)
(root / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print('Verified compressed evidence identities', flush=True)
