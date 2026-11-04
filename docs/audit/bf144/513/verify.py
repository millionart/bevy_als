import ast
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

def save(name,value):
    (root/name).write_text(json.dumps(value,indent=2)+'\n')

def f32(value):
    return struct.unpack('<f',struct.pack('<f',value))[0]

def frames(rows,key='ordinal'):
    return {row[key]:row for row in rows}

prior_model = ast.parse((parent/'512/verify.py').read_text())
functions = [node for node in prior_model.body if isinstance(node,ast.FunctionDef) and node.name in ('f32','clamp','step')]
assert len(functions)==3
model = dict(struct=struct)
exec(compile(ast.Module(body=functions,type_ignores=[]),'existing Sprint calculation','exec'),model)

labels = ('baseline','normal','acceleration-profile','restored')
runs = {}
for label in labels:
    for name in (f'{label}-functional-process.json',f'runtime-{label}/process.report.json'):
        assert load(name)['success']
    gate = load(f'{label}-functional.json')
    assert gate['pass'] and gate['images']==[] and gate['validation_scope']=='functional'
    data = load(f'runtime-{label}/parameters.json')
    runs[label] = dict(after=frames(data['after']),dynamics=frames(data['dynamics']))
    assert len(runs[label]['dynamics'])==80
    meta = load(f'runtime-{label}/profile.json')
    original,effective = meta['original'],meta['effective']
    assert meta['only_responsive_slot_changed']
    assert original==load('runtime-baseline/profile.json')['original']
    assert effective['normal']==original['normal'] and effective['sluggish']==original['sluggish']
    expected = dict(original['responsive'])
    if label == 'normal':
        expected = original['normal']
    elif label == 'acceleration-profile':
        expected['acceleration'] = original['normal']['acceleration']
    assert effective['responsive']==expected
assert load('compile-observed.report.json')['success']
assert load('prepare.report.json')['success']
for label in ('baseline','restored'):
    assert load(f'replay-{label}.report.json')['success']
    assert load(f'replay-compile-{label}.report.json')['success']
assert raw('replay-baseline.json')==raw('replay-restored.json')

native = frames(json.loads((parent/'512/native-sprint.json').read_bytes()),'frame')
source_report = json.loads(gzip.decompress((parent/'512/sprint.native.json.gz').read_bytes()))
source_frame = source_report['character_animation_probe']['cases'][0]['frames'][26]
facts = load('profile-facts.json')
baseline = runs['baseline']['after']
normal = runs['normal']['after']
partial = runs['acceleration-profile']['after']
restored = runs['restored']['after']
summary_rows = {}
for label,run in runs.items():
    after,dynamics = run['after'],run['dynamics']
    start = next(i for i,row in after.items() if row['gait'].endswith('.Sprinting'))
    cutoff = next(i for i,row in after.items() if row['sprint']['time'] is not None and row['sprint']['time']>=.5)
    expected_start = 24 if label in ('baseline','restored') else 25
    assert (start,cutoff)==(expected_start,expected_start+14)
    assert after[60]['sprint']['time']==0
    assert after[60]['gait'].endswith('.Running')
    state = None
    for i,row in after.items():
        m = row['sprint']['mixed']
        if m:
            entry, = m
            state = model['step'](state,entry['input'],row['velocity_blend']['delta'])
            assert entry['alpha']==model['clamp'](state)
        else:
            state = None
    for i,row in dynamics.items():
        assert row['dynamics']==row['public']
        assert row['input']==runs['baseline']['dynamics'][i]['input']
        assert row['rotation_mode']==runs['baseline']['dynamics'][i]['rotation_mode']
        assert row['stance']=='Standing'
    profile = (facts['starts']['normal'] if label=='normal' else
               [20,20,10] if label=='acceleration-profile' else facts['starts']['responsive'])
    assert dynamics[20]['dynamics']==profile
    assert dynamics[20]['rotation_mode']=='VelocityDirection'
    leaf, = after[25]['sprint_leaves']
    clock, = leaf['clocks']
    summary_rows[label] = dict(entry=start,cutoff=cutoff,initial_dynamics=profile,
        first_planned_speed=dynamics[20]['planned_velocity'][0],
        sprint_acceleration_clip_time_25=clock['relevant']['position_seconds'],
        blend_43=after[43]['sprint']['mixed'][0]['alpha'],after_release_speed_60=after[60]['velocity'][0])
assert native[25]['gait'].endswith('.Sprinting') and native[39]['time']==.5
assert not native[24]['gait'].endswith('.Sprinting')
assert normal[24]['gait'].endswith('.Running') and partial[24]['gait'].endswith('.Running')
assert summary_rows['normal']['sprint_acceleration_clip_time_25']==summary_rows['acceleration-profile']['sprint_acceleration_clip_time_25']
assert summary_rows['normal']['sprint_acceleration_clip_time_25']!=summary_rows['baseline']['sprint_acceleration_clip_time_25']
assert normal[60]['velocity'][0]>partial[60]['velocity'][0]
assert partial[60]['velocity'][0]==baseline[60]['velocity'][0]
for i in range(20,26):
    assert normal[i]['velocity'][0]==partial[i]['velocity'][0]
    assert abs(normal[i]['velocity'][0]-native[i]['velocity']['x']/100) < 0.0001
    assert abs(baseline[i]['velocity'][0]-native[i]['velocity']['x']/100) > 0.1
