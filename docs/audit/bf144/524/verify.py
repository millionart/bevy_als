import gzip
import hashlib
import json
import math
import os
from pathlib import Path

root = Path(__file__).resolve().parent
prior = root.parent / '523'
labels = ('positive', 'negative', 'cleared', 'restored')
frames = (49, 50)
threshold = 1e-4

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

initial = read(prior / 'initial.json')
pitches = {case['name']: case['wall_pitch_degrees'] for case in read(root / 'wall.request.json')['cases']}
assert pitches == {'positive': -.003, 'negative': .003}
request = read(root / 'geometry.request.json')['samples']
geometry = read(root / 'geometry.native.json')['geometry_query_probe']['samples']
assert read(root / 'geometry.native.json.reader.report.json')['success']
assert [row['input'] for row in geometry] == request
common = {}
for row in geometry:
    name = row['input']['name']
    hit = row['capsule_sweep']
    assert hit['hit'] and 0 < hit['distance_cm'] < row['input']['capsule_sweep']['length_cm']
    vertical = hit['normal']['z']
    assert abs(vertical + math.sin(math.radians(pitches[name]))) < 1e-12
    assert abs(vertical) < threshold and (vertical > 0) == (name == 'positive')
    common[name] = dict(vertical=vertical, normal_bevy_axes=[hit['normal'][axis] for axis in ('x','z','y')],
                        distance_cm=hit['distance_cm'])

assert read(root / 'build-pitch.report.json')['success']
assert read(root / 'wall.native.json.reader.report.json')['success']
native_path = root / 'wall.native.json'
native_cases = read(native_path)['character_animation_probe']['cases']
native = {}
for case in native_cases:
    name = case['name']
    q49 = case['frames'][49]
    before = q49['movement_before_tick']
    assert before['movement_mode_name'] == 'MOVE_Falling'
    assert [before['velocity_cm_s'][key] / 100 for key in ('x','z','y')] == initial['velocity_mps']
    assert q49['world_delta_seconds'] == initial['delta_s']
    native[name] = [
        dict(mode=row['movement_after_tick']['movement_mode_name'],
             floor=row['movement_after_tick']['walkable_floor'],
             velocity_cm_s=row['movement_after_tick']['velocity_cm_s'],
             position_cm=row['actor_transform']['translation'])
        for row in (case['frames'][frame] for frame in frames)
    ]
    a, b = native[name]
    assert a['mode'] == 'MOVE_Falling' and not a['floor']
    assert b['mode'] == 'MOVE_Walking' and b['floor']
    a['deck_gap_m'] = a['position_cm']['z'] / 100 + 2.0 - .9 - initial['deck_top_m']
    assert .013 < a['deck_gap_m'] < .014
assert native.keys() == pitches.keys()

