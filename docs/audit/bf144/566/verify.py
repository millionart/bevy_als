import gzip
import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parent
labels = ('moving', 'still')
new_base = 'AuditWalkOffSecondBase'
old_base = 'AuditWalkOffPlatform'


def raw(path):
    if path.exists():
        return path.read_bytes()
    return gzip.decompress(path.with_suffix('.json.gz').read_bytes())


def read(path):
    return json.loads(raw(path))


def digest(data):
    return dict(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())


def world(vector):
    return [vector['x'] / 100, 2 + vector['z'] / 100, vector['y'] / 100]


def nearby(a, b):
    assert all(abs(x - y) < 2e-6 for x, y in zip(a, b))


for name in ('build-bases.report', 'bases.native.json.reader.report', 'compile-bases.report'):
    assert read(root / f'{name}.json')['success']
native_path = root / 'bases.native.json'
cases = {c['name']: c['frames'][40] for c in read(native_path)['character_animation_probe']['cases']}
assert set(cases) == {'first-step', *labels}
first = cases['first-step']
assert first['movement_after_tick']['base_component'] == new_base
assert first['movement_after_tick']['movement_mode_name'] == 'MOVE_Walking'
assert first['movement_after_tick']['velocity_cm_s'] == dict(x=625, y=0, z=0)
assert first['actual_current_floor']['walkable']
assert first['actual_current_floor']['hit']['component'] == new_base
assert first['mode_events'] == []

native, target = {}, {}
for label, side_speed in (('moving', 1), ('still', 0)):
    row = cases[label]
    assert row['frame'] == first['frame'] == 40
    assert row['world_delta_seconds'] == 2 * first['world_delta_seconds']
    before, after = (row[f'movement_{phase}_tick'] for phase in ('before', 'after'))
    assert before['base_component'] == first['movement_before_tick']['base_component'] == 'BoxComponent_0'
    assert before['movement_mode_name'] == 'MOVE_Walking'
    assert before['velocity_cm_s'] == first['movement_before_tick']['velocity_cm_s'] == dict(x=650, y=0, z=0)
    assert before['old_box_velocity_cm_s'] == dict(x=0, y=0, z=0)
    assert before['new_box_velocity_cm_s'] == dict(x=0, y=100 * side_speed, z=0)
    assert after['movement_mode_name'] == 'MOVE_Falling' and after['base_component'] == 'None'
    event, = row['mode_events']
    assert (event['previous'], event['current']) == ('MOVE_Walking', 'MOVE_Falling')
    assert event['velocity_cm_s'] == after['velocity_cm_s'] == dict(x=600, y=100 * side_speed, z=0)
    assert event['location_cm'] == row['actor_transform']['translation']
    assert not event['walking_floor']['walkable'] and not row['blocking_hits']
    native[label] = dict(old_base=before['base_component'], new_base_velocity_cm_s=before['new_box_velocity_cm_s'],
                         transition_velocity_cm_s=event['velocity_cm_s'], position=world(event['location_cm']))

    functional = read(root / f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images'] == []
    runtime = root / f'runtime-{label}'
    assert read(runtime / 'process.report.json')['success']
    second = read(runtime / 'second-base.json')
    assert second['old'] == old_base and second['velocity'] == [0, 0, side_speed]
    step = read(runtime / 'walking-step.json')
    assert step['delta_s'] == row['world_delta_seconds'] and step['step_s'] == first['world_delta_seconds']
    assert step['start_floor']['variants']['none']['result']['entity'] == old_base
    assert step['first_floor']['variants']['none']['result']['entity'] == new_base
    first_pos = world(first['actor_transform']['translation'])
    origin = step['first_floor']['origin']
    nearby([origin[0], origin[2]], [first_pos[0], first_pos[2]])
    assert step['first_step_velocity'][0] == 6.25
    start = read(runtime / 'stage-before.json')
    stage = read(runtime / 'stage-after.json')
    txn = read(runtime / 'walking-transaction.json')
    actual = read(runtime / 'contact-after-40.json')
    assert start['mode'] == 'Grounded' and start['support'] == [0, 0, 0]
    assert start['substeps'] == stage['substeps'] == 2
    assert txn['end_floor']['variants']['none']['result'] is None
    assert actual['mode'] == stage['mode'] == txn['mode'] == 'Air' and not actual['has_base']
    assert actual['transitions'] == [['LocomotionState(Grounded)', 'LocomotionState(Air)']]
    assert actual['position'] == stage['position'] == txn['position']
    nearby(actual['position'], native[label]['position'])
    assert actual['published'] == stage['collision_velocity'] == txn['collision_velocity']
    nearby(actual['published'], [6, 0, 0])
    target[label] = dict(initial_support=start['support'], first_floor_entity=new_base,
                         first_floor_query=origin, position=actual['position'], published=actual['published'])

assert native['moving']['position'] == native['still']['position']
assert target['moving'] == target['still']
print('Verified native switch to the second base and loss of its velocity in the target', flush=True)

paths = [native_path]
paths.extend(root / f'runtime-{label}' / f'{name}.json' for label in labels
             for name in ('parameters', 'report', 'walking-step', 'walking-transaction'))
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
for name in ('551/stall.request.json', '533/observer.py', '533/walking_probe.rs', '553/prepare_reader.py'):
    path = root.parent / name
    external.append(dict(path=str(path.relative_to(Path.cwd())), **digest(path.read_bytes())))
summary = dict(gate='PASS_WALKING_SWITCHED_BASE_AT_SECOND_STEP_DEPARTURE', findings=['BF144-D313'],
               new_differences=1, total_differences=309, ordinal=40, native=native, target=target,
               native_first_step=dict(base=new_base, position=world(first['actor_transform']['translation'])),
               decompressed_files=decompressed, external_source_files=external)
(root / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print('Verified compressed evidence identities', flush=True)
