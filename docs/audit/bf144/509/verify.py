import gzip
import hashlib
import json
import math
import os
import struct
from pathlib import Path

root = Path(__file__).resolve().parent
parent = root.parent
def raw(name):
    path = root / name
    return path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix('.json.gz').read_bytes())

def load(name):
    return json.loads(raw(name))

def save(name, value):
    (root / name).write_text(json.dumps(value, indent=2) + '\n')

def props(rows):
    return {row['name']: row for row in rows}

def f32(value):
    return struct.unpack('<f', struct.pack('<f', value))[0]

def damper_alpha(delta, half_life):
    x = f32(f32(f32(math.log(2)) / f32(half_life + f32(1e-8))) * delta)
    first = f32(f32(1.00746054) * x)
    second = f32(f32(f32(0.45053901) * x) * x)
    third = f32(f32(f32(f32(0.25724632) * x) * x) * x)
    return f32(1 - f32(1 / f32(f32(f32(1 + first) + second) + third)))

def damp(previous, goal, delta, half_life):
    if half_life <= 0:
        return goal
    alpha = damper_alpha(delta, half_life)
    return [f32(old + f32(alpha * f32(new - old))) for old, new in zip(previous, goal, strict=True)]

def cases(name):
    return {row['name']: row for row in load(f'{name}.native.json')['character_animation_probe']['cases']}

def read_lean(fields):
    fields = props(props(fields)['LeanState']['fields'])
    return [fields[key]['numeric_value'] for key in ('ForwardAmount', 'RightAmount')]

def project(case, with_lean=True):
    result = []
    for row in case['frames']:
        source = props(row['source_state_after_tick'])
        locomotion = props(source['LocomotionState']['fields'])
        projected = dict(frame=row['frame'], delta=row['world_delta_seconds'],
                         tag=props(source['LocomotionMode']['fields'])['TagName']['exported_text'],
                         pending=source['bPendingUpdate']['value'],
                         velocity=[x['numeric_value'] for x in locomotion['VelocityWorldSpace']['fields']],
                         rotation=[x['numeric_value'] for x in locomotion['RotationQuaternionWorldSpace']['fields']],
                         actor_transform=row['actor_transform'], actor_velocity=row['actor_velocity_cm_s'],
                         pose=row['mesh_local_pose'], curves=row['curves'])
        if with_lean:
            projected['lean'] = read_lean(row['source_state_after_tick'])
        result.append(projected)
    return result

def without(row, names):
    return {k:v for k,v in row.items() if k not in names}

for label in ('preflight', 'matrix', 'omit-write', 'restored', 'canonical'):
    assert load(f'{label}.native.json.reader.report.json')['success']
assert load('import_check.report.json')['success']
assert load('import-check.json')['default_full_asset_exact']
assert not load('preflight-readout.json')['available']
matrix = cases('matrix')
legacy = cases('canonical')
ablated = cases('omit-write')
restored = cases('restored')
preflight = cases('preflight')
for label in ('first-lean', 'omit-write', 'restored'):
    assert load(f'build-{label}.report.json')['success']
    assert load(f'prepare_reader-{label}.report.json')['success']
a, b, c = (load(f'reader-source-{mode}.json') for mode in ('first-lean', 'omit-write', 'restored'))
assert a['diagnostic_sha256'] == c['diagnostic_sha256'] != b['diagnostic_sha256']
assert a['canonical_sha256'] == b['canonical_sha256'] == c['canonical_sha256']
assert load('matrix.package.json')['sha256'] != load('omit-write.package.json')['sha256']
assert load('restored.package.json')['sha256'] != load('omit-write.package.json')['sha256']

