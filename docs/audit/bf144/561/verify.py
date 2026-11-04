import gzip
import hashlib
import json
import math
import struct
from pathlib import Path

root = Path(__file__).resolve().parent
labels = ('baseline', 'deflected', 'no-time')


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


def velocity_m_s(value):
    return [value[k] / 100 for k in 'xzy']


for name in ('build-time.report', 'time.native.json.reader.report', 'compile-time.report'):
    assert read(root / f'{name}.json')['success']
source_file = root / 'time.native.json'
frames = {case['name']: case['frames'][49] for case in read(source_file)['character_animation_probe']['cases']}
assert set(frames) == {'third', 'second'}
native = {}
for label in ('third', 'second'):
    third = label == 'third'
    row = frames[label]
    assert row['frame'] == 49 and row['world_delta_seconds'] == f32(.05)
    assert row['gravity_cm_s2'] == -981
    before, after = (row[f'movement_{phase}_tick'] for phase in ('before', 'after'))
    assert before['movement_mode_name'] == 'MOVE_Falling'
    assert before['velocity_cm_s'] == {'x': 400, 'y': 200, 'z': -200}
    assert after['movement_mode_name'] == 'MOVE_Walking'
    hits = row['blocking_hits']
    expected = ['AuditAirCornerWall', 'AuditThirdFloor']
    if third:
        expected.insert(1, 'AuditAirCornerSlope')
    assert [h['component'] for h in hits] == expected
    assert all(not h['penetrating'] and 0 < h['time'] < 1 for h in hits)
    assert not any(h['walkable_hit'] for h in hits[:-1]) and hits[-1]['walkable_hit']
    assert hits[-1]['impact_normal'] == {'x': 0, 'y': 0, 'z': 1}
    geometric_left = row['world_delta_seconds']
    for hit in hits:
        geometric_left = f32(geometric_left * f32(1 - hit['time']))
    event, = row['mode_events']
    assert [event['previous'], event['current']] == ['MOVE_Falling', 'MOVE_Walking']
    assert event['walking_floor']['component'] == 'AuditThirdFloor'
    landing = world(event['location_cm'])
    end = world(row['actor_transform']['translation'])
    event_v = velocity_m_s(event['velocity_cm_s'])
    final_v = velocity_m_s(row['actor_velocity_cm_s'])
    assert landing[::2] == world(hits[-1]['move_stop_cm'])[::2]
    assert event_v[1] == final_v[1] == 0
    if third:
        assert landing == end and event_v == final_v
        assert -.576 < final_v[2] < -.574
        assert .021 < geometric_left < .022
    else:
        assert abs(event_v[2] - 2) < 1e-6
        assert .020 < end[2] - landing[2] < .022
        assert .54 < final_v[2] < .55
        assert .038 < geometric_left < .039
    native[label] = dict(hits=hits, geometric_time_left=geometric_left, landing=landing,
                         landing_velocity=event_v, end=end, end_velocity=final_v)

target = {}
for name in ('air-corner', 'third-contact', 'stage-before', 'stage-after', 'walking-handoff-replay'):
    assert load('baseline', name) == load('deflected', name) == load('no-time', name)
for label in labels:
    functional = read(root / f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images'] == []
    assert load(label, 'process.report')['success']
    trace = load(label, 'third-contact')
    assert trace['initial'] == [4, -2, 2]
    assert trace['landing_candidate'] and trace['hits'][2]['travel'] > 0
    assert [h['name'] for h in trace['hits']] == ['AuditAirCornerWall', 'AuditAirCornerSlope', 'AuditThirdFloor']
    replay = load(label, 'walking-handoff-replay')
    assert replay['remaining'] == trace['hits'][2]['time_left']
    assert .021 < replay['remaining'] < .022
    v = trace['third_velocity']
    variants = replay['variants']
    incoming, deflected, no_time = (variants[k] for k in ('incoming', 'deflected', 'no-time'))
    assert incoming['walking_input'] == [4, 0, 2]
    assert deflected['walking_input'] == no_time['walking_input'] == [v[0], 0, v[2]]
    assert math.dist(deflected['walking_input'], native['third']['landing_velocity']) < .0003
    assert deflected['feedback'] == deflected['requested_velocity'] == [0, 0, 0]
    assert no_time['feedback'] == no_time['walking_input'] and no_time['requested_velocity'] is None
    assert deflected['end'] == no_time['end'] == replay['landing']

    stage = load(label, 'stage-after')
    handoff = load(label, 'handoff-control')
    assert handoff['choice'] == label and stage['mode'] == 'Grounded'
    assert incoming['end'] == handoff['original_position'] == stage['position']
    assert incoming['feedback'] == handoff['original_feedback'] == stage['collision_velocity']
    selected = incoming if label == 'baseline' else variants[label]
    assert handoff['selected_feedback'] == selected['feedback']
    assert handoff['selected_position'] == selected['end']
    for key in ('motor_boost', 'motor_acceleration'):
        assert handoff[key] == load('baseline', 'handoff-control')[key]
    actual = load(label, 'contact-after-49')
    assert actual['mode'] == 'Grounded' and actual['has_base']
    assert actual['transitions'] == [['LocomotionState(Air)', 'LocomotionState(Grounded)']]
    assert actual['published'] == selected['feedback']
    if label != 'baseline':
        assert actual['position'] == replay['landing']
        assert math.dist(actual['position'], native['third']['end']) < .0003
    target[label] = dict(replay=replay, handoff=handoff, actual=actual)

assert math.dist(target['no-time']['actual']['published'], native['third']['end_velocity']) < .0003
assert math.dist(target['deflected']['actual']['published'], native['third']['end_velocity']) > .57
print('Verified third-landing time reset independently of deflected velocity handoff', flush=True)

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
    '557/corner.request.json', '557/prepare_reader.py', '559/observer.py', '559/corner_probe.inc',
    '560/observer.py', '560/handoff_probe.inc')]
external = [dict(path=str(p.relative_to(Path.cwd())), **digest(p.read_bytes())) for p in external_paths]
summary = dict(gate='PASS_THIRD_LANDING_DROPS_REMAINING_WALKING_TIME', findings=['BF144-D310'],
               new_differences=1, total_differences=307, ordinal=49, native=native, target=target,
               decompressed_files=decompressed, external_source_files=external)
(root / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print('Verified compressed evidence identities', flush=True)
