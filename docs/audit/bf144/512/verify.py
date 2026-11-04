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

def clamp(value):
    return min(1.0, max(0.0, value))

def step(previous, raw_alpha, delta, clamp_before_interp=False):
    target = f32(raw_alpha * 4)
    if clamp_before_interp:
        target = clamp(target)
    if previous is None:
        return target
    distance = f32(target - previous)
    if f32(distance * distance) < 1e-8:
        return target
    scale = clamp(f32(delta * (20 if target >= previous else 4)))
    return f32(previous + f32(distance * scale))

def source_forward(locomotion):
    a, v, q = [[f['numeric_value'] for f in locomotion[key]['fields']] for key in
               ('AccelerationWorldSpace', 'VelocityWorldSpace', 'RotationQuaternionWorldSpace')]
    ax, ay, az = a
    qx, qy, qz, qw = q
    assert az == qx == qy == 0
    is_accelerating = sum(x*y for x,y in zip(a,v,strict=True)) >= 0
    limit = locomotion['MaxAcceleration' if is_accelerating else 'MaxBrakingDeceleration']['numeric_value']
    if limit <= f32(1e-4):
        return 0.0
    tx, ty = 2*qz*ay, -2*qz*ax
    scale = f32(1/limit)
    x, y = f32(f32(ax+qw*tx+qz*ty)*scale), f32(f32(ay+qw*ty-qz*tx)*scale)
    squared = f32(f32(x*x)+f32(y*y))
    return f32(x*f32(1/f32(math.sqrt(squared)))) if squared > 1 else x

normal_id = '92992f00-4a96-a667-a6b3-6b81e0f00ba2'
acceleration_id = 'f96b7815-4516-b730-ccda-41afd8c5be66'
assert load('sprint.native.json.reader.report.json')['success']
native_rows = load('sprint.native.json')['character_animation_probe']['cases'][0]['frames']
assert len(native_rows) == 80
native = []
previous_time = 0.0
alpha_state = premature = None
early_clamp_mismatches = {'native':[], 'target':[]}
for row in native_rows:
    frame = row['frame']
    state = props(row['source_state_after_tick'])
    stand = props(state['StandingState']['fields'])
    loco = props(state['LocomotionState']['fields'])
    gait = props(state['Gait']['fields'])['TagName']['exported_text']
    sprint = gait.endswith('.Sprinting')
    assert sprint == (25 <= frame < 60)
    delta = row['world_delta_seconds']
    actual_time = stand['SprintTime']['numeric_value']
    raw_alpha = stand['SprintAccelerationAmount']['numeric_value']
    previous_time = f32(previous_time+delta) if sprint else 0
    assert actual_time == previous_time
    if sprint:
        assert raw_alpha == (source_forward(loco) if actual_time < 0.5 else 0)
    else:
        assert raw_alpha == 0
    instance = next(v for v in row['linked_instances'] if v['class'].endswith('.AB_Als_Standing_C'))
    weights = {item['node_guid']:item['cached_blend_weight'] for item in instance['asset_players']}
    a,b = weights[normal_id],weights[acceleration_id]
    total = f32(a+b)
    alpha = None
    if sprint:
        alpha_state = step(alpha_state,raw_alpha,delta)
        premature = step(premature,raw_alpha,delta,True)
        alpha = clamp(alpha_state)
        assert (a,b) == (f32(total*f32(1-alpha)),f32(total*alpha))
        if clamp(premature) != alpha:
            early_clamp_mismatches['native'].append(frame)
    native.append(dict(frame=frame,gait=gait,time=actual_time,acceleration=raw_alpha,alpha=alpha,
                       weights=[a,b],max_acceleration=loco['MaxAcceleration']['numeric_value'],
                       velocity=row['actor_velocity_cm_s']))
assert native[25]['time'] == f32(1/30) and native[39]['time'] == 0.5
assert native[41]['alpha'] < 1 and native[40]['alpha']==1
save('native-sprint.json',native)
print('Verified native timer, acceleration, and both cached Sprint weights',flush=True)

