import gzip
import hashlib
import json
import math
import struct
from pathlib import Path

root = Path(__file__).resolve().parent
labels = ('crouched', 'standing')


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


for name in ('build-speed.report', 'speed.native.json.reader.report', 'compile-speed.report'):
    assert read(root / f'{name}.json')['success']
source_file = root / 'speed.native.json'
cases = {case['name']: case['frames'] for case in read(source_file)['character_animation_probe']['cases']}
assert set(cases) == set(labels)
native = {}
target = {}
for label in labels:
    crouched = label == 'crouched'
    cached_cap = 200 if crouched else 375
    frames = cases[label]
    last_ground = frames[48]['movement_after_tick']
    assert last_ground['movement_mode_name'] == 'MOVE_Walking'
    assert last_ground['capsule_half_cm'] == (56 if crouched else 90)
    assert last_ground['max_speed_cm_s'] == last_ground['max_walk_speed_cm_s'] == cached_cap
    row = frames[49]
    assert row['frame'] == 49 and row['world_delta_seconds'] == f32(.05)
    assert row['gravity_cm_s2'] == -981 and row['blocking_hits'] == [] and row['mode_events'] == []
    before, after = (row[f'movement_{phase}_tick'] for phase in ('before', 'after'))
    assert before['movement_mode_name'] == after['movement_mode_name'] == 'MOVE_Falling'
    assert before['capsule_half_cm'] == last_ground['capsule_half_cm']
    assert after['capsule_half_cm'] == 90
    assert before['max_speed_cm_s'] == after['max_speed_cm_s'] == cached_cap
    assert before['max_walk_speed_cm_s'] == after['max_walk_speed_cm_s'] == cached_cap
    assert before['velocity_cm_s'] == {'x': 200, 'y': 0, 'z': 0}
    vx = after['velocity_cm_s']['x'] / 100
    assert abs(vx - (2 if crouched else 2.15)) < 1e-6
    assert after['velocity_cm_s']['y'] == 0
    source_x = row['actor_transform']['translation']['x'] / 100
    assert abs(source_x + 13 - (2 + vx) * .5 * row['world_delta_seconds']) < 1e-7
    for follow in frames[50:52]:
        values = follow['movement_after_tick']
        assert values['movement_mode_name'] == 'MOVE_Falling'
        assert values['capsule_half_cm'] == 90
        assert values['max_speed_cm_s'] == values['max_walk_speed_cm_s'] == cached_cap
        if crouched:
            assert abs(values['velocity_cm_s']['x'] - 200) < 1e-6
    native[label] = dict(last_ground=last_ground, before=before, after=after, x=source_x)

    functional = read(root / f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images'] == []
    assert load(label, 'process.report')['success']
    report = load(label, 'parameters')
    last, = [f for f in report['dynamics'] if f['ordinal'] == 48]
    assert last['stance'] == ('Crouching' if crouched else 'Standing')
    assert last['max_speed'] == cached_cap / 100
    probe = load(label, 'air-speed')
    assert probe['mode'] == 'Air' and probe['stance'] == 'Als.Stance.Standing'
    assert probe['gait'] == 'Als.Gait.Running' and abs(probe['half_m'] - .9) < 1e-6
    assert probe['initial'] == [2, 0, 0] and probe['step'] == row['world_delta_seconds']
    assert math.dist(probe['desired'], [1, 0, 0]) < 1e-6
    assert probe['acceleration'] == 3.0 and probe['live_cap'] == 3.75
    assert abs(probe['live_speed'][0] - 2.15) < 1e-6 and probe['crouch_cap_speed'][0] == 2
    expected_x = f32(probe['position'][0] + f32((2 + probe['live_speed'][0]) * .5 * probe['step']))
    transaction = load(label, 'stage-after')
    final = load(label, 'contact-after-49')
    assert transaction['substeps'] == 1 and final['mode'] == transaction['mode'] == 'Air'
    assert final['position'] == transaction['position'] and final['transitions'] == []
    assert final['published'] == transaction['collision_velocity']
    assert final['published'][::2] == probe['live_speed'][::2]
    assert abs(final['position'][0] - expected_x) < 1e-6
    assert abs(final['published'][1] - after['velocity_cm_s']['z'] / 100) < 1e-6
    if crouched:
        assert abs(probe['crouch_cap_speed'][0] - vx) < 1e-6
        assert .0037 < final['position'][0] - source_x < .0038
    else:
        assert abs(final['published'][0] - vx) < 1e-6
        assert abs(final['position'][0] - source_x) < 1e-6
    target[label] = dict(last_ground=last, probe=probe, transaction=transaction, actual=final)

assert target['crouched']['actual']['published'] == target['standing']['actual']['published']
print('Verified retained native Falling speed cap after uncrouch and live target speed re-selection', flush=True)

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
    '555/apex.request.json', '555/prepare_reader.py', '556/residual.inc',
    '523/observer.py', '523/native_initial.inc', '553/prepare_reader.py')]
external = [dict(path=str(p.relative_to(Path.cwd())), **digest(p.read_bytes())) for p in external_paths]
summary = dict(gate='PASS_FALLING_REUSES_WALKING_MAX_SPEED_AFTER_UNCROUCH', findings=['BF144-D311'],
               new_differences=1, total_differences=307, ordinal=49, native=native, target=target,
               decompressed_files=decompressed, external_source_files=external)
(root / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print('Verified compressed evidence identities', flush=True)
