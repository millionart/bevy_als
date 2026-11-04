import gzip
import hashlib
import json
import math
import struct
from pathlib import Path

root = Path(__file__).resolve().parent
prior = root.parent / '557'
labels = ('away-first', 'toward')
contacts = ['AuditAirCornerWall', 'AuditAirCornerSlope']


def raw(path):
    if not path.exists():
        return gzip.decompress(path.with_suffix('.json.gz').read_bytes())
    return gzip.decompress(path.read_bytes()) if path.suffix == '.gz' else path.read_bytes()


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


assert read(root / 'compile-away.report.json')['success']
assert read(root / 'away.native.json.reader.report.json')['success']
assert read(root / 'away.package.json') == read(prior / 'corner.package.json')
source_file = root / 'away.native.json'
frames = {case['name']: case['frames'][49] for case in read(source_file)['character_animation_probe']['cases']}
assert set(frames) == set(labels)
old = read(prior / 'summary.json')
native = {}
target = {}
for label in labels:
    away = label == 'away-first'
    row = frames[label]
    assert row['frame'] == 49 and row['world_delta_seconds'] == f32(.05)
    assert row['gravity_cm_s2'] == -981 and row['mode_events'] == []
    before, after = (row[f'movement_{phase}_tick'] for phase in ('before', 'after'))
    assert before['movement_mode_name'] == after['movement_mode_name'] == 'MOVE_Falling'
    assert mapped(before['velocity_cm_s']) == [400, -200, 200]
    one, two = row['blocking_hits']
    assert [one['component'], two['component']] == contacts
    assert all(not hit['penetrating'] and not hit['walkable_hit'] and 0 < hit['time'] < 1 for hit in (one, two))
    assert one['move_stop_cm'] == two['trace_start_cm']
    n1, n2 = (mapped(hit['normal']) for hit in (one, two))
    assert n1 == [-1, 0, 0] and abs(n2[1] - .5) < 1e-6 and dot(n1, n2) < -.14
    dt = row['world_delta_seconds']
    t1 = f32(dt * f32(1 - one['time']))
    t2 = f32(t1 * f32(1 - two['time']))
    air = scale(mapped(after['acceleration_cm_s2']), .01 * f32(.15))
    expected_input = [-.5, 0, math.sqrt(.75)] if away else [0, 0, 1]
    assert math.dist(air, scale(expected_input, 3)) < 1e-6
    assert dot(air, n2) < 0
    limited = project(air, unit([n2[0], 0, n2[2]]))
    delta_v = scale(limited, t2)
    accepted = dot(delta_v, n1) > 0
    assert accepted == away
    no_air = [4, -2 - 9.81 * dt, 2]
    crease = unit(cross(n2, n1))
    base = scale(crease, dot(project(no_air, n1), crease))
    final_velocity = scale(mapped(after['velocity_cm_s']), .01)
    predicted = add(base, delta_v) if accepted else base
    assert math.dist(predicted, final_velocity) < 1e-6
    end = world(row['actor_transform']['translation'])
    assert math.dist(end, add(world(two['move_stop_cm']), scale(final_velocity, t2))) < 1e-7
    if away:
        assert -.031 < final_velocity[0] < -.03
        assert .0009 < world(two['move_stop_cm'])[0] - end[0] < .001
    else:
        assert old['native']['toward']['hits'] == [one, two]
        assert old['native']['toward']['velocity'] == final_velocity
    native[label] = dict(hits=[one, two], end=end, velocity=final_velocity, without_add=base,
                         limited_delta_v=delta_v, accepted=accepted)

    functional = read(root / f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images'] == []
    assert load(label, 'process.report')['success']
    replay = load(label, 'corner-replay')
    assert replay['initial'] == [4, -2, 2] and replay['step'] == dt
    assert math.dist(replay['desired'], expected_input) < 1e-6
    original, omitted, control = (replay['variants'][k] for k in (
        'actual', 'without-second-input', 'source-second-control'))
    for variant in (original, omitted, control):
        assert [hit['name'] for hit in variant['hits']] == contacts
        assert variant['hits'][0] == original['hits'][0]
        for key in ('name', 'normal', 'start', 'request', 'stop', 'travel', 'time_left'):
            assert variant['hits'][1][key] == original['hits'][1][key]
    controlled_hit = control['hits'][1]
    assert controlled_hit['added'] == away
    assert (dot(controlled_hit['air_delta_v'], original['hits'][0]['normal']) > 0) == away
    assert math.dist(control['velocity'], final_velocity) < .001
    assert math.dist(control['end'], end) < .001
    actual = load(label, 'contact-after-49')
    assert actual['mode'] == 'Air' and actual['transitions'] == []
    assert actual['position'] == original['end'] and actual['published'] == original['velocity']
    if away:
        assert abs(omitted['velocity'][0]) < .0002 and abs(original['velocity'][0]) < .0002
        assert math.dist(control['velocity'], add(omitted['velocity'], controlled_hit['air_delta_v'])) < 1e-6
        assert math.dist(omitted['velocity'], final_velocity) > .03
    else:
        assert omitted['velocity'] == control['velocity'] and omitted['end'] == control['end']
        assert actual == old['target']['toward']['actual']
    target[label] = dict(replay=replay, actual=actual)

assert load('away-first', 'air-corner') == load('toward', 'air-corner')
print('Verified positive and negative second-impact reintroduction gates', flush=True)

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

external_paths = [prior / name for name in ('corner.request.json', 'corner.package.json', 'summary.json',
                                             'observer.py', 'corner_probe.inc', 'corner_fixture.inc')]
external = [dict(path=str(p.relative_to(Path.cwd())), **digest(p.read_bytes())) for p in external_paths]
summary = dict(gate='PASS_SECOND_IMPACT_REINTRODUCES_AWAY_FROM_FIRST_WALL', supplements=['BF144-D307'],
               new_differences=0, total_differences=304, ordinal=49, native=native, target=target,
               decompressed_files=decompressed, external_source_files=external)
(root / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print('Verified compressed evidence identities', flush=True)
