import gzip
import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parent
labels = ('stuck', 'wide', 'clear')


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


for name in ('build-stuck.report', 'stuck.native.json.reader.report', 'compile-stuck.report'):
    assert read(root / f'{name}.json')['success']
warning_log = root / 'stuck.native.json.reader.stdout.log'
if warning_log.exists():
    prefix = 'LogCharacterMovement: '
    messages = [line[line.index(prefix):] for line in warning_log.read_text().splitlines()
                if prefix in line and 'is stuck and failed to move!' in line]
    assert len(messages) == 1
    (root / 'stuck-warning.json').write_text(json.dumps(dict(messages=messages), indent=2) + '\n')
warning, = read(root / 'stuck-warning.json')['messages']
assert 'is stuck and failed to move! Velocity: X=625.00' in warning
assert 'PenetrationDepth:2.500' in warning and 'Component:AuditStuckRight' in warning
source_path = root / 'stuck.native.json'
cases = {case['name']: case['frames'][40] for case in read(source_path)['character_animation_probe']['cases']}
assert set(cases) == set(labels)
native, target = {}, {}
for label in labels:
    stuck = label == 'stuck'
    row = cases[label]
    before, after = (row[f'movement_{phase}_tick'] for phase in ('before', 'after'))
    assert row['frame'] == 40 and abs(row['world_delta_seconds'] - .1) < 2e-8
    assert before['movement_mode_name'] == after['movement_mode_name'] == 'MOVE_Walking'
    assert before['base_component'] == after['base_component'] == 'BoxComponent_0'
    assert row['actual_current_floor']['walkable'] and not row['mode_events']
    assert before['velocity_cm_s'] == dict(x=650, y=0, z=0)
    assert not before['just_teleported'] and after['just_teleported'] == stuck
    end = world(row['actor_transform']['translation'])
    hits = row['blocking_hits']
    velocity = after['velocity_cm_s']['x'] / 100
    native[label] = dict(position=end, forward_velocity=velocity, just_teleported=after['just_teleported'])

    run = root / f'runtime-{label}'
    functional = read(root / f'{label}-functional.json')
    assert functional['pass'] and functional['images'] == [] and not functional['issues']
    assert read(run / 'process.report.json')['success']
    cast = read(run / 'forward.json')
    start = read(run / 'stage-before.json')
    stage = read(run / 'stage-after.json')
    step = read(run / 'walking-step.json')
    txn = read(run / 'walking-transaction.json')
    contact = read(run / 'contact-after-40.json')
    assert cast['base'] == txn['end_floor']['variants']['none']['result']['entity'] == 'AuditWalkOffPlatform'
    assert cast['mode'] == start['mode'] == stage['mode'] == txn['mode'] == contact['mode'] == 'Grounded'
    assert contact['has_base'] and not contact['transitions']
    assert cast['actor'] == start['position']
    assert start['relative'] == [6.5, 0, 0]
    assert step['first_step_velocity'][0] == 6.25
    assert start['substeps'] == stage['substeps'] == 2
    assert stage['position'] == txn['position'] == contact['position']
    assert stage['collision_velocity'] == txn['collision_velocity'] == contact['published']
    published = contact['published'][0]
    if stuck:
        assert {hit['component'] for hit in hits} == {'AuditStuckLeft', 'AuditStuckRight'}
        assert all(hit['penetrating'] and hit['time'] == 0 for hit in hits)
        assert cast['overlaps'] == ['AuditStuckLeft', 'AuditStuckRight']
        assert cast['raw']['distance'] == cast['used']['distance'] == 0
        assert cast['used']['entity'] == 'AuditStuckRight'
        assert velocity == step['first_step_velocity'][0] and published == 0
        assert stage['position'] == cast['actor']
        assert abs(stage['position'][0] - end[0]) < 2e-6
        assert abs(stage['position'][1] - end[1]) < 2e-6
    elif label == 'wide':
        assert not cast['overlaps'] and cast['raw'] == cast['used']
        assert hits and not any(hit['penetrating'] for hit in hits)
        assert 0 < cast['used']['distance'] < .025
        assert 0 < published < .25 and 0 < velocity < .25
    else:
        assert not cast['overlaps'] and not hits
        assert cast['raw'] is None and cast['used'] is None
        assert abs(published - velocity) < 5e-6
        assert abs(stage['position'][0] - end[0]) < 2e-6
    target[label] = dict(initial_overlaps=cast['overlaps'], selected_forward_cast=cast['used'],
                         first_step_input=step['first_step_velocity'][0], position=contact['position'],
                         forward_velocity=published)

print('Verified retained native planned speed after failed Walking depenetration', flush=True)
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
for name in ('566/bases.request.json', '566/observer.py', '553/prepare_reader.py',
             '533/observer.py', '533/walking_probe.rs', '522/contact.rs'):
    path = root.parent / name
    external.append(dict(path=str(path.relative_to(Path.cwd())), **digest(path.read_bytes())))
summary = dict(gate='PASS_WALKING_STUCK_RETAINS_PLANNED_VELOCITY', findings=['BF144-D314'],
               new_differences=1, total_differences=310, ordinal=40, native=native, target=target,
               decompressed_files=decompressed, external_source_files=external)
(root / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print('Verified compressed evidence identities', flush=True)
