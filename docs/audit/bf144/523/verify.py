import gzip
import hashlib
import json
import math
import os
from pathlib import Path

root = Path(__file__).resolve().parent
prior = root.parent / '522'
labels = ('wall-deck', 'wall-only', 'cleared', 'restored', 'restored-repeat')
frames = (49, 50)

def raw(path):
    return path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix('.json.gz').read_bytes())

def read(path):
    return json.loads(raw(path))

def identity(data):
    return dict(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())

def save(name, result):
    (root / name).write_text(json.dumps(result, indent=2) + '\n')

def file(label, name):
    return root / f'runtime-{label}' / f'{name}.json'

def load(label, name):
    return read(file(label, name))

initial = read(root / 'initial.json')
source_request = read(root / 'wall.request.json')['cases']
native_path = root / 'wall.native.json'
native_cases = read(native_path)['character_animation_probe']['cases']
assert read(root / 'build-deck.report.json')['success']
assert read(root / 'wall.native.json.reader.report.json')['success']
native = {}
for request, case in zip(source_request, native_cases, strict=True):
    assert request['name'] == case['name']
    q49 = case['frames'][49]
    before = q49['movement_before_tick']
    assert before['movement_mode_name'] == 'MOVE_Falling'
    assert [before['velocity_cm_s'][axis] for axis in ('x','y','z')] == request['frames'][49]['falling_initial_cm'][3:]
    assert q49['world_delta_seconds'] == initial['delta_s']
    native[case['name']] = [
        dict(mode=row['movement_after_tick']['movement_mode_name'],
            floor=row['movement_after_tick']['walkable_floor'],
            floor_cm=row['movement_after_tick']['floor_distance_cm'],
            velocity_cm_s=row['movement_after_tick']['velocity_cm_s'],
            position_cm=row['actor_transform']['translation'])
        for row in (case['frames'][frame] for frame in frames)
    ]
deck, wall_only = native['wall-deck'], native['wall-only']
assert deck[0] == wall_only[0] and deck[0]['mode'] == 'MOVE_Falling' and not deck[0]['floor']
assert deck[1]['mode'] == 'MOVE_Walking' and deck[1]['floor']
assert wall_only[1]['mode'] == 'MOVE_Falling' and not wall_only[1]['floor']
native_q49_gap = deck[0]['position_cm']['z'] / 100 + 2.0 - .9 - initial['deck_top_m']
assert .013 < native_q49_gap < .014
assert wall_only[1]['position_cm']['z'] / 100 + 2.0 - .9 < initial['deck_top_m']

