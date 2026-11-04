import gzip
import hashlib
import json
import math
import struct
from pathlib import Path

root = Path(__file__).resolve().parent
source = root.parent / '575'
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


assert read(root / 'compile-landing.report.json')['success']
assert not read(root / 'first-compile.report.json')['success']
assert all(value == 'PASS' for value in read(root / 'watchdog-selftest.json').values())
native = {case['name']: case['frames'][58]
          for case in read(source / 'lateral.native.json')['character_animation_probe']['cases']}
summary = {}
for label, sign, ulps in (('positive', 1, 29), ('negative', -1, 30)):
    frame = native[label]
    hit, = frame['landed_events']
    event, = frame['mode_events']
    assert event['previous'] == 'MOVE_Falling' and event['current'] == 'MOVE_Walking'
    assert hit['component'] == 'AuditLowerFloor' and not hit['penetrating']
    assert hit['impact_normal'] == dict(x=0, y=0, z=1)
    before, after = (frame[f'movement_{phase}_tick'] for phase in ('before', 'after'))
    assert before['acceleration_cm_s2']['x'] == after['acceleration_cm_s2']['x'] == 2000
    assert after['max_acceleration_cm_s2'] == 750
    native_velocity = after['velocity_cm_s']['y'] / 100
    native_time = read(root / 'native-time.json')[label]
    assert abs(native_time - f32(f32(f32(.125-f32(.05)) * .5) * f32(1-hit['time']))) < 2e-9
    end = frame['actor_transform']['translation']
    assert abs(native_time - (end['x'] - event['location_cm']['x']) / after['velocity_cm_s']['x']) < 1e-15
    assert abs(native_velocity - (end['y'] - event['location_cm']['y']) / (100*native_time)) < 1e-12

    run = root / f'runtime-{label}'
    replay = read(run / 'landing-replay.json')
    actual = read(run / 'landing-published.json')
    assert read(run / 'process.report.json')['success']
    functional = read(root / f'{label}-functional.json')
    assert not functional['pass'] and functional['images'] == []
    assert set(functional['issues']) == {'faults is non-empty', 'temporal_anomalies is non-empty'}
    visual = read(run / 'report.json')
    assert not visual['graph_errors'] and visual['primary_failure'] is None and visual['screenshots'] == []
    warning, = visual['temporal_anomalies']
    assert warning['kind'] == 'local_translation_spike' and warning['subject'] == 'pelvis'
    assert actual['mode'] == 'Grounded' and actual['steps'] == 4 and actual['contact']
    assert [step['hit_distance'] is not None for step in replay['air_steps']] == [False, False, True]
    assert replay['floor_impact'] == 0 and replay['hit_normal'][1] == 1
    assert math.hypot(replay['hit_normal'][0], replay['hit_normal'][2]) < 2e-5
    assert replay['walk_end'][::2] == actual['position'][::2]
    assert replay['replayed_velocity'] == actual['velocity']
    runtime = read(run / 'parameters.json')
    final = next(row for row in runtime['after'] if row['ordinal'] == 58)
    assert final['velocity'] == actual['velocity'] and final['gameplay'] == 'Grounded'
    assert replay['fresh_acceleration'] == 7.5 and replay['cached_acceleration'] == 20
    assert replay['source_time'] == f32(native_time)
    assert abs(replay['walk_entry'][2] - event['velocity_cm_s']['y']/100) < 2e-9
    time = replay['walk_time']
    plan = replay['planned_velocity'][2]
    published = actual['velocity'][2]
    start = replay['walk_start'][2]
    request = replay['requested_walk'][2]
    assert -16 < start < -8
    spacing = 2**-20
    assert replay['applied_walk'][2] == sign * ulps * spacing
    assert f32((f32(start + request) - start) / time) == published
    widened = ((start + request) - start) / time
    assert abs(widened - plan) < 2e-10

    cached = replay['cached_plan'][2]
    same_time = replay['cached_plan_source_time'][2]
    d07 = plan - cached
    d254 = published - plan
    time_effect = cached - same_time
    arithmetic_remainder = same_time - native_velocity
    assert 50e-6 < sign*d07 < 52e-6
    assert -25e-6 < d254 < -20e-6
    assert abs(time_effect) < .25e-6
    assert abs(arithmetic_remainder) < 1e-9
    assert math.isclose(d07 + d254 + time_effect + arithmetic_remainder,
                        published - native_velocity, abs_tol=1e-12)
    old = read(source / 'summary.json')['landing_velocity_residual'][label]
    assert published - native_velocity == old
    summary[label] = dict(native_velocity=native_velocity, published=published, planned=plan,
                          cached_plan=cached, cached_common_time=same_time, ulps=ulps,
                          source_time=native_time, target_time=time,
                          d07=d07, d254=d254, time_effect=time_effect,
                          arithmetic_remainder=arithmetic_remainder)

print('Verified actual q58 publication and independent acceleration, time and F32 contributions', flush=True)
paths = [root / f'runtime-{label}' / f'{name}.json'
         for label in labels for name in ('parameters', 'report')]
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
for name in ('575/lateral.native.json.gz', '575/lateral.request.json', '575/lateral.package.json',
             '575/evidence-manifest.json', '575/observer.py', '575/direction_probe.inc', '575/summary.json',
             '574/height_probe.inc', '573/cached_acceleration.inc'):
    path = root.parent / name
    external.append(dict(path=str(path.relative_to(Path.cwd())), **digest(path.read_bytes())))
result = dict(gate='PASS_LANDING_VELOCITY_WORLD_POSITION_QUANTIZATION', findings=['BF144-D07', 'BF144-D254'],
              new_differences=0, total_differences=310, decomposition=summary,
              public_functional_pass=False, decompressed_files=decompressed, external_source_files=external)
(root / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
print('Verified compressed evidence identities', flush=True)