pose_a,pose_b = [row[25]['rig_poses']['root_result']['pose'] for row in (baseline,normal)]
assert [b['name'] for b in pose_a['bones']] == [b['name'] for b in pose_b['bones']]
changed = [a['name'] for a,b in zip(pose_a['bones'],pose_b['bones'],strict=True) if a!=b]
assert len(changed)==75 and None not in changed
first_restored_difference = next((i for i,row in baseline.items() if row!=restored[i]),None)
assert first_restored_difference is None or first_restored_difference>=34
assert summary_rows['restored']['entry']==summary_rows['baseline']['entry']
assert summary_rows['restored']['cutoff']==summary_rows['baseline']['cutoff']
print('Verified profile-only Sprint entry, cutoff, pose and braking effects',flush=True)

fresh = runs['normal']['dynamics'][26]
cached = source_frame['movement_before_tick']['max_acceleration_cm_s2']/100
assert cached == source_frame['movement_after_tick']['acceleration_cm_s2']['x']/100 == 20
assert source_frame['movement_after_tick']['max_acceleration_cm_s2'] < 1700
v = fresh['world_velocity_at_dynamics'][0]
dt = fresh['delta']
assert fresh['planned_velocity'][0] == f32(v+f32(fresh['dynamics'][0]*dt))
cached_only_plan = f32(v+f32(cached*dt))
native_result = native[26]['velocity']['x']/100
assert abs(cached_only_plan-native_result) < 0.0001
assert abs(fresh['planned_velocity'][0]-native_result) > 0.1
d07 = dict(ordinal=26,native_cached_acceleration=cached,
           native_refreshed_acceleration=source_frame['movement_after_tick']['max_acceleration_cm_s2']/100,
           target_refreshed_acceleration=fresh['dynamics'][0],target_plan=fresh['planned_velocity'][0],
           target_cached_scalar_counterfactual=cached_only_plan,native_result=native_result)
save('actual-comparison.json',dict(runs=summary_rows,root_changed_bones=changed,d07=d07,
    restored_first_difference=first_restored_difference))
replay = load('replay-baseline.json')
changed_samples = {row['case']:[value['ordinal'] for value in row['changes']] for row in replay}
assert changed_samples=={'baseline':[27,30,66],'normal':[29,30,62,64,65,66,67,69]}
print('Verified D07 ordering witness and the D267 live sampler effects',flush=True)

decoded = {}
for name in [f'runtime-{label}/{file}.json' for label in labels for file in ('report','parameters')]:
    data = raw(name)
    path = root/name
    archive = path.with_suffix('.json.gz')
    if path.exists():
        archive.write_bytes(gzip.compress(data,compresslevel=6,mtime=0))
    assert gzip.decompress(archive.read_bytes())==data
    decoded[name] = dict(bytes=len(data),sha256=hashlib.sha256(data).hexdigest())
    print('Verified gzip '+name,flush=True)
env = json.loads(Path('tools/env.local.json').read_bytes())
als = Path(env['ue_project']).parent/'Plugins/ALS-Refactored/Source/ALS'
ue = Path(os.environ['ALS_UE_SOURCE_ROOT'])
source_facts = []
for key,path,ranges in (
    ('source:Private/AlsCharacterMovementComponent.cpp',als/'Private/AlsCharacterMovementComponent.cpp',[(292,301),(330,341)]),
    ('Engine/Source/Runtime/Engine/Private/Components/CharacterMovementComponent.cpp',
     ue/'Engine/Source/Runtime/Engine/Private/Components/CharacterMovementComponent.cpp',[(6442,6457),(8134,8137)]),
    ('src/als/settings/params.rs',Path('src/als/settings/params.rs'),[(78,99),(500,547)]),
    ('src/als/character/movement/dynamics.rs',Path('src/als/character/movement/dynamics.rs'),[(543,566)]),
    ('tools/export/fit_ground_dynamics_curves.py',Path('tools/export/fit_ground_dynamics_curves.py'),[(34,44),(60,82)]),
):
    data = path.read_bytes()
    lines = data.decode().splitlines()
    source_facts.append(dict(key=key,sha256=hashlib.sha256(data).hexdigest(),
        excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b])+'\n') for a,b in ranges]))
save('source-facts.json',source_facts)
save('final-source.json',dict(sha256=hashlib.sha256((root/'replay.rs').read_bytes()).hexdigest()))
paths = [parent/name for name in (
    '484/playground_observed.rs','484/foot_observer.rs','505/observer.py','505/velocity_observer.rs',
    '509/observer.py','509/lean_observer.rs','510/observer.py','510/acceleration_observer.rs',
    '512/observer.py','512/sprint_observer.rs','512/native-sprint.json','512/sprint.native.json.gz',
    '512/verify.py','441a/vector.request.json','491/reader-migration.json','441c/library-identities.json')]
external = [dict(path=str(path.resolve().relative_to(Path.cwd())),bytes=path.stat().st_size,
                 sha256=hashlib.sha256(path.read_bytes()).hexdigest()) for path in paths]
save('summary.json',dict(
    gate='PASS_SPRINT_D05_PROFILE_AND_D07_D267_RESIDUALS',
    findings=['BF144-D05','BF144-D07','BF144-D267'],new_differences=0,total_differences=283,
    runs=summary_rows,root_changed_bones_at_25=len(changed),d267_changed_motor_samples=changed_samples,
    d07=d07,restored_first_difference=first_restored_difference,screenshots=0,performance_acceptance='paused',
    decoded=decoded,external_source_files=external))
