import gzip
import hashlib
import json
import math
from pathlib import Path

root = Path(__file__).resolve().parent
labels = ('overlap', 'wide', 'clear')


def raw(path):
    if path.exists():
        return path.read_bytes()
    return gzip.decompress(path.with_suffix('.json.gz').read_bytes())


def read(path):
    return json.loads(raw(path))


def digest(data):
    return dict(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())


def world(v):
    return [v['x'] / 100, 2 + v['z'] / 100, v['y'] / 100]


for name in ('build-recovery.report', 'recovery.native.json.reader.report', 'compile-recovery.report'):
    assert read(root / f'{name}.json')['success']
assert all(result == 'PASS' for result in read(root / 'watchdog-selftest.json').values())
source_path = root / 'recovery.native.json'
cases = {case['name']: case['frames'][40] for case in read(source_path)['character_animation_probe']['cases']}
assert set(cases) == set(labels)
native, target = {}, {}
for label in labels:
    overlap = label == 'overlap'
    row = cases[label]
    before, after = (row[f'movement_{phase}_tick'] for phase in ('before', 'after'))
    assert row['frame'] == 40 and abs(row['world_delta_seconds'] - .05) < 2e-8
    assert before['movement_mode_name'] == after['movement_mode_name'] == 'MOVE_Walking'
    assert before['base_component'] == after['base_component'] == 'BoxComponent_0'
    assert row['actual_current_floor']['walkable'] and not row['mode_events']
    assert before['velocity_cm_s'] == dict(x=650, y=0, z=0)
    assert not before['just_teleported'] and after['just_teleported'] == overlap
    state, = [state for state in row['source_state_after_tick'] if state['name'] == 'LocomotionState']
    fields = {field['name']: field for field in state['fields']}
    source_yaw = fields['VelocityYawAngleWorldSpace']['numeric_value']
    assert source_yaw == fields['TargetYawAngleWorldSpace']['numeric_value'] == 0
    end = world(row['actor_transform']['translation'])
    hits = row['blocking_hits']
    assert not any(hit['penetrating'] for hit in hits)
    velocity = after['velocity_cm_s']['x'] / 100
    native[label] = dict(position=end, forward_velocity=velocity, just_teleported=after['just_teleported'],
                         source_velocity_yaw=source_yaw)

    run = root / f'runtime-{label}'
    functional = read(root / f'{label}-functional.json')
    assert functional['pass'] and functional['images'] == [] and not functional['issues']
    assert read(run / 'process.report.json')['success']
    cast = read(run / 'forward.json')
    start = read(run / 'stage-before.json')
    stage = read(run / 'stage-after.json')
    step = read(run / 'walking-step.json')
    after_physics = read(run / 'contact-before-40.json')
    contact = read(run / 'contact-after-40.json')
    direction = read(run / 'direction.json')
    assert cast['base'] == after_physics['floor']['name'] == 'AuditWalkOffPlatform'
    assert cast['mode'] == start['mode'] == stage['mode'] == contact['mode'] == direction['mode'] == 'Grounded'
    assert contact['has_base'] and not contact['transitions']
    assert cast['actor'] == start['position']
    assert start['relative'] == [6.5, 0, 0]
    assert step['first_step_velocity'][0] == 6.25
    assert start['substeps'] == stage['substeps'] == 1
    assert stage['collision_velocity'] is None
    assert direction['position'] == contact['position']
    assert direction['velocity'] == direction['motion_velocity'] == contact['published']
    assert direction['rotation_mode'] == 'Als.RotationMode.VelocityDirection'
    assert abs(direction['target_yaw'] + math.pi / 2) < 1e-6
    published = contact['published'][0]
    physical_distance = after_physics['position'][0] - stage['position'][0]
    final_distance = contact['position'][0] - stage['position'][0]
    measured = after_physics['position'][0] - start['position'][0]
    assert abs(published - measured / row['world_delta_seconds']) < 2e-6
    if overlap:
        hit, = hits
        assert hit['component'] == 'AuditStuckRight' and hit['time'] == 0
        assert abs(end[0] + 13.50625) < 2e-8
        assert cast['overlaps'] == ['AuditStuckRight'] and cast['used']['distance'] == 0
        assert cast['raw'] == cast['used']
        assert stage['position'] == cast['actor']
        assert -.013 < physical_distance < -.012 and -.026 < final_distance < -.024
        assert velocity == step['first_step_velocity'][0] and -.26 < published < -.24
        assert direction['motion_velocity_yaw'] == 90
        assert abs(direction['response_velocity_yaw'] - math.pi / 2) < 1e-6
    elif label == 'wide':
        assert not cast['overlaps'] and cast['raw'] == cast['used']
        assert hits[0]['time'] > 0 and 0 < cast['used']['distance'] < .025
        assert 0 < published < .25 and 0 < velocity < .25
    else:
        assert not cast['overlaps'] and not hits
        assert cast['raw'] is None and cast['used'] is None
        assert abs(published - velocity) < 2e-5
        assert abs(stage['position'][0] + physical_distance - end[0]) < 2e-6
        assert abs(direction['motion_velocity_yaw'] + 90) < 1e-4
    target[label] = dict(initial_overlaps=cast['overlaps'], actual_yaw=direction['motion_velocity_yaw'],
                         rotation_target_yaw=direction['target_yaw'], position=contact['position'],
                         forward_velocity=published, physics_distance=physical_distance, final_distance=final_distance)

print('Verified backward solver displacement published as actual velocity and direction', flush=True)
paths = [source_path]
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
for name in ('568/observer.py', '566/bases.request.json', '566/observer.py', '553/prepare_reader.py',
             '533/observer.py', '533/walking_probe.rs', '522/contact.rs'):
    path = root.parent / name
    external.append(dict(path=str(path.relative_to(Path.cwd())), **digest(path.read_bytes())))
summary = dict(gate='PASS_WALKING_SUCCESSFUL_DEPENETRATION_ACTUAL_DIRECTION', findings=['BF144-D314'],
               new_differences=0, total_differences=310, ordinal=40, native=native, target=target,
               decompressed_files=decompressed, external_source_files=external)
(root / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print('Verified compressed evidence identities', flush=True)
