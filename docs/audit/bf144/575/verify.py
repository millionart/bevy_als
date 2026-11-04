import gzip
import hashlib
import json
import math
from pathlib import Path

root = Path(__file__).resolve().parent
labels = ('natural', 'axial', 'positive', 'negative')
side = 1/1024
signed = dict(positive=side, negative=-side)


def raw(path):
    if path.exists():
        return path.read_bytes()
    return gzip.decompress(path.with_suffix('.json.gz').read_bytes())


def read(path):
    return json.loads(raw(path))


def digest(data):
    return dict(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())


assert read(root / 'compile-lateral.report.json')['success']
assert read(root / 'lateral.native.json.reader.report.json')['success']
assert all(value == 'PASS' for value in read(root / 'watchdog-selftest.json').values())
source_path = root / 'lateral.native.json'
native = {case['name']: case['frames'] for case in read(source_path)['character_animation_probe']['cases']}
assert set(native) == set(labels) - {'natural'}
for label, frames in native.items():
    assert all(frames[frame]['world_delta_seconds'] == .125 for frame in range(52, 59))
    assert frames[53]['movement_after_tick']['velocity_cm_s']['y'] == 0
    assert frames[54]['movement_before_tick']['movement_mode_name'] == 'MOVE_Walking'
    assert frames[54]['movement_after_tick']['movement_mode_name'] == 'MOVE_Falling'
    assert not frames[54]['blocking_hits']
    assert frames[58]['landed_events'][0]['component'] == 'AuditLowerFloor'
    assert frames[58]['movement_after_tick']['movement_mode_name'] == 'MOVE_Walking'
    assert frames[54]['movement_after_tick']['last_input_world']['y'] == signed.get(label, 0)
    assert all(frames[frame]['movement_after_tick']['last_input_world']['y'] == 0
               for frame in range(55, 59))

samples, residual = {}, {}
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
    rows = read(path / 'parameters.json')
    actual = {row['ordinal']: row for row in rows['after']}
    dynamics = {row['ordinal']: row for row in rows['dynamics']}
    initial_side = actual[53]['position'][2]
    assert actual[53]['velocity'][2] == 0
    cache = read(path / 'mixed-before.json')
    assert cache['walking_acceleration'] == cache['cached_acceleration'] == 20
    assert read(path / 'height-transaction-52.json')['control']
    records = []
    for frame in range(52, 59):
        direction = read(path / f'direction-{frame}.json')
        assert direction['frame'] == frame
        assert dynamics[frame]['motion'] == direction['submitted']
        assert actual[frame]['gameplay'] == ('Air' if 54 <= frame <= 57 else 'Grounded')
        displacement = actual[frame]['position'][2] - initial_side
        speed = actual[frame]['velocity'][2]
        if label == 'natural':
            assert direction['submitted'] == direction['natural']
            angle = direction['published_yaw']
            assert angle is not None
            assert abs(direction['natural'][2] + math.cos(angle)) < 2e-7
            if frame != 54:
                assert abs(direction['natural'][2]) < 1e-6
            record = dict(frame=frame, direction=direction, position=displacement, speed=speed)
        else:
            z = signed.get(label, 0) if frame == 54 else 0
            assert direction['submitted'] == ([1 - 2**-21, 0, z] if z else [1, 0, 0])
            src = native[label][frame]
            source_position = (src['actor_transform']['translation']['y'] + 1530) / 100
            source_speed = src['movement_after_tick']['velocity_cm_s']['y'] / 100
            assert abs(displacement - source_position) < 3e-6
            if frame <= 57:
                assert abs(speed - source_speed) < 1e-8
            if label == 'axial':
                assert displacement == speed == source_position == source_speed == 0
            elif frame == 58:
                residual[label] = speed - source_speed
                assert 2e-5 < abs(residual[label]) < 9e-5
            record = dict(frame=frame, direction=direction, position=displacement, speed=speed,
                          source_position=source_position, source_speed=source_speed)
        records.append(record)
    samples[label] = records

q54 = samples['natural'][2]
input_side = q54['direction']['natural'][2]
assert abs(input_side) > 1e-5
assert abs(q54['direction']['view_yaw'] + math.pi / 2) < 1e-7
reference = samples['positive']
assert abs(q54['speed'] - reference[2]['source_speed'] * input_side / side) < 2e-7
natural_landing = samples['natural'][-1]['position']
assert abs(natural_landing - reference[-1]['source_position'] * input_side / side) < 5e-6
for a, b in zip(samples['positive'][2:6], samples['negative'][2:6]):
    assert a['speed'] == -b['speed'] and a['position'] == -b['position']

print('Verified the natural lateral offset and equal signed Falling response under common world input', flush=True)
paths = [source_path]
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
for name in ('572/pelvis.request.json', '572/pelvis.package.json', '572/evidence-manifest.json',
             '574/observer.py', '574/height_probe.inc', '574/summary.json', '573/cached_acceleration.inc'):
    path = root.parent / name
    external.append(dict(path=str(path.relative_to(Path.cwd())), **digest(path.read_bytes())))
summary = dict(gate='PASS_COMMON_WORLD_INPUT_ISOLATES_RUNOFF_LATERAL_DRIFT', findings=[],
               new_differences=0, total_differences=310, samples=samples, landing_velocity_residual=residual,
               public_functional_pass=False, decompressed_files=decompressed, external_source_files=external)
(root / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print('Verified compressed evidence identities', flush=True)