labels = ('baseline','acceleration-only','cutoff','restored')
runs = {}
assert load('runtime-cutoff-strict-baseline/process.report.json')['process_exit_code'] == 101
for label in labels:
    for name in (f'runtime-{label}/process.report.json',f'{label}-functional-process.json'):
        assert load(name)['success']
    gate = load(f'{label}-functional.json')
    assert gate['pass'] and gate['images']==[] and gate['validation_scope']=='functional'
    runs[label] = {row['ordinal']:row for row in load(f'runtime-{label}/parameters.json')['after']}
baseline = runs['baseline']
for label,rows in runs.items():
    previous_time = 0.0
    alpha_state = premature = None
    for i,row in rows.items():
        sprint = row['gait'].endswith('.Sprinting')
        assert sprint == (24 <= i < 60)
        assert row['sprint']['flag'] == float(sprint)
        state = row['sprint']
        delta = row['velocity_blend']['delta']
        if state['written'] == [row['animation_frame']]*2:
            previous_time = f32(previous_time+delta) if sprint else 0
            expected = (state['override'] if state['override'] is not None else row['acceleration']['standing_acceleration'])
            assert state['time'] == previous_time
            assert state['acceleration'] == (expected if sprint and previous_time < .5 else 0)
        else:
            assert not sprint
        if state['mixed']:
            mixed, = state['mixed']
            assert mixed['input'] == state['acceleration']
            alpha_state = step(alpha_state,mixed['input'],delta)
            premature = step(premature,mixed['input'],delta,True)
            assert mixed['alpha'] == clamp(alpha_state), (label,i,mixed['alpha'],clamp(alpha_state))
            if label=='baseline' and clamp(premature)!=mixed['alpha']:
                early_clamp_mismatches['target'].append(i)
        else:
            alpha_state = premature = None
assert baseline[24]['sprint']['time']==f32(1/30) and baseline[38]['sprint']['time']==.5
assert baseline[42]['sprint']['mixed'][0]['alpha']==1 and baseline[43]['sprint']['mixed'][0]['alpha']<1
assert baseline[60]['sprint']['time']==0 and baseline[60]['sprint']['mixed'][0]['alpha']>0
assert early_clamp_mismatches['native'] and early_clamp_mismatches['target']

def bones(pose_a,pose_b):
    assert len(pose_a['bones'])==len(pose_b['bones'])
    assert [bone['name'] for bone in pose_a['bones']]==[bone['name'] for bone in pose_b['bones']]
    result = [a['name'] for a,b in zip(pose_a['bones'],pose_b['bones'],strict=True) if a!=b]
    assert None not in result
    return result

a,b = baseline[24],runs['acceleration-only'][24]
assert {k:v for k,v in a.items() if k not in ('rig_poses','sprint','sprint_leaves')} == {
    k:v for k,v in b.items() if k not in ('rig_poses','sprint','sprint_leaves')}
assert a['sprint']['mixed'][0]['alpha']==1 and b['sprint']['mixed'][0]['alpha']==.5
assert b['sprint']['override']==b['sprint']['acceleration']==.125
assert len(a['sprint_leaves'])==1 and len(b['sprint_leaves'])==2
changed = bones(a['rig_poses']['root_result']['pose'], b['rig_poses']['root_result']['pose'])
assert len(changed)==72
c = runs['cutoff'][38]
assert c['sprint']['override']==f32(.9)
assert c['sprint']['time']==.5 and c['sprint']['acceleration']==c['sprint']['mixed'][0]['input']==0
assert c['sprint']['mixed'][0]['alpha']==baseline[38]['sprint']['mixed'][0]['alpha']==1
restored_exact = raw('runtime-baseline/parameters.json') == raw('runtime-restored/parameters.json')
first_restored_difference = next((i for i,row in baseline.items() if row!=runs['restored'][i]),None)
assert first_restored_difference == 34
assert baseline[24] == runs['restored'][24]
assert baseline[34]['position'] != runs['restored'][34]['position']
for label in ('baseline','cutoff','restored'):
    state = runs[label][38]['sprint']
    assert (state['time'],state['acceleration'],state['mixed'][0]['alpha']) == (.5,0,1)

