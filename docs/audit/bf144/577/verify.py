import gzip
import hashlib
import json
import math
import struct
from pathlib import Path

root = Path(__file__).resolve().parent
labels = ('positive', 'negative')


def raw(path):
    if path.exists():
        return path.read_bytes()
    return gzip.decompress(path.with_suffix('.json.gz').read_bytes())


def read(path):
    return json.loads(raw(path))


def digest(data):
    return dict(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())


def f32(value):
    return struct.unpack('<f', struct.pack('<f', value))[0]


def native_time(distance_cm, length_cm, step):
    ratio = f32(distance_cm / length_cm)
    length_cm = f32(length_cm)
    pullback = f32(min(max(f32(.1), f32(f32(.1)/length_cm)), f32(1/length_cm)) + f32(.001))
    return f32(step * f32(1 - f32(max(0, ratio - pullback))))


def target_time(distance, length, step):
    pullback = f32(min(max(f32(length*f32(.1)), f32(.001)), f32(.01)) + f32(length*f32(.001)))
    travel = f32(max(0, f32(distance - pullback)))
    return f32(step * f32(1-f32(travel/length)))


for name in ('compile-geometry.report', 'floor.native.json.reader.report', 'geometry.native.json.reader.report'):
    assert read(root / f'{name}.json')['success']
assert all(value == 'PASS' for value in read(root / 'watchdog-selftest.json').values())
floor = read(root / 'floor.native.json')
body, = [item for item in floor['asset_objects'] if item['class_path'] == '/Script/Engine.BodySetup']
properties = {item['name']: item['exported_text'] for item in body['properties']}
assert properties['AggGeom'] == '(BoxElems=((Center=(X=0.000000,Y=0.000000,Z=-25.000000),X=2000.000000,Y=2000.000000,Z=50.000000,bIsGenerated=True)))'
assert properties['CollisionTraceFlag'] == 'CTF_UseDefault'
request = read(root / 'geometry.request.json')['samples']
queries = read(root / 'geometry.native.json')['geometry_query_probe']['samples']
assert len(request) == len(queries) == 10
cases = {}
for requested, actual in zip(request, queries):
    assert requested == actual['input']
    assert actual['capsule_sweep']['hit']
    assert actual['capsule_sweep']['normal'] == dict(x=0, y=0, z=1)
    cases[requested['name']] = actual

old = read(root.parent / '576' / 'summary.json')['decomposition']
output = {}
for label in labels:
    run = root / f'runtime-{label}'
    live = read(run / 'landing-geometry.json')
    replay = read(run / 'landing-replay.json')
    published = read(run / 'landing-published.json')
    assert read(run / 'process.report.json')['success']
    functional = read(root / f'{label}-functional.json')
    assert not functional['pass'] and functional['images'] == []
    assert set(functional['issues']) == {'faults is non-empty', 'temporal_anomalies is non-empty'}
    report = read(run / 'report.json')
    assert not report['graph_errors'] and report['primary_failure'] is None and report['screenshots'] == []
    warning, = report['temporal_anomalies']
    assert warning['kind'] == 'local_translation_spike' and warning['subject'] == 'pelvis'
    assert live['floor_center'] == [0, -.25, -20]
    assert live['floor_rotation'] == [0, 0, 0, 1] and live['floor_half_extents'] == [10, .25, 10]
    assert abs(live['capsule_extents'][0] - .3) < 4e-8 and abs(live['capsule_extents'][1] - .9) < 4e-8
    assert abs(live['orientation'][0]) < 1e-7 and abs(live['orientation'][2]) < 1e-7
    assert live['actual_distance'] == replay['hit_distance'] == live['actual']['distance']
    assert live['direct_world'] == live['direct_relative'] == live['actual_distance']
    assert replay['air_steps'][2]['position'] == live['actual']['start']
    assert published['steps'] == 4 and published['mode'] == 'Grounded'
    assert published['velocity'] == replay['replayed_velocity']
    assert replay['walk_time'] == old[label]['target_time']
    assert published['velocity'][2] == old[label]['published']
    assert 5.8e-5 < live['actual']['start'][1] - live['height_control']['start'][1] < 6e-5

    native = {name: cases[f'{label}-{name}'] for name in
              ('actual', 'direction_control', 'height_control', 'reference_box', 'archived')}
    distances = {name: row['capsule_sweep']['distance_cm'] for name, row in native.items()}
    times = {name: native_time(distances[name], row['input']['capsule_sweep']['length_cm'], live['step'])
             for name, row in native.items()}
    assert distances['actual'] == distances['direction_control']
    assert abs(distances['actual'] - distances['reference_box']) < 3e-6
    assert abs(times['archived'] - old[label]['source_time']) < 5e-9
    target_query_time = target_time(live['actual_distance'], live['max_distance'], live['step'])
    target_common_time = target_time(f32(distances['actual']/100), live['max_distance'], live['step'])
    assert target_query_time == replay['walk_time']
    assert target_common_time == times['actual']
    height = times['actual'] - times['height_control']
    query = target_query_time - target_common_time
    pullback = target_common_time - times['actual']
    frame_input = times['height_control'] - old[label]['source_time']
    assert -10.85e-6 < height < -10.82e-6
    assert abs(frame_input) < 1e-8 and pullback == 0
    if label == 'negative':
        assert 58.93e-6 < query < 58.96e-6
        assert .00049 < distances['actual']/100 - live['actual_distance'] < .00051
        assert live['actual']['point_on_capsule'][1] - live['actual']['point_on_floor'][1] > .0003
    else:
        assert abs(query) < 3e-9
        assert abs(distances['actual']/100 - live['actual_distance']) < 3e-8
    assert math.isclose(height+query+pullback+frame_input,
                        old[label]['target_time']-old[label]['source_time'], abs_tol=1e-14)
    output[label] = dict(native_distances_cm=distances, target_distance=live['actual_distance'],
                         target_direction_control=live['direction_control']['distance'],
                         height=height, query=query, pullback=pullback,
                         remaining_input_effect=frame_input, target_time=target_query_time)

print('Verified same-geometry Chaos and live shape-cast time decomposition', flush=True)
paths = [root / 'floor.native.json', root / 'geometry.native.json']
paths.extend(root / f'runtime-{label}' / f'{name}.json'
             for label in labels for name in ('parameters', 'report'))
decompressed = {}
for path in paths:
    data = raw(path)
    archive = path.with_suffix('.json.gz')
    if path.exists():
        archive.write_bytes(gzip.compress(data, compresslevel=6, mtime=0))
    assert gzip.decompress(archive.read_bytes()) == data
    decompressed[str(archive.relative_to(root))] = digest(data)
    path.unlink(missing_ok=True)

external = []
for name in ('576/summary.json', '576/observer.py', '576/landing_probe.inc', '576/native-time.json',
             '575/lateral.native.json.gz', '572/native_floor.inc', '460/floor.native.json'):
    path = root.parent / name
    external.append(dict(path=str(path.relative_to(Path.cwd())), **digest(path.read_bytes())))
result = dict(gate='PASS_MATCHED_LANDING_SWEEP_TIME_DECOMPOSITION', findings=['BF144-D125'],
              new_differences=0, total_differences=310, decomposition=output,
              public_functional_pass=False, decompressed_files=decompressed, external_source_files=external)
(root / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
print('Verified compressed evidence identities', flush=True)
