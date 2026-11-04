import gzip
import hashlib
import json
import os
import struct
from pathlib import Path

root = Path(__file__).resolve().parent
parent = root.parent
def raw(name):
    path = root/name
    return path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix('.json.gz').read_bytes())

def load(name):
    return json.loads(raw(name))

def save(name, value):
    (root/name).write_text(json.dumps(value, indent=2) + '\n')

def props(rows):
    return {row['name']: row for row in rows}

def f32(value):
    return struct.unpack('<f', struct.pack('<f', value))[0]

def dot32(a, b):
    terms = [f32(f32(x) * f32(y)) for x,y in zip(a,b,strict=True)]
    return f32(f32(terms[0]+terms[1])+terms[2])

replay = load('replay-baseline.json')
assert raw('replay-baseline.json') == raw('replay-restored.json')
for label in ('baseline','restored'):
    assert load(f'replay-{label}.report.json')['success']
    assert load(f'replay-compile-{label}.report.json')['success']
assert replay['native_clamps'] == 6 and replay['native_brakes'] == 8
assert len(replay['target']) == 38 and len(replay['edges']) == 13
for row in load('acceleration-input.json')['native']:
    a,v = row['acceleration'],row['velocity']
    assert (sum(x*y for x,y in zip(a,v,strict=True)) < 0) == (dot32(a,v) < 0)
assert all((row['dot'] < 0) == (row['wide_dot'] < 0) for row in replay['target'])
clamp_frames = [row['frame'] for row in replay['target'] if row['actual'] != row['clamp_local']]
assert clamp_frames == [21,23,24,59,61,62,63,64]
assert all(row['capped'] for row in replay['target'] if row['frame'] in clamp_frames)
division_frames = [row['frame'] for row in replay['target'] if row['actual'] != row['reciprocal_world']]
assert division_frames == [65]
source_difference_frames = [row['frame'] for row in replay['target'] if row['actual'] != row['source_kernel_same_physical_inputs']]

control = load('controls.json')
edge = control['edge']
assert edge in replay['edges'] and edge['dot32'] == 0 and edge['dot64'] < 0
assert dot32(edge['acceleration'],edge['velocity']) == edge['dot32']
native_dot = sum(a*v for a,v in zip(edge['native_acceleration'],edge['native_velocity'],strict=True))
assert native_dot < 0
assert load('witness.native.json.reader.report.json')['success']
assert load('build-witness.report.json')['success']
requests = {case['name']:case['acceleration_witness'] for case in load('witness.request.json')['cases']}
witnesses = {}
for case in load('witness.native.json')['character_animation_probe']['cases']:
    name = case['name']
    assert case['actual_lean_half_life'] == 0 and case['source_character_bound']
    settings = requests[name]
    state = props(case['acceleration_witness'])
    l = props(state['LocomotionState']['fields'])
    lean = props(state['LeanState']['fields'])
    for key, native in (('acceleration','AccelerationWorldSpace'),('velocity','VelocityWorldSpace'),
                        ('rotation','RotationQuaternionWorldSpace')):
        assert settings[key] == [item['numeric_value'] for item in l[native]['fields']]
    for key, native in (('max_acceleration','MaxAcceleration'),('max_braking','MaxBrakingDeceleration')):
        assert f32(settings[key]) == l[native]['numeric_value']
    witnesses[name] = [lean[k]['numeric_value'] for k in ('ForwardAmount','RightAmount')]
assert witnesses['cancel'] == edge['native']
assert witnesses['cancel-no-call'] == [0,0]
assert witnesses['cancel-acceleration-limit'] != witnesses['cancel']
for row in control['natural']:
    assert witnesses[f'natural-{row["frame"]}'] == row['source_kernel_same_physical_inputs']
assert witnesses['natural-65'][0] == control['natural'][1]['reciprocal_world'][0]
save('native-witnesses.json', witnesses)
print('Verified actual native RefreshGrounded, omitted call and natural input witnesses', flush=True)

for label in ('baseline','cancel','braking-only','restored'):
    assert load(f'runtime-{label}/process.report.json')['success']
    assert load(f'{label}-functional-process.json')['success']
    functional = load(f'{label}-functional.json')
    assert functional['pass'] and functional['validation_scope'] == 'functional' and functional['images'] == []
for label in ('baseline','controlled'):
    assert load(f'compile-{label}.report.json')['success']
assert raw('runtime-baseline/parameters.json') == raw('runtime-restored/parameters.json')
old = json.loads(gzip.decompress((parent/'509/runtime-baseline/parameters.json.gz').read_bytes()))
base = load('runtime-baseline/parameters.json')
assert {side:[{k:v for k,v in row.items() if k!='acceleration'}
              for row in rows if row['ordinal'] >= 24] for side,rows in base.items()} == old