for i,row in baseline.items():
    other = runs['restored'][i]
    assert (row['gait'],row['sprint']['time'],row['sprint']['flag']) == (other['gait'],other['sprint']['time'],other['sprint']['flag'])
    assert len(row['sprint']['mixed'])==len(other['sprint']['mixed'])
save('actual-controls.json',dict(early=dict(baseline=a,controlled=b,changed_root_bones=changed),
        cutoff=dict(baseline=baseline[38],controlled=c),restored_parameters_exact=restored_exact,
        premature_clamp_counterexamples=early_clamp_mismatches,first_restored_difference=first_restored_difference))
print('Verified target Sprint producer, active consumers and cutoff',flush=True)

assert native[20]['max_acceleration']==2000
assert baseline[20]['acceleration']['max_acceleration']==25
env = json.loads(Path('tools/env.local.json').read_bytes())
als = Path(env['ue_project']).parent/'Plugins/ALS-Refactored/Source/ALS'
ue = Path(os.environ['ALS_UE_SOURCE_ROOT'])
facts = []
for key,path,ranges in (
    ('source:Private/AlsAnimationInstance.cpp',als/'Private/AlsAnimationInstance.cpp',[(1045,1065)]),
    ('Engine/Source/Runtime/Engine/Private/Animation/InputScaleBias.cpp',ue/'Engine/Source/Runtime/Engine/Private/Animation/InputScaleBias.cpp',[(119,149)]),
    ('src/als/character/animation_instances/source_native_refresh.rs',Path('src/als/character/animation_instances/source_native_refresh.rs'),[(232,254)]),
    ('src/animation_graph_ext/source_pose_nodes.rs',Path('src/animation_graph_ext/source_pose_nodes.rs'),[(161,194),(199,222)]),
):
    data = path.read_bytes()
    lines = data.decode().splitlines()
    facts.append(dict(key=key,sha256=hashlib.sha256(data).hexdigest(),
        excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b])+'\n') for a,b in ranges]))
save('source-facts.json',facts)
save('final-source.json',dict(sha256=hashlib.sha256((root/'sprint_observer.rs').read_bytes()).hexdigest()))

decoded = {}
all_runs = labels + ('initial-baseline','initial-acceleration-only')
for name in ['sprint.native.json'] + [f'runtime-{label}/{file}.json'
        for label in all_runs for file in ('report','parameters')]:
    data = raw(name)
    path = root/name
    archive = path.with_suffix('.json.gz')
    if path.exists():
        archive.write_bytes(gzip.compress(data,compresslevel=6,mtime=0))
    assert gzip.decompress(archive.read_bytes())==data
    decoded[name] = dict(bytes=len(data),sha256=hashlib.sha256(data).hexdigest())
    print('Verified gzip '+name,flush=True)
paths = [parent/name for name in (
    '484/playground_observed.rs','484/foot_observer.rs','505/observer.py','505/velocity_observer.rs',
    '508/matrix.request.json','509/observer.py','509/lean_observer.rs',
    '510/observer.py','510/acceleration_observer.rs','511/standing.native.json.gz',
    '491/reader-migration.json','441c/library-identities.json')]
paths.append(Path('assets/ALS/Character/AnimationInstances/Stances/AB_Als_Standing/AnimGraph.animgraph.ron'))
external = [dict(path=str(path.resolve().relative_to(Path.cwd())),bytes=path.stat().st_size,
                 sha256=hashlib.sha256(path.read_bytes()).hexdigest()) for path in paths]
save('summary.json',dict(
    gate='PASS_NATURAL_SPRINT_ACCELERATION_CONSUMER_AND_CUTOFF',
    native_sprint_start=25,target_sprint_start=24,native_cutoff=39,target_cutoff=38,
    native_first_weight_below_one=41,target_first_weight_below_one=43,
    early_override_changed_bones=len(changed),cutoff_override_blocked=True,
    premature_clamp_counterexamples=early_clamp_mismatches,restored_parameters_exact=restored_exact,
    first_restored_difference=first_restored_difference,restored_entry_and_cutoff_exact=True,
    known_upstream_profile='BF144-D05',new_differences=0,total_differences=283,
    screenshots=0,performance_acceptance='paused',decoded=decoded,external_source_files=external))
