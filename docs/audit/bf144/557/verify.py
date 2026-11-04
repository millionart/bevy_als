import gzip
import hashlib
import json
import math
import struct
from pathlib import Path

root = Path(__file__).resolve().parent
labels = ('toward', 'none')
contacts = ['AuditAirCornerWall', 'AuditAirCornerSlope']


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


def mapped(value):
    return [value[k] for k in 'xzy']


def world(value):
    x, y, z = mapped(value)
    return [x / 100, 2 + y / 100, z / 100]


def add(left, right):
    return [a + b for a, b in zip(left, right, strict=True)]


def sub(left, right):
    return [a - b for a, b in zip(left, right, strict=True)]


def scale(vec, factor):
    return [v * factor for v in vec]


def dot(left, right):
    return sum(a * b for a, b in zip(left, right, strict=True))


def unit(vec):
    return scale(vec, 1 / math.hypot(*vec))


def cross(a, b):
    return [a[(i + 1) % 3] * b[(i + 2) % 3] - a[(i + 2) % 3] * b[(i + 1) % 3] for i in range(3)]


def project(vec, normal):
    return sub(vec, scale(normal, dot(vec, normal)))


def request_m(hit):
    return scale(sub(mapped(hit['trace_end_cm']), mapped(hit['trace_start_cm'])), .01)


for name in ('build-corner.report', 'corner.native.json.reader.report', 'compile-corner.report'):
    assert read(root / f'{name}.json')['success']
source_file = root / 'corner.native.json'
frames = {case['name']: case['frames'][49] for case in read(source_file)['character_animation_probe']['cases']}
assert set(frames) == set(labels)
native = {}
target = {}
for label in labels:
    toward = label == 'toward'
    row = frames[label]
    assert row['frame'] == 49 and row['world_delta_seconds'] == f32(.05)
    assert row['gravity_cm_s2'] == -981 and row['mode_events'] == []
    before, after = (row[f'movement_{phase}_tick'] for phase in ('before', 'after'))
    assert before['movement_mode_name'] == after['movement_mode_name'] == 'MOVE_Falling'
    assert before['velocity_cm_s'] == {'x': 400, 'y': 200, 'z': -200}
    one, two = row['blocking_hits']
    assert [one['component'], two['component']] == contacts
    assert all(not hit['penetrating'] and not hit['walkable_hit'] and 0 < hit['time'] < 1 for hit in (one, two))
    assert one['trace_start_cm'] == {'x': -1300, 'y': -1500, 'z': 200}
    assert one['move_stop_cm'] == two['trace_start_cm']
    n1, n2 = (mapped(hit['normal']) for hit in (one, two))
    assert n1 == [-1, 0, 0] and abs(n2[1] - .5) < 1e-6 and dot(n1, n2) < -.14
    dt = row['world_delta_seconds']
    t1 = f32(dt * f32(1 - one['time']))
    t2 = f32(t1 * f32(1 - two['time']))
    initial = scale(mapped(before['velocity_cm_s']), .01)
    air = scale(mapped(after['acceleration_cm_s2']), .01 * f32(.15))
    assert abs(air[2] - (3 if toward else 0)) < 1e-6 and air[:2] == [0, 0]
    assert dot(air, n1) == 0
    no_air = add(initial, [0, -9.81 * dt, 0])
    with_air = add(no_air, scale(air, dt))
    assert math.dist(scale(add(initial, with_air), .5 * dt), request_m(one)) < 1e-7
    assert math.dist(scale(project(with_air, n1), t1), request_m(two)) < 1e-7
    if toward:
        assert dot(air, n2) < 0
        horizontal_n2 = unit([n2[0], 0, n2[2]])
        assert dot(project(air, horizontal_n2), n1) < 0

    crease = unit(cross(n2, n1))
    deflect = scale(crease, dot(project(no_air, n1), crease))
    uncorrected = scale(crease, dot(project(with_air, n1), crease))
    assert deflect[1] < 0 and uncorrected[1] < 0
    end = world(row['actor_transform']['translation'])
    final_velocity = scale(mapped(after['velocity_cm_s']), .01)
    assert math.dist(deflect, final_velocity) < 1e-6
    assert math.dist(end, add(world(two['move_stop_cm']), scale(final_velocity, t2))) < 1e-7
    if toward:
        assert .065 < uncorrected[1] - final_velocity[1] < .066
    else:
        assert uncorrected == deflect
    native[label] = dict(hits=[one, two], position=end, velocity=final_velocity,
                         no_second_control_velocity=uncorrected, second_time_left=t2)

    functional = read(root / f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images'] == []
    assert load(label, 'process.report')['success']
    replay = load(label, 'corner-replay')
    assert replay['position'] == world(one['trace_start_cm'])
    assert replay['initial'] == initial and replay['step'] == dt
    assert math.dist(replay['incoming'], with_air) < 1e-6
    assert math.dist(replay['no_input'], no_air) < 1e-6
    assert math.dist(replay['desired'], [0, 0, 1] if toward else [0, 0, 0]) < 1e-6
    actual, control = (replay['variants'][key] for key in ('actual', 'without-second-input'))
    assert [h['name'] for h in actual['hits']] == [h['name'] for h in control['hits']] == contacts
    assert abs(actual['hits'][0]['normal'][1]) < .001
    assert math.dist(actual['hits'][1]['normal'], n2) < .0002
    assert actual['hits'][0] == control['hits'][0]
    assert all(actual['hits'][1][k] == v for k, v in control['hits'][1].items() if k != 'slide')
    result = load(label, 'contact-after-49')
    assert result['mode'] == 'Air' and result['transitions'] == []
    assert result['position'] == actual['end'] and result['published'] == actual['velocity']
    if toward:
        assert .065 < actual['velocity'][1] - final_velocity[1] < .066
        assert math.dist(actual['velocity'], final_velocity) > .075
        assert math.dist(control['velocity'], final_velocity) < .0004
        assert math.dist(control['end'], end) < .001
    else:
        assert actual == control
        assert math.dist(actual['velocity'], final_velocity) < .001
    target[label] = dict(replay=replay, actual=result)

assert math.dist(native['toward']['velocity'], native['none']['velocity']) < 2e-7
assert load('toward', 'air-corner') == load('none', 'air-corner')
print('Verified independent second-impact air-control recomputation', flush=True)

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
    '555/apex.request.json', '556/residual.inc', '523/observer.py',
    '523/native_initial.inc', '553/prepare_reader.py')]
external = [dict(path=str(p.relative_to(Path.cwd())), **digest(p.read_bytes())) for p in external_paths]
summary = dict(gate='PASS_FALLING_SECOND_IMPACT_REMOVES_AIR_CONTROL', findings=['BF144-D307'],
               new_differences=1, total_differences=304, ordinal=49, native=native, target=target,
               decompressed_files=decompressed, external_source_files=external)
(root / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print('Verified compressed evidence identities', flush=True)
