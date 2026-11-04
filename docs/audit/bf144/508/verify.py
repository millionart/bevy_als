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
    denominator = f32(f32(f32(1 + first) + second) + third)
    return f32(1 - f32(1 / denominator))

def rows(case):
    projected = []
    for row in case['frames']:
        src = props(row['source_state_after_tick'])
        blend = props(props(src['GroundedState']['fields'])['VelocityBlend']['fields'])
        loco = props(src['LocomotionState']['fields'])
        assert blend['bInitializationRequired']['exported_text'] == 'False'
        assert props(src['LocomotionMode']['fields'])['TagName']['exported_text'].endswith('.Grounded')
        projected.append(dict(
            frame=row['frame'], delta=row['world_delta_seconds'],
            velocity=[p['numeric_value'] for p in loco['VelocityWorldSpace']['fields']],
            rotation=[p['numeric_value'] for p in loco['RotationQuaternionWorldSpace']['fields']],
            amounts=[blend[direction + 'Amount']['numeric_value'] for direction in ('Forward', 'Backward', 'Left', 'Right')],
            actor_transform=row['actor_transform'], actor_velocity=row['actor_velocity_cm_s'],
            pose=row['mesh_local_pose'], curves=row['curves']))
    return projected

def cases(name):
    return {row['name']: row for row in load(f'{name}.native.json')['character_animation_probe']['cases']}

assert load('source_check.report.json')['success']
for label in ('matrix', 'omit-write', 'restored', 'canonical'):
    assert load(f'{label}.native.json.reader.report.json')['success']
for label in ('baseline', 'omit-write', 'restored'):
    assert load(f'build-{label}.report.json')['success']
    assert load(f'prepare_reader-{label}.report.json')['success']
assert load('invalid.native.json.reader.report.json')['process_exit_code'] == 7
assert not (root/'invalid.native.json').exists()
native_cases = cases('matrix')
native = {name: rows(case) for name, case in native_cases.items()}
assert list(native) == ['omitted', 'explicit-default', 'zero', 'slow', 'negative']
default = native['omitted']
instant = native['zero']
assert all(len(run) == 48 for run in native.values())
assert default == native['explicit-default']
assert instant == native['negative']
assert 'actual_velocity_blend_half_life' not in native_cases['omitted']
for key, value in (('explicit-default', 0.1), ('zero', 0), ('slow', 0.4), ('negative', -0.1)):
    assert native_cases[key]['requested_velocity_blend_half_life'] == value
    assert native_cases[key]['actual_velocity_blend_half_life'] == f32(value)
    assert native_cases[key]['settings'] != native_cases['omitted']['settings']
for run in native.values():
    for current, baseline in zip(run, default, strict=True):
        assert {k:v for k,v in current.items() if k not in ('amounts', 'pose', 'curves')} == {
            k:v for k,v in baseline.items() if k not in ('amounts', 'pose', 'curves')}

replayed_fields = 0
for label in ('omitted', 'explicit-default', 'slow'):
    run = native[label]
    half_life = native_cases[label].get('actual_velocity_blend_half_life', f32(0.1))
    state = [0.0] * 4
    for current, goal in zip(run, instant, strict=True):
        alpha = damper_alpha(current['delta'], half_life)
        state = [f32(old + f32(alpha * f32(new - old))) for old, new in zip(state, goal['amounts'], strict=True)]
        assert state == current['amounts'], (label, current['frame'])
        replayed_fields += len(state)
assert instant[11]['amounts'] == default[11]['amounts'] == native['slow'][11]['amounts'] == [0, 0, 0, 0]
assert instant[12]['amounts'][0] > default[12]['amounts'][0] > native['slow'][12]['amounts'][0]
assert instant[47]['amounts'] == [0, 0, 0, 0] and default[47]['amounts'] != instant[47]['amounts']
pose_changes = {label: sum(a['pose'] != b['pose'] for a, b in zip(default, native[label], strict=True))
                for label in ('zero', 'slow')}
assert pose_changes == {'zero': 35, 'slow': 35}
print('Verified exact source recurrence and 35 pose consumers per variant', flush=True)

negative = cases('omit-write')['zero']
restored = cases('restored')['zero']
assert negative['requested_velocity_blend_half_life'] == 0
assert negative['actual_velocity_blend_half_life'] == f32(0.1)
assert restored['actual_velocity_blend_half_life'] == 0
assert rows(negative) == default and rows(restored) == instant
assert rows(cases('canonical')['omitted']) == default
for label in ('omit-write', 'restored', 'canonical'):
    package = load(f'{label}.package.json')
    assert len(package['sha256']) == 64
assert load('omit-write.package.json')['sha256'] != load('restored.package.json')['sha256']
a, b, c = (load(f'reader-source-{mode}.json') for mode in ('baseline', 'omit-write', 'restored'))
assert a['diagnostic_sha256'] == c['diagnostic_sha256'] != b['diagnostic_sha256']
assert a['canonical_sha256'] == b['canonical_sha256'] == c['canonical_sha256']
print('Verified write ablation, byte restoration and canonical reader compatibility', flush=True)

for label in ('baseline', 'snap', 'restored'):
    assert load(f'runtime-{label}/process.report.json')['success']
    assert load(f'{label}-functional-process.json')['success']
    gate = load(f'{label}-functional.json')
    assert gate['pass'] and gate['validation_scope'] == 'functional' and gate['images'] == []