records = {}
parameters = {}
first = {}
for label in labels:
    functional = read(root / f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images'] == []
    assert load(label, 'process.report')['success']
    records[label] = {
        f'{stage}-{frame}': load(label, f'contact-{stage}-{frame}')
        for frame in frames for stage in ('before', 'after')
    }
    first[label] = load(label, 'initial-observed')
    row = first[label]
    assert row['position'] == initial['position_m']
    assert row['velocity'] == row['initial_with_motor'] == initial['velocity_mps']
    assert row['mode'] == 'Air' and row['step'] == initial['delta_s']
    assert row['wall'] == 'AuditWalkOffBarrier' and row['second_hit'] is None
    assert 0 < row['normal'][1] < .0001
    before = records[label]['before-49']
    assert before['position'] == row['single_hit_position']
    assert before['planned_velocity'] == row['single_hit_velocity']
    assert before['mode'] == 'Air' and not before['has_basis_displacement']
    assert before['manifolds'] == [] and before['requested_mode'] is None
    assert before['generated_contact']
    parameters[label] = read(file(label, 'parameters'))

assert all(row == first['wall-deck'] for row in first.values())
base = records['wall-deck']
clear = records['cleared']
assert records['restored'] == records['restored-repeat'] == base
assert base['before-49'] == (clear['before-49'] | {'consumed_contact': True})
assert not clear['before-49']['consumed_contact']
assert records['wall-only']['before-49'] == (base['before-49'] | {'floor': None})
floor = base['before-49']['floor']
assert floor['name'] == 'AuditLandingDeck'
assert .013 < floor['gap'] < .014 and floor['impact'] < floor['lower_hemisphere']
assert math.isclose(floor['impact'], initial['deck_top_m'], abs_tol=1e-6)
after, removed = base['after-49'], clear['after-49']
assert after['position'] == removed['position'] == base['before-49']['position']
assert after['mode'] == 'Grounded' and after['has_base'] and after['velocity'][1] == 0
assert after['transitions'] == [['LocomotionState(Air)', 'LocomotionState(Grounded)']]
assert removed['mode'] == 'Air' and not removed['has_base'] and removed['transitions'] == []
assert removed['velocity'] == removed['published'] == base['before-49']['planned_velocity']
assert removed == records['wall-only']['after-49']
assert clear['after-50']['mode'] == 'Grounded' and clear['after-50']['transitions'] == after['transitions']
assert clear['before-50']['mode'] == 'Grounded' and clear['before-50']['generated_contact']
assert clear['before-50']['floor']['name'] == 'AuditLandingDeck'
assert records['wall-only']['after-50']['mode'] == 'Air'

for label, table in parameters.items():
    rows = {row['ordinal']: row for row in table['after']}
    for frame in frames:
        row = rows[frame]
        assert row['gameplay'] == records[label][f'after-{frame}']['mode']
        assert row['graph_tag'] == 'Als.LocomotionMode.' + ('InAir' if row['gameplay'] == 'Air' else 'Grounded')
        assert row['graph_error'] is None
def first_parameter_difference(left, right):
    first_difference = {}
    for key in left:
        rows = [{row['ordinal']: row for row in report[key]} for report in (left, right)]
        for ordinal in sorted(rows[0].keys() | rows[1].keys()):
            a, b = [group.get(ordinal, {}) for group in rows]
            if a != b:
                first_difference[key] = dict(ordinal=ordinal, fields=sorted(name for name in
                    a.keys() | b.keys() if a.get(name) != b.get(name)))
                break
    return first_difference

restoration = {label: first_parameter_difference(parameters['wall-deck'], parameters[label])
               for label in ('restored', 'restored-repeat')}
restoration['same_binary'] = first_parameter_difference(parameters['restored'], parameters['restored-repeat'])
assert all(row['ordinal'] > max(frames) for phase in restoration.values() for row in phase.values())


decoded = {}
paths = [native_path] + [file(label, name) for label in labels for name in ('parameters', 'report')]
for path in paths:
    data = raw(path)
    archive = path.with_suffix('.json.gz')
    archive.write_bytes(gzip.compress(data, compresslevel=6, mtime=0))
    assert gzip.decompress(archive.read_bytes()) == data
    decoded[str(path.relative_to(root))] = identity(data)
    if path.exists():
        path.unlink()
    print('Verified gzip ' + str(path.relative_to(root)), flush=True)

ue = Path(os.environ['ALS_UE_SOURCE_ROOT'])
cmc = 'Engine/Source/Runtime/Engine/Private/Components/CharacterMovementComponent.cpp'
header = 'Engine/Source/Runtime/Core/Public/Math/UnrealMathUtility.h'
motion = 'src/als/character/movement/motion.rs'
facts = []
for key, path, ranges in (
    (cmc, ue / cmc, [(5047, 5083), (6983, 7016), (7374, 7442)]),
    (header, ue / header, [(128, 132)]),
    (motion, Path(motion), [(2723, 2747), (2923, 2934), (3979, 4008), (4070, 4124)]),
    ('src/als/settings/movement_constants.rs', Path('src/als/settings/movement_constants.rs'), [(1, 5)]),
):
    data = path.read_bytes()
    lines = data.decode().splitlines()
    facts.append(dict(key=key, **identity(data), excerpts=[
        dict(start=a, end=b, text='\n'.join(lines[a-1:b]) + '\n') for a,b in ranges]))
save('source-facts.json', facts)

inputs = {row['path'] for row in read(prior / 'summary.json')['external_source_files']}
inputs.update(str((prior / 'runtime-baseline' / name).relative_to(Path.cwd())) for name in
              ('rig-definition.json', 'skeleton.json', 'profile.json', 'state-reader-selftest.json'))
inputs.update(str(path.relative_to(Path.cwd())) for path in (
    prior / 'contact.rs', prior / 'observer.py', prior / 'summary.json',
    root.parent / '519' / 'prepare_reader.py', root.parent / '519' / 'native_wall.inc',
    root.parent / '516' / 'prepare_reader.py', root.parent / '520' / 'wall.request.json',
    root.parent / '521' / 'runtime-wall-162-aligned-air/alignment.json',
    root.parent / '521' / 'runtime-wall-162-aligned-air/fixture.json',
))
save('summary.json', dict(
    gate='PASS_WALL_CONTACT_EARLY_GROUNDED_CAUSAL_ABLATION', ordinals=list(frames),
    related_difference='BF144-D290', new_differences=0, total_differences=287,
    initial=initial, single_hit=first['wall-deck'], native=native, native_q49_deck_gap_m=native_q49_gap,
    target=records, focused_restoration_exact=True, first_late_parameter_differences=restoration,
    decoded=decoded, screenshots=0,
    performance_acceptance='paused',
    external_source_files=[dict(path=name, **identity(Path(name).read_bytes())) for name in sorted(inputs)]))
print('Verified the isolated wall-contact landing and one-frame timing difference', flush=True)
