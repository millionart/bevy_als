import gzip
import hashlib
import json
import struct
from pathlib import Path

root = Path(__file__).resolve().parent
labels = ('baseline', 'band')


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


for name in ('compile-brake.report', 'geometry.native.json.reader.report'):
    assert read(root / f'{name}.json')['success']
assert all(value == 'PASS' for value in read(root / 'watchdog-selftest.json').values())
native = next(case['frames'] for case in read(root.parent / '575' / 'lateral.native.json')['character_animation_probe']['cases']
              if case['name'] == 'axial')
for frame in range(44, 52):
    src = native[frame]
    assert src['movement_before_tick']['movement_mode_name'] == 'MOVE_Walking'
    assert src['movement_after_tick']['movement_mode_name'] == 'MOVE_Walking'
    assert not src.get('blocking_hits', [])
    assert src['actor_transform']['translation']['z'] == native[44]['actor_transform']['translation']['z']
    assert 1.9 < src['movement_after_tick']['floor_distance_cm'] < 2.4

snapshots, trajectory = {}, {}
for label in labels:
    path = root / f'runtime-{label}'
    functional = read(root / f'{label}-functional.json')
    assert not functional['pass'] and functional['images'] == []
    assert set(functional['issues']) == {'faults is non-empty', 'temporal_anomalies is non-empty'}
    report = read(path / 'report.json')
    assert not report['graph_errors'] and report['primary_failure'] is None and report['screenshots'] == []
    warning, = report['temporal_anomalies']
    assert warning['kind'] == 'local_translation_spike' and warning['subject'] == 'pelvis'
    assert read(path / 'process.report.json')['success']
    model = read(path / 'brake-replay.json')
    actual = read(path / 'brake-submitted.json')
    physics = read(path / 'brake-physics.json')
    assert model['factor'] == 3 and model['velocity'] == [2.5, 0, 0]
    assert [step['empty'] for step in model['steps']] == [False, True, True]
    assert model['final'] == actual['candidate']
    assert model['final_velocity'] == actual['velocity'] == physics['velocity'] == [0, 0, 0]
    assert actual['mode'] == 'Grounded'
    assert actual['physical_y'] == actual['motor_boost_y'] == 0
    assert actual['motor_acceleration_y'] == -actual['gravity_y']
    assert actual['submitted'] == physics['position']
    assert actual['submitted'][::2] == actual['candidate'][::2]
    assert actual['submitted'][1] == (model['start'][1] if label == 'band' else actual['candidate'][1])
    assert read(path / 'mixed-before.json')['walking_acceleration'] == 20
    rows = {row['ordinal']: row for row in read(path / 'parameters.json')['after']}
    result = {}
    for frame in range(44, 59):
        row = rows[frame]
        src = native[frame]
        src_position = src['actor_transform']['translation']
        src_velocity = src['movement_after_tick']['velocity_cm_s']
        expected = 'Air' if 54 <= frame <= 57 else 'Grounded'
        assert row['gameplay'] == expected
        assert abs(row['position'][0] - src_position['x']/100) < 5e-6
        assert abs(row['velocity'][1] - src_velocity['z']/100) < 2e-6
        assert row['velocity'][2] == 0
        gap = row['position'][1] - (2 + src_position['z']/100)
        if 45 <= frame <= 57:
            assert 58e-6 < gap < 61e-6 if label == 'baseline' else abs(gap) < .5e-6
        result[frame] = dict(position=row['position'], velocity=row['velocity'], mode=expected, height_gap=gap)
    assert rows[45]['position'] == actual['submitted']
    snapshots[label] = dict(replay=model, submitted=actual, physics=physics)
    trajectory[label] = result

assert snapshots['baseline']['replay'] == snapshots['band']['replay']
first = snapshots['baseline']['replay']['steps'][0]
request = read(root / 'geometry.request.json')['samples']
queries = read(root / 'geometry.native.json')['geometry_query_probe']['samples']
assert len(request) == len(queries) == 2
native_casts = {}
for submitted, result in zip(request, queries):
    assert submitted == result['input']
    sweep = result['capsule_sweep']
    assert sweep['hit'] and sweep['normal'] == dict(x=0, y=0, z=1)
    native_casts[submitted['name']] = sweep['distance_cm']/100
full_difference = native_casts['1-full'] - first['full_hit']
short_difference = native_casts['1-short'] - first['short_hit']
assert 145e-6 < full_difference < 152e-6
assert 58e-6 < short_difference < 61e-6
assert .024 < first['short_start'][1] - first['floor_center'] < .0242
native_center = f32(first['short_start'][1] - f32(f32(native_casts['1-short']) - first['short_shrink']))
assert first['short_start'][1] - native_center > .024
corrected = f32(native_center + f32((f32(.019)+f32(.024)) * .5))
assert abs(corrected - first['start'][1]) < .5e-6

print('Verified the actual brake-frame height write and isolated the native shortened-sweep distance', flush=True)
paths = [root / 'geometry.native.json']
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
for name in ('577/summary.json', '575/lateral.native.json.gz', '575/lateral.request.json',
             '575/observer.py', '575/direction_probe.inc', '574/height_probe.inc',
             '573/cached_acceleration.inc'):
    path = root.parent / name
    external.append(dict(path=str(path.relative_to(Path.cwd())), **digest(path.read_bytes())))
result = dict(gate='PASS_BRAKING_FLOOR_QUERY_WRITES_INHERITED_HEIGHT', findings=['BF144-D273','BF144-D125'],
              new_differences=0, total_differences=310, snapshots=snapshots, trajectory=trajectory,
              full_distance_difference=full_difference, short_distance_difference=short_difference,
              native_distance_replay_height=corrected, public_functional_pass=False,
              decompressed_files=decompressed, external_source_files=external)
(root / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
print('Verified compressed evidence identities', flush=True)