assert load('compile-visible.report.json')['success']
assert raw('runtime-baseline/parameters.json') == raw('runtime-restored/parameters.json')
assert raw('runtime-baseline/parameters.json') == gzip.decompress(
    (parent/'505/runtime-baseline/parameters.json.gz').read_bytes())
base = load('runtime-baseline/parameters.json')
snap = load('runtime-snap/parameters.json')
witness = 32
comparison = {}
for label, data in (('baseline', base), ('snap', snap)):
    before = next(row for row in data['before'] if row['ordinal'] == witness)
    after = next(row for row in data['after'] if row['ordinal'] == witness)
    assert after['gameplay'] == 'Grounded' and after['graph_error'] is None
    assert after['velocity_blend']['required'] is False
    comparison[label] = dict(before=before, after=after)
assert comparison['baseline']['before'] == comparison['snap']['before']
for side in ('before', 'after'):
    assert [row for row in base[side] if row['ordinal'] < witness] == [
        row for row in snap[side] if row['ordinal'] < witness]
control, target = (comparison[label]['after'] for label in ('snap', 'baseline'))
assert {k:v for k,v in target.items() if k != 'velocity_blend'} == {
    k:v for k,v in control.items() if k != 'velocity_blend'}
before = comparison['baseline']['before']['velocity_blend']['candidates']
assert target['velocity_blend']['amounts'] == before['damp']
assert control['velocity_blend']['amounts'] == before['snap'] != before['damp']
save('actual-boundary.json', comparison)
print('Verified actual one-frame target control and exact default restoration', flush=True)

compact = {key: [{k:v for k,v in row.items() if k not in ('pose', 'curves')} for row in run]
           for key, run in native.items()}
save('native-velocity-blend.json', compact)
env = json.loads(Path('tools/env.local.json').read_bytes())
als = Path(env['ue_project']).parent / 'Plugins/ALS-Refactored/Source/ALS'
ue = Path(os.environ['ALS_UE_SOURCE_ROOT'])
facts = []
for key, path, start, end in (
    ('source:Public/Settings/AlsGroundedSettings.h', als/'Public/Settings/AlsGroundedSettings.h', 24, 26),
    ('source:Private/AlsAnimationInstance.cpp', als/'Private/AlsAnimationInstance.cpp', 850, 900),
    ('source:Public/Utility/AlsMath.h', als/'Public/Utility/AlsMath.h', 84, 95),
    ('Engine/Source/Runtime/Core/Public/Math/UnrealMathUtility.h', ue/'Engine/Source/Runtime/Core/Public/Math/UnrealMathUtility.h', 1552, 1561),
    ('src/als/character/animation_instances/source_native_refresh.rs', Path('src/als/character/animation_instances/source_native_refresh.rs'), 135, 184),
    ('tools/import/gen_standing_curves.py', Path('tools/import/gen_standing_curves.py'), 93, 111),
):
    body = path.read_bytes()
    lines = body.decode().splitlines()
    facts.append(dict(key=key, sha256=hashlib.sha256(body).hexdigest(),
                      excerpts=[dict(start=start, end=end, text='\n'.join(lines[start-1:end])+'\n')]))
save('source-facts.json', facts)
save('final-source.json', dict(sha256=hashlib.sha256((root/'snap_observer.rs').read_bytes()).hexdigest()))

decoded = {}
for name in [f'{label}.native.json' for label in ('matrix', 'omit-write', 'restored', 'canonical')] + [
    f'runtime-{label}/{file}.json' for label in ('baseline', 'snap', 'restored') for file in ('report', 'parameters')
]:
    data = raw(name)
    path = root / name
    archive = path.with_suffix('.json.gz')
    if path.exists():
        archive.write_bytes(gzip.compress(data, compresslevel=6, mtime=0))
    assert gzip.decompress(archive.read_bytes()) == data
    decoded[name] = dict(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())
    print('Verified gzip ' + name, flush=True)
paths = [parent/name for name in (
    '484/playground_observed.rs', '484/foot_observer.rs', '493/virtual-motion.request.json',
    '505/observer.py', '505/velocity_observer.rs', '505/runtime-baseline/parameters.json.gz',
    '506/ab_als.native.json.gz', '491/reader-migration.json', '441c/library-identities.json')]
external = [dict(path=str(path.resolve().relative_to(Path.cwd())), bytes=path.stat().st_size,
                 sha256=hashlib.sha256(path.read_bytes()).hexdigest()) for path in paths]
save('summary.json', dict(
    gate='PASS_VELOCITY_BLEND_HALF_LIFE_CONFIGURATION_D283',
    known_difference='BF144-D283', new_differences=1, total_differences=280,
    native_cases=len(native), frames_per_case=48, native_positive_replayed_fields=replayed_fields,
    native_changed_pose_frames=pose_changes,
    native_first_movement_frame=12,
    native_first_forward={label: run[12]['amounts'][0] for label, run in native.items()},
    target_control_ordinal=witness, target_default_amounts=target['velocity_blend']['amounts'],
    target_instant_amounts=control['velocity_blend']['amounts'],
    native_initial_package_hash_recorded=False, diagnostic_source_restored=True,
    screenshots=0, performance_acceptance='paused', decoded=decoded, external_source_files=external))