actual = {}
for label, expected in (('cancel',edge['actual']),('braking-only',edge['braking_only'])):
    run = load(f'runtime-{label}/parameters.json')
    assert [row for row in run['after'] if row['ordinal'] < 20] == [row for row in base['after'] if row['ordinal'] < 20]
    row = next(row for row in run['after'] if row['ordinal']==20)
    baseline = next(row for row in base['after'] if row['ordinal']==20)
    for key in ('gameplay','graph_tag','position','velocity','input','animation_frame'):
        assert row[key] == baseline[key]
    assert row['gameplay'] == 'Grounded' and row['graph_error'] is None
    assert row['lean']['goal'] == expected
    state = row['acceleration']
    assert state['acceleration'] == edge['acceleration'] and state['velocity'] == edge['velocity']
    assert state['rotation'] == edge['rotation'] and state['max_braking'] == edge['max_braking']
    assert state['max_acceleration'] == edge['max_acceleration' if label=='cancel' else 'max_braking']
    assert state['standing_acceleration'] == expected[0] and state['standing_write_frame'] == row['animation_frame']
    assert row['lean']['write_frame'] == row['animation_frame']
    actual[label] = row
assert actual['cancel']['lean']['amounts'] != actual['braking-only']['lean']['amounts']
assert actual['braking-only']['acceleration']['standing_acceleration'] == witnesses['cancel'][0]
save('actual-boundary.json', actual)
print('Verified actual Standing and Lean consumers with exact restored baseline', flush=True)

env = json.loads(Path('tools/env.local.json').read_bytes())
als = Path(env['ue_project']).parent/'Plugins/ALS-Refactored/Source/ALS'
ue = Path(os.environ['ALS_UE_SOURCE_ROOT'])
facts = []
for key,path,ranges in (
    ('source:Private/AlsAnimationInstance.cpp', als/'Private/AlsAnimationInstance.cpp', [(829,848),(903,919)]),
    ('source:Public/Utility/AlsVector.h', als/'Public/Utility/AlsVector.h', [(100,111)]),
    ('Engine/Source/Runtime/Core/Public/Math/Quat.h', ue/'Engine/Source/Runtime/Core/Public/Math/Quat.h', [(1252,1259)]),
    ('Engine/Source/Runtime/Core/Public/Math/Vector.h', ue/'Engine/Source/Runtime/Core/Public/Math/Vector.h', [(322,329),(1726,1729)]),
    ('src/als/utility/animation.rs', Path('src/als/utility/animation.rs'), [(12,31)]),
    ('src/als/character/animation_instances/source_native_refresh.rs', Path('src/als/character/animation_instances/source_native_refresh.rs'), [(183,204)]),
):
    data = path.read_bytes()
    lines = data.decode().splitlines()
    facts.append(dict(key=key,sha256=hashlib.sha256(data).hexdigest(),
        excerpts=[dict(start=start,end=end,text='\n'.join(lines[start-1:end])+'\n') for start,end in ranges]))
save('source-facts.json', facts)
save('final-source.json', dict(sha256=hashlib.sha256((root/'replay.rs').read_bytes()).hexdigest()))

decoded = {}
for name in ['witness.native.json'] + [f'runtime-{label}/{file}.json'
        for label in ('baseline','cancel','braking-only','restored') for file in ('report','parameters')]:
    data = raw(name)
    path = root/name
    archive = path.with_suffix('.json.gz')
    if path.exists():
        archive.write_bytes(gzip.compress(data, compresslevel=6, mtime=0))
    assert gzip.decompress(archive.read_bytes()) == data
    decoded[name] = dict(bytes=len(data),sha256=hashlib.sha256(data).hexdigest())
    print('Verified gzip ' + name, flush=True)
paths = [parent/name for name in ('484/playground_observed.rs','484/foot_observer.rs','505/observer.py',
        '505/velocity_observer.rs','508/prepare_reader.py','509/observer.py','509/lean_observer.rs',
        '509/zero.request.json','509/matrix.native.json.gz','509/runtime-baseline/parameters.json.gz',
        '491/reader-migration.json','441c/library-identities.json')]
external = [dict(path=str(path.resolve().relative_to(Path.cwd())),bytes=path.stat().st_size,
                 sha256=hashlib.sha256(path.read_bytes()).hexdigest()) for path in paths]
save('summary.json',dict(
    gate='PASS_GROUNDED_ACCELERATION_DOT_AND_LOCAL_CLAMP',
    differences=['BF144-D285','BF144-D286'], related='BF144-D281',
    new_differences=2,total_differences=283,native_frames=48,target_frames=38,
    native_controlled_cases=len(witnesses),source_formula_changed_frames=source_difference_frames,
    local_clamp_changed_frames=clamp_frames,shared_reciprocal_changed_frames=division_frames,
    actual_branch_ordinal=20,controlled_dot64=edge['dot64'],controlled_dot32=edge['dot32'],
    screenshots=0,performance_acceptance='paused',decoded=decoded,external_source_files=external))
