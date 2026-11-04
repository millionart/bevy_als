import gzip
import hashlib
import json
import math
import struct
from pathlib import Path

root = Path(__file__).resolve().parent
labels = ('switch', 'same')
old_base = 'AuditWalkOffPlatform'
new_base = 'AuditJumpBaseNew'


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


for name in ('build-jump.report', 'jump.native.json.reader.report', 'compile-jump.report'):
    assert read(root / f'{name}.json')['success']
source_file = root / 'jump.native.json'
cases = {case['name']: case['frames'] for case in read(source_file)['character_animation_probe']['cases']}
assert set(cases) == set(labels)
native = {}
target = {}
for label in labels:
    switched = label == 'switch'
    frames = cases[label]
    row = frames[49]
    assert row['frame'] == 49 and row['world_delta_seconds'] == f32(.05)
    before, after = (row[f'movement_{phase}_tick'] for phase in ('before', 'after'))
    old = frames[48]['movement_after_tick']
    assert old['movement_mode_name'] == before['movement_mode_name'] == 'MOVE_Walking'
    assert old['base_component'] == before['base_component'] == 'BoxComponent_0'
    assert old['velocity_cm_s'] == before['velocity_cm_s'] == {'x': 0, 'y': 0, 'z': 0}
    assert before['old_box_velocity_cm_s'] == {'x': 100, 'y': 0, 'z': 0}
    assert before['new_box_velocity_cm_s'] == {'x': -100, 'y': 0, 'z': 0}
    assert after['movement_mode_name'] == 'MOVE_Falling' and after['base_component'] == 'None'
    event, = row['mode_events']
    assert [event['previous'], event['current']] == ['MOVE_Walking', 'MOVE_Falling']
    assert event['velocity_cm_s'] == {'x': 100, 'y': 0, 'z': 420}
    assert after['velocity_cm_s']['x'] == 100
    assert abs(after['velocity_cm_s']['z'] - 370.95) < .0001
    end_x = row['actor_transform']['translation']['x']
    assert abs(end_x - event['location_cm']['x'] - 5) < .0001
    assert row['blocking_hits'] == []
    native[label] = dict(old=old, before=before, event=event, after=after, end_x_cm=end_x)

    functional = read(root / f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images'] == []
    assert load(label, 'process.report')['success']
    before_target = load(label, 'jump-base-before')
    assert before_target['mode'] == 'Grounded' and before_target['switched'] == switched
    assert before_target['base'] == before_target['dynamics_base'] == old_base
    assert before_target['previous_support'] == [0, 0, 0]
    platforms = before_target['platforms']
    assert platforms[old_base]['velocity'] == [1, 0, 0] and platforms[new_base]['velocity'] == [-1, 0, 0]
    assert (platforms[new_base]['position'][1] > platforms[old_base]['position'][1]) == switched
    motor = load(label, 'jump-motor')
    sensor = new_base if switched else old_base
    expected_x = -1 if switched else 1
    assert motor['mode'] == 'Air' and motor['velocity'] == [0, 0, 0]
    assert motor['memory'] == motor['sensor']['entity'] == sensor
    assert motor['sensor']['velocity'] == [expected_x, 0, 0]
    assert motor['boost'] == [expected_x, f32(4.2), 0]
    assert motor['acceleration'] == [0, 0, 0]
    result = load(label, 'contact-after-49')
    txn = load(label, 'stage-after')
    assert result['mode'] == txn['mode'] == 'Air' and not result['has_base']
    assert result['transitions'] == [['LocomotionState(Grounded)', 'LocomotionState(Air)']]
    assert result['position'] == txn['position']
    assert result['published'] == txn['collision_velocity']
    assert result['published'][0] == expected_x
    assert abs(result['published'][1] - after['velocity_cm_s']['z'] / 100) < 1e-6
    assert abs(result['position'][0] - motor['position'][0] - .05 * expected_x) < 1e-6
    if switched:
        assert .099 < end_x / 100 - result['position'][0] < .101
    else:
        assert abs(end_x / 100 - result['position'][0]) < 1e-6
    target[label] = dict(before=before_target, motor=motor, actual=result)

assert native['switch']['event']['velocity_cm_s'] == native['same']['event']['velocity_cm_s']
assert target['switch']['motor']['position'] == target['same']['motor']['position']
print('Verified native previous-base inheritance and actual target sensor-owner reversal', flush=True)

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
    '564/speed.request.json', '553/prepare_reader.py', '523/observer.py')]
external = [dict(path=str(p.relative_to(Path.cwd())), **digest(p.read_bytes())) for p in external_paths]
summary = dict(gate='PASS_JUMP_INHERITS_OLD_BASE_NOT_CURRENT_SENSOR', findings=['BF144-D312'],
               new_differences=1, total_differences=308, ordinal=49, native=native, target=target,
               decompressed_files=decompressed, external_source_files=external)
(root / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print('Verified compressed evidence identities', flush=True)