rows = {}
first = {}
parameters = {}
for label in labels:
    functional = read(root / f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images'] == []
    assert load(label, 'process.report')['success']
    sign = 'negative' if label == 'negative' else 'positive'
    barrier = load(label, 'barrier')
    assert math.isclose(barrier['pitch_source_degrees'], pitches[sign], abs_tol=1e-9)
    assert barrier['yaw_source_degrees'] == 45.0
    rows[label] = {
        f'{stage}-{frame}': load(label, f'contact-{stage}-{frame}')
        for frame in frames for stage in ('before', 'after')
    }
    first[label] = actual = load(label, 'initial-observed')
    assert actual['position'] == initial['position_m'] and actual['step'] == initial['delta_s']
    assert actual['velocity'] == actual['initial_with_motor'] == initial['velocity_mps']
    assert actual['wall'] == 'AuditWalkOffBarrier' and actual['second_hit'] is None and actual['mode'] == 'Air'
    assert actual['request'] == first['positive']['request']
    before = rows[label]['before-49']
    assert before['position'] == actual['single_hit_position']
    assert before['planned_velocity'] == actual['single_hit_velocity']
    assert before['mode'] == 'Air' and not before['has_basis_displacement'] and before['manifolds'] == []
    assert before['requested_mode'] is None and before['floor']['name'] == 'AuditLandingDeck'
    assert .013 < before['floor']['gap'] < .014 and before['floor']['impact'] < before['floor']['lower_hemisphere']
    n = common[sign]['normal_bevy_axes']
    dot = sum(a*b for a,b in zip(n, actual['request'], strict=True))
    assert dot < 0
    if sign == 'positive':
        assert abs(actual['normal'][1] - n[1]) < 1e-8
    assert (actual['normal'][1] > 0) == (n[1] > 0)
    assert abs(actual['normal'][1]) < threshold
    assert before['generated_contact'] == (n[1] > 0) == (sign == 'positive')
    assert before['consumed_contact'] == (sign == 'positive' and label != 'cleared')
    parameters[label] = read(file(label, 'parameters'))

positive, negative, cleared, restored = [rows[label] for label in labels]
assert restored == positive and first['restored'] == first['cleared'] == first['positive']
assert positive['before-49'] == (cleared['before-49'] | {'consumed_contact': True})
assert positive['after-49']['mode'] == 'Grounded' and positive['after-49']['has_base']
assert positive['after-49']['velocity'][1] == 0 and positive['after-49']['published'][1] == 0
landing_event = [['LocomotionState(Air)', 'LocomotionState(Grounded)']]
assert positive['after-49']['transitions'] == landing_event
for case in (negative, cleared):
    assert case['after-49']['mode'] == 'Air' and not case['after-49']['has_base']
    assert case['after-49']['velocity'] == case['before-49']['planned_velocity']
    assert case['after-49']['transitions'] == []
    assert case['before-50']['mode'] == 'Grounded'
    assert case['after-50']['mode'] == 'Grounded' and case['after-50']['transitions'] == landing_event
assert cleared['after-49']['position'] == positive['after-49']['position']

restoration = {}
for key in parameters['positive']:
    old, new = [{row['ordinal']:row for row in parameters[label][key]} for label in ('positive','restored')]
    assert old.keys() == new.keys()
    restoration[key] = next((ordinal for ordinal in sorted(old) if old[ordinal] != new[ordinal]), None)
    assert restoration[key] is None or restoration[key] > max(frames)
for label, records in parameters.items():
    after = {row['ordinal']:row for row in records['after']}
    for frame in frames:
        row = after[frame]
        assert row['gameplay'] == rows[label][f'after-{frame}']['mode']
        assert row['graph_tag'] == 'Als.LocomotionMode.' + ('InAir' if row['gameplay'] == 'Air' else 'Grounded')
        assert row['graph_error'] is None

decoded = {}
for path in [native_path] + [file(label, name) for label in labels for name in ('parameters','report')]:
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
    (cmc, ue / cmc, [(5047,5083), (6983,7016), (7374,7442)]),
    (header, ue / header, [(128,132)]),
    (motion, Path(motion), [(2723,2747), (4070,4124)]),
):
    data = path.read_bytes()
    lines = data.decode().splitlines()
    facts.append(dict(key=key, **identity(data), excerpts=[
        dict(start=a, end=b, text='\n'.join(lines[a-1:b]) + '\n') for a,b in ranges]))
save('source-facts.json', facts)

inputs = {row['path'] for row in read(prior / 'summary.json')['external_source_files']}
inputs.update(str(path.relative_to(Path.cwd())) for path in (
    prior / 'initial.json', prior / 'wall.request.json', prior / 'prepare_reader.py',
    prior / 'native_initial.inc', prior / 'native_deck.inc', prior / 'deck.rs',
    prior / 'deck_fixture.inc', prior / 'observer.py', prior / 'summary.json',
    root.parent / '521' / 'geometry.request.json',
))
save('summary.json', dict(
    gate='PASS_INDEPENDENT_FALLING_WALL_LANDING_QUALIFICATION', findings=['BF144-D291'],
    new_differences=1, total_differences=288, source_fallback_threshold=threshold,
    ordinals=list(frames), pitches=pitches, common_native_geometry=common,
    target_normals={label: first[label]['normal'] for label in labels}, native=native, target=rows,
    focused_restoration_exact=True, restored_first_late_parameter_difference=restoration,
    decoded=decoded, screenshots=0, performance_acceptance='paused',
    external_source_files=[dict(path=name, **identity(Path(name).read_bytes())) for name in sorted(inputs)]))
print('Verified independent positive wall normal qualification, negative control and live restoration', flush=True)