native_report = {}
positive_fields = 0
for domain, frames, expected_changes in (('ground', 48, 36), ('air', 25, 24)):
    grouped = {kind: matrix[f'{domain}-{kind}'] for kind in ('default', 'zero', 'slow', 'negative')}
    runs = {kind:project(case) for kind,case in grouped.items()}
    base, zero = runs['default'], runs['zero']
    assert all(len(run) == frames for run in runs.values())
    assert all(row['tag'].endswith('.Grounded' if domain == 'ground' else '.InAir') for row in base)
    assert runs['negative'] == zero
    assert 'actual_lean_half_life' not in grouped['default']
    for role, expected in (('zero', 0.0), ('slow', 0.6), ('negative', -0.1)):
        assert grouped[role]['actual_lean_half_life'] == f32(expected)
    for role, run in runs.items():
        assert [without(row, ('lean',)) for row in run] == project(preflight[f'{domain}-{role}'], with_lean=False)
        assert [without(row, ('lean', 'pose', 'curves')) for row in run] == [
            without(row, ('lean', 'pose', 'curves')) for row in base]
    assert [without(row, ('lean',)) for row in base] == project(legacy[f'{domain}-default'], with_lean=False)
    negative, restored_zero = ablated[f'{domain}-zero'], restored[f'{domain}-zero']
    assert negative['requested_lean_half_life'] == 0 and negative['actual_lean_half_life'] == f32(0.2)
    assert restored_zero['actual_lean_half_life'] == 0
    assert project(negative) == base and project(restored_zero) == zero
    pose_changes = {role: sum(a['pose'] != b['pose'] for a,b in zip(base,runs[role],strict=True))
                    for role in ('zero', 'slow')}
    assert pose_changes == dict(zero=expected_changes, slow=expected_changes)
    for kind, half_life in (('default', 0.2), ('slow', 0.6)):
        initial = grouped[kind]['initialization']['source_state']
        previous = read_lean(initial)
        pending = props(initial)['bPendingUpdate']['value'] == 'True'
        for row, snap in zip(runs[kind], zero, strict=True):
            previous = damp(previous, snap['lean'], row['delta'], 0 if pending else f32(half_life))
            pending = row['pending'] == 'True'
            assert previous == row['lean'], (domain, kind, row['frame'], previous, row['lean'])
            positive_fields += len(previous)
    first = next(i for i,(a,b) in enumerate(zip(base,zero,strict=True)) if a['lean'] != b['lean'])
    native_report[domain] = dict(pose_changes=pose_changes, first_changed_frame=first,
        first_values={role:run[first]['lean'] for role,run in runs.items()},
        trace={role:[without(row, ('pose', 'curves')) for row in run] for role,run in runs.items()})
    print(f'Verified native {domain} settings and direct readout restoration', flush=True)
save('native-lean.json', native_report)

labels = ('baseline', 'config-zero', 'config-slow', 'ground-snap', 'restored')
for label in labels:
    for report in (f'{label}-functional-process.json', f'runtime-{label}/process.report.json'):
        assert load(report)['success']
    gate = load(f'{label}-functional.json')
    assert gate['pass'] and gate['validation_scope'] == 'functional' and gate['images'] == []
assert raw('runtime-baseline/parameters.json') == raw('runtime-restored/parameters.json')
baseline = load('runtime-baseline/parameters.json')
previous_base = json.loads(gzip.decompress((parent/'505/runtime-baseline/parameters.json.gz').read_bytes()))
assert {side:[without(row, ('lean',)) for row in rows] for side,rows in baseline.items()} == previous_base
boundary = {}
for label in labels:
    data = load(f'runtime-{label}/parameters.json')
    boundary[label] = {}
    for ordinal, mode in ((32, 'Grounded'), (44, 'Air')):
        before = next(row for row in data['before'] if row['ordinal'] == ordinal)
        after = next(row for row in data['after'] if row['ordinal'] == ordinal)
        assert after['gameplay'] == mode and after['graph_error'] is None
        assert after['lean']['pending'] is False and after['lean']['write_frame'] == after['animation_frame']
        declared = {'config-zero': 0, 'config-slow': f32(0.6)}.get(label, f32(0.2))
        assert after['lean']['air_half_life'] == declared
        effective = declared if mode == 'Air' else (0 if label == 'ground-snap' else f32(0.2))
        assert after['lean']['amounts'] == damp(before['lean']['amounts'], after['lean']['goal'],
                                              after['velocity_blend']['delta'], effective)
        boundary[label][mode] = dict(before=before, after=after)
