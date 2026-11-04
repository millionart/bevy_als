import gzip
import hashlib
import json
import math
import struct
from pathlib import Path

root = Path(__file__).resolve().parent
prior = root.parent / '555'
labels = ('axis-low', 'diagonal-low', 'diagonal-high', 'zero')


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


assert read(root / 'compile-residual.report.json')['success']
assert read(root / 'residual.native.json.reader.report.json')['success']
assert read(root / 'residual.package.json') == read(prior / 'apex-observed.package.json')

request = read(root / 'residual.request.json')
source_file = root / 'residual.native.json'
frames = {case['name']: case['frames'][49] for case in read(source_file)['character_animation_probe']['cases']}
assert set(frames) == set(labels)
source = {}
target = {}
for label, case in zip(labels, request['cases'], strict=True):
    assert case['name'] == label
    row = frames[label]
    assert row['frame'] == 49 and row['world_delta_seconds'] == f32(.06)
    assert row['gravity_cm_s2'] == -981 and row['blocking_hits'] == []
    assert row['mode_events'] == []
    before, after = (row[f'movement_{phase}_tick'] for phase in ('before', 'after'))
    assert before['movement_mode_name'] == after['movement_mode_name'] == 'MOVE_Falling'
    vx, vz, vy = case['frames'][49]['falling_initial_cm'][3:]
    assert before['velocity_cm_s'] == {'x': vx, 'y': vz, 'z': vy}
    clear_horizontal = vx * vx + vz * vz <= f32(f32(.0001) * 10)
    assert clear_horizontal == (label != 'diagonal-high')
    source_end_velocity = [after['velocity_cm_s'][k] for k in 'xzy']
    assert source_end_velocity[::2] == ([0, 0] if clear_horizontal else [vx, vz])
    dt = row['world_delta_seconds']
    step = f32(dt * .5)
    end = row['actor_transform']['translation']
    displacement_cm = [end['x'] + 1300, end['y'] + 1500]
    consumed = step if clear_horizontal else dt
    assert math.dist(displacement_cm, [vx * consumed, vz * consumed]) < 1e-9
    assert abs(end['z'] - (200 + vy * dt - .5 * 981 * dt**2)) < 1e-6
    assert abs(source_end_velocity[1] - (vy - 981 * dt)) < 1e-5
    source[label] = dict(initial_cm_s=[vx, vy, vz], end_velocity_cm_s=source_end_velocity,
                         horizontal_displacement_cm=displacement_cm, consumed_horizontal_time_s=consumed,
                         position=[end['x'] / 100, 2 + end['z'] / 100, end['y'] / 100])

    functional = read(root / f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images'] == []
    assert load(label, 'process.report')['success']
    start = load(label, 'air-start')
    assert start['mode'] == 'Air' and start['step'] == dt
    assert start['position'] == [-13, 4, -15]
    assert start['velocity'] == [f32(vx / 100), f32(vy / 100), f32(vz / 100)]
    assert start['boost'] == start['desired'] == [0, 0, 0]
    assert start['effective_gravity'] == [0, f32(-9.81), 0]
    transaction = load(label, 'stage-after')
    assert transaction['substeps'] == 2 and transaction['delta'] == dt
    final = load(label, 'contact-after-49')
    assert final['mode'] == transaction['mode'] == 'Air' and final['transitions'] == []
    assert final['position'] == transaction['position']
    assert final['published'] == final['velocity'] == transaction['collision_velocity']
    assert final['published'][::2] == start['velocity'][::2]
    assert abs(final['published'][1] - source_end_velocity[1] / 100) < 1e-6
    assert abs(final['position'][1] - source[label]['position'][1]) < 1e-6
    x, z = start['position'][::2]
    dx, dz = [f32(v * step) for v in start['velocity'][::2]]
    for _ in range(2):
        x = f32(x + dx)
        z = f32(z + dz)
    assert final['position'][::2] == [x, z]
    if label in ('axis-low', 'diagonal-low'):
        assert .000004 < math.dist(final['position'][::2], source[label]['position'][::2]) < .000011
    else:
        assert math.dist(final['position'][::2], source[label]['position'][::2]) < .000001
    target[label] = dict(start=start, transaction=transaction, final=final)

assert len({row['final']['position'][1] for row in target.values()}) == 1
print('Verified native vector threshold and residual consumption in the second target substep', flush=True)

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

external_paths = [prior / name for name in ('apex.request.json', 'apex-observed.package.json', 'apex.inc')]
external_paths.append(root.parent / '523' / 'observer.py')
external = [dict(path=str(p.relative_to(Path.cwd())), **digest(p.read_bytes())) for p in external_paths]
summary = dict(gate='PASS_NATIVE_AIR_RESIDUAL_ZEROES_BETWEEN_SUBSTEPS', supplements=['BF144-D22'],
               new_differences=0, total_differences=303, ordinal=49, native=source, target=target,
               decompressed_files=decompressed, external_source_files=external)
(root / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print('Verified compressed evidence identities', flush=True)
