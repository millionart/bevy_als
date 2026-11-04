import gzip
import hashlib
import json
import math
from pathlib import Path

root = Path(__file__).resolve().parent
prior = root.parent / '553'
labels = ('beyond', 'beyond-adjusted', 'beyond-restored')


def raw(path):
    if not path.exists():
        return gzip.decompress(path.with_suffix('.json.gz').read_bytes())
    return gzip.decompress(path.read_bytes()) if path.suffix == '.gz' else path.read_bytes()


def read(path):
    return json.loads(raw(path))


def load(label, name):
    return read(root / f'runtime-{label}' / f'{name}.json')


def digest(data):
    return dict(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())


def world(value):
    return [value['x'] / 100, 2 + value['z'] / 100, value['y'] / 100]


def frame(report, ordinal):
    row, = [row for row in report['after'] if row['ordinal'] == ordinal]
    return row


for name in ('compile-edge.report', 'departure.native.json.reader.report'):
    assert read(root / f'{name}.json')['success']
assert read(root / 'departure.package.json') == read(prior / 'lower.package.json')

source_file = root / 'departure.native.json'
case, = read(source_file)['character_animation_probe']['cases']
old = next(c for c in read(prior / 'lower.native.json.gz')['character_animation_probe']['cases'] if c['name'] == 'beyond')
assert case['name'] == 'beyond'
assert len(case['frames']) == 51
for old_row, row in zip(old['frames'], case['frames']):
    for key in ('actor_transform', 'movement_before_tick', 'movement_after_tick', 'controller_input'):
        assert old_row[key] == row[key]
source = {}
for q in range(40, 50):
    row = case['frames'][q]
    after = row['movement_after_tick']
    state = after['movement_mode_name']
    position = world(row['actor_transform']['translation'])
    velocity = after['velocity_cm_s']
    if q < 48:
        assert state == 'MOVE_Walking' and after['floor_component'] == 'AuditLandingDeck'
        assert abs(after['floor_distance_cm'] - 2.15) < .0001
    else:
        assert state == 'MOVE_Falling' and after['floor_component'] == 'None'
    if q <= 48:
        assert velocity['z'] == 0
        assert position[1] == world(case['frames'][40]['actor_transform']['translation'])[1]
    else:
        assert abs(velocity['z'] + 32.7) < .0001
    if q == 48:
        event, = row['mode_events']
        assert [event['previous'], event['current']] == ['MOVE_Walking', 'MOVE_Falling']
        assert event['location_cm'] == row['actor_transform']['translation'] and event['velocity_cm_s'] == velocity
    else:
        assert row['mode_events'] == []
    source[q] = dict(mode=state, position=position, velocity_cm_s=velocity)

target = {}
for label in labels:
    adjusted = label == 'beyond-adjusted'
    functional = read(root / f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images'] == []
    assert load(label, 'process.report')['success']
    assert load(label, 'floor-adjustment') == read(prior / f'runtime-{label}/floor-adjustment.json')
    report = load(label, 'parameters')
    for q in range(40, 48):
        after = load(label, f'contact-after-{q}')
        before = load(label, f'contact-before-{q}')
        assert before['floor']['name'] == 'AuditLandingDeck'
        assert after['mode'] == 'Grounded' and after['has_base'] and after['transitions'] == []
        assert after['published'][1] == 0
        if adjusted:
            assert abs(after['position'][1] - source[q]['position'][1]) < .003
        else:
            assert after['position'][1] == load(label, 'contact-before-40')['position'][1]
    edges = {}
    for q in (47, 48):
        before = load(label, f'edge-before-{q}')
        after = load(label, f'edge-after-{q}')
        assert before['base'] == before['sensor_support'] == 'AuditLandingDeck'
        assert before['velocity'][1] == after['velocity'][1] == 0
        assert before['displacement'] is not None and before['spring_strength'] == 400
        assert abs(before['acceleration'][1] - (9.81 - before['displacement'][1] * before['spring_strength'])) < .0001
        expect_candidate = q == 47 or adjusted
        assert (before['next_y'] is not None) == expect_candidate
        if expect_candidate:
            assert after['acceleration'][1] == 9.8100004196167 and after['boost'][1] == 0
        else:
            assert before['displacement'][1] > .447
            assert before['acceleration'][1] == after['acceleration'][1]
            assert before['boost'][1] == after['boost'][1]
        edges[q] = dict(before=before, after=after)

    before48 = load(label, 'contact-before-48')
    after48 = load(label, 'contact-after-48')
    assert before48['floor'] is None and before48['has_basis_displacement']
    assert before48['mode'] == 'Grounded' and after48['mode'] == 'Air' and not after48['has_base']
    assert after48['transitions'] == [['LocomotionState(Grounded)', 'LocomotionState(Air)']]
    assert before48['raw_velocity'][1] == after48['published'][1] == frame(report, 48)['velocity'][1]
    assert abs(after48['position'][0] - source[48]['position'][0]) < 1e-5
    assert not before48['manifolds']
    motor = edges[48]['after']
    dt = case['frames'][48]['world_delta_seconds']
    predicted = motor['velocity'][1] + motor['boost'][1] + (motor['acceleration'][1] - 9.8100004196167) * dt
    assert abs(predicted - after48['published'][1]) < .00002
    if adjusted:
        assert after48['published'][1] == 0
        assert frame(report, 49)['velocity'][1] < 0
    else:
        assert -5.974 < after48['published'][1] < -5.972
    target[label] = dict(edges=edges, before48=before48, after48=after48)

assert abs(target['beyond']['after48']['published'][1] - target['beyond-restored']['after48']['published'][1]) < .00003
base_pos = target['beyond']['after48']['position']
restored_pos = target['beyond-restored']['after48']['position']
assert base_pos[0] == restored_pos[0]
assert abs(base_pos[1] - restored_pos[1]) < 1e-6
assert abs(base_pos[2] - restored_pos[2]) < .001
print('Verified low-deck base retention and motor-to-physics onset of the departure spring', flush=True)

paths = [source_file]
paths.extend(root / f'runtime-{label}' / f'{name}.json' for label in labels for name in (
    'parameters', 'report', 'walking-step', 'walking-transaction', 'step-down-replay'))
decompressed = {}
for path in paths:
    data = raw(path)
    archive = path.with_suffix('.json.gz')
    if path.exists():
        archive.write_bytes(gzip.compress(data, compresslevel=6, mtime=0))
    assert gzip.decompress(archive.read_bytes()) == data
    decompressed[str(archive.relative_to(root))] = digest(data)
    path.unlink(missing_ok=True)

external_paths = [prior / name for name in (
    'lower.request.json', 'lower.native.json.gz', 'lower.package.json', 'observer.py', 'adjust_probe.inc',
    *(f'runtime-{label}/floor-adjustment.json' for label in labels))]
external = [dict(path=str(p.relative_to(Path.cwd())), **digest(p.read_bytes())) for p in external_paths]
summary = dict(gate='PASS_D306_SUSPENDED_FLOOR_SPRING_AT_DEPARTURE', supplements=['BF144-D306', 'BF144-D288'],
               new_differences=0, total_differences=303, ordinal=48, native=source, target=target,
               decompressed_files=decompressed, external_source_files=external)
(root / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print('Verified compressed evidence identities', flush=True)