for label in ('config-zero', 'config-slow'):
    for mode in ('Grounded', 'Air'):
        original, control = boundary['baseline'][mode], boundary[label][mode]
        assert original['before']['lean']['amounts'] == control['before']['lean']['amounts']
        assert original['after']['lean']['goal'] == control['after']['lean']['goal']
        assert without(original['after'], ('lean',)) == without(control['after'], ('lean',))
        assert (original['after']['lean']['amounts'] == control['after']['lean']['amounts']) == (mode == 'Grounded')
assert boundary['ground-snap']['Grounded']['after']['lean']['amounts'] == [0, 0]
save('actual-boundary.json', boundary)
print('Verified five visible runs and per-node half-life counterfactuals', flush=True)

env = json.loads(Path('tools/env.local.json').read_bytes())
als = Path(env['ue_project']).parent/'Plugins/ALS-Refactored/Source/ALS'
ue = Path(os.environ['ALS_UE_SOURCE_ROOT'])
facts = []
for key, path, ranges in (
    ('source:Private/AlsAnimationInstance.cpp', als/'Private/AlsAnimationInstance.cpp', [(903, 919), (1220, 1241)]),
    ('source:Public/Settings/AlsGeneralAnimationSettings.h', als/'Public/Settings/AlsGeneralAnimationSettings.h', [(19, 21)]),
    ('Engine/Source/Runtime/Core/Public/Math/UnrealMathUtility.h', ue/'Engine/Source/Runtime/Core/Public/Math/UnrealMathUtility.h', [(1552, 1561)]),
    ('src/als/character/animation_instances/source_native_refresh.rs', Path('src/als/character/animation_instances/source_native_refresh.rs'), [(113, 131), (183, 201)]),
    ('src/als/character/animation_instances/locomotion_layer.rs', Path('src/als/character/animation_instances/locomotion_layer.rs'), [(45, 49), (208, 217)]),
    ('tools/import/gen_in_air_settings.py', Path('tools/import/gen_in_air_settings.py'), [(14, 23)]),
):
    data = path.read_bytes()
    lines = data.decode().splitlines()
    facts.append(dict(key=key, sha256=hashlib.sha256(data).hexdigest(),
        excerpts=[dict(start=start,end=end,text='\n'.join(lines[start-1:end])+'\n') for start,end in ranges]))
save('source-facts.json', facts)
save('final-source.json', dict(sha256=hashlib.sha256((root/'lean_observer.rs').read_bytes()).hexdigest()))

decoded = {}
native_names = ('preflight', 'preflight-omit-write', 'first-lean-zero', 'matrix', 'omit-write', 'restored', 'canonical')
for name in [f'{label}.native.json' for label in native_names] + [
    f'runtime-{label}/{file}.json' for label in labels for file in ('report', 'parameters')
]:
    data = raw(name)
    path = root/name
    archive = path.with_suffix('.json.gz')
    if path.exists():
        archive.write_bytes(gzip.compress(data, compresslevel=6, mtime=0))
    assert gzip.decompress(archive.read_bytes()) == data
    decoded[name] = dict(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())
    print('Verified gzip ' + name, flush=True)
paths = [parent/name for name in (
    '484/playground_observed.rs', '484/foot_observer.rs', '505/observer.py',
    '505/velocity_observer.rs', '505/runtime-baseline/parameters.json.gz',
    '508/prepare_reader.py', '508/settings.native.json', '491/reader-migration.json',
    '441c/library-identities.json')]
paths.append(Path('assets/ALS/Data/AnimationInstance/Air/als_in_air_settings.json'))
external = [dict(path=str(path.resolve().relative_to(Path.cwd())), bytes=path.stat().st_size,
                 sha256=hashlib.sha256(path.read_bytes()).hexdigest()) for path in paths]
save('summary.json', dict(
    gate='PASS_GROUNDED_AND_IN_AIR_LEAN_SETTINGS_D284', known_difference='BF144-D284',
    new_differences=1, total_differences=281, native_cases=len(matrix),
    native_exact_positive_fields=positive_fields,
    native_pose_changes={domain: report['pose_changes'] for domain,report in native_report.items()},
    target_witness_ordinals=dict(grounded=32,air=44), default_generated_asset_exact=True,
    diagnostic_source_restored=True, screenshots=0, performance_acceptance='paused',
    decoded=decoded, external_source_files=external))
