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

def table(rows):
    return {row['ordinal']:row for row in rows}

assert load('sprint.native.json.reader.report.json')['success']
native = load('sprint.native.json')['character_animation_probe']['cases'][0]['frames'][26]
source = json.loads(gzip.decompress((parent/'512/sprint.native.json.gz').read_bytes()))
old_source = source['character_animation_probe']['cases'][0]['frames'][26]
prior,source_after = native['movement_before_tick'],native['movement_after_tick']
assert prior==old_source['movement_before_tick']
assert native['world_delta_seconds']==f32(.06)
old_acceleration = prior['max_acceleration_cm_s2']/100
assert old_acceleration==source_after['acceleration_cm_s2']['x']/100==20
assert source_after['max_acceleration_cm_s2']==old_source['movement_after_tick']['max_acceleration_cm_s2']
assert source_after['max_acceleration_cm_s2']/100<old_acceleration
source_position_before = prior['last_update_location_cm']['x']/100
source_displacement = source_after['last_update_location_cm']['x']/100-source_position_before
source_speed = native['actor_velocity_cm_s']['x']/100
source_initial_speed = prior['velocity_cm_s']['x']/100
half = native['world_delta_seconds']*0.5
first = source_initial_speed+old_acceleration*half
second = first+old_acceleration*half
assert abs(source_speed-second)<1e-6
assert abs(source_displacement-(first+second)*half)<1e-6
assert abs(source_displacement-second*native['world_delta_seconds'])>0.01
print('Verified source cached acceleration and two-step displacement',flush=True)

labels = ('baseline','plan-only','cached','restored')
runs = {}
stages = {}
for label in labels:
    for name in (f'{label}-functional-process.json',f'runtime-{label}/process.report.json'):
        assert load(name)['success']
    functional = load(f'{label}-functional.json')
    assert functional['pass'] and functional['images']==[] and functional['validation_scope']=='functional'
    profile = load(f'runtime-{label}/profile.json')
    assert profile['effective']['responsive']==profile['original']['normal']
    report = load(f'runtime-{label}/parameters.json')
    runs[label] = {key:table(rows) for key,rows in report.items()}
    before = load(f'runtime-{label}/stage-before.json')
    after = load(f'runtime-{label}/stage-after.json')
    assert before['ordinal']==after['ordinal']==26
    assert before['delta']==native['world_delta_seconds'] and before['steps']==[half,half]
    assert before['cached_acceleration']==old_acceleration
    assert before['control']==(label if label in ('plan-only','cached') else None)
    assert before['transaction_acceleration']==after['transaction_acceleration']
    assert after['transaction_acceleration']==(20 if label=='cached' else before['fresh'][0])
    assert after['published_acceleration']==before['fresh'][0]
    preplan = before['cached_plan' if label=='plan-only' else 'baseline_plan']
    assert before['preplan']==preplan[1] and before['preplan_displacement']==preplan[2]
    assert after['substeps']==2
    row = runs[label]['after'][26]
    assert row['velocity']==after['collision_velocity']
    assert row['acceleration']['max_acceleration']==after['published_acceleration']
    assert row['sprint']['mixed'][0]['alpha']==1 and row['sprint']['time']==f32(f32(1/30)+f32(.06))
    assert runs[label]['dynamics'][26]['planned_velocity']==after['last_input']
    assert runs[label]['dynamics'][26]['dynamics']==before['fresh']
    assert row['gameplay']=='Grounded' and row['gait'].endswith('.Sprinting') and row['graph_error'] is None
    stages[label] = dict(before=before,after=after)
assert load('compile-observed.report.json')['success']
base = stages['baseline']
for label,stage in stages.items():
    assert {k:v for k,v in stage['before'].items() if k not in
            ('control','transaction_acceleration','preplan','preplan_displacement')} == {
            k:v for k,v in base['before'].items() if k not in
            ('control','transaction_acceleration','preplan','preplan_displacement')}
assert stages['plan-only']['before']['preplan']!=base['before']['preplan']
assert stages['plan-only']['after']==base['after']
assert stages['restored']==base
assert stages['cached']['before']['preplan']==base['before']['preplan']
assert stages['cached']['after']['collision_velocity']!=base['after']['collision_velocity']
for key in runs['baseline']:
    for label in labels:
        assert [row for i,row in runs[label][key].items() if i<26] == [
            row for i,row in runs['baseline'][key].items() if i<26]
    assert runs['plan-only'][key][26]==runs['restored'][key][26]==runs['baseline'][key][26]

normal,changed = (runs[label]['after'][26] for label in ('baseline','cached'))
start_x = base['before']['position'][0]
motion_normal = normal['position'][0]-start_x
motion_cached = changed['position'][0]-start_x
assert abs(motion_cached-source_displacement)<1e-5
assert abs(motion_normal-source_displacement)>0.008
assert abs(changed['velocity'][0]-source_speed)<0.0001
assert abs(normal['velocity'][0]-source_speed)>0.18
assert abs(changed['acceleration']['acceleration'][0]-20)<0.001
assert changed['sprint']['acceleration']!=normal['sprint']['acceleration']
clips = [row['sprint_leaves'][0]['clocks'][0]['relevant']['position_seconds'] for row in (normal,changed)]
assert clips[0]!=clips[1]
poses = [row['rig_poses']['root_result']['pose'] for row in (normal,changed)]
assert [x['name'] for x in poses[0]['bones']]==[x['name'] for x in poses[1]['bones']]
changed_bones = [x['name'] for x,y in zip(poses[0]['bones'],poses[1]['bones'],strict=True) if x!=y]
assert len(changed_bones)==73 and None not in changed_bones
restored_diff = {key:next((i for i,row in rows.items() if row!=runs['restored'][key][i]),None)
                 for key,rows in runs['baseline'].items()}
assert all(i is None or i>26 for i in restored_diff.values())
save('actual-comparison.json',dict(native=dict(velocity=source_speed,displacement=source_displacement,
        acceleration=old_acceleration,refreshed_acceleration=source_after['max_acceleration_cm_s2']/100),
    target={label:dict(stage=stage,output=runs[label]['after'][26]) for label,stage in stages.items()},
    changed_root_bones=changed_bones,restored_first_difference=restored_diff))
print('Verified that only transaction acceleration changes actual two-step movement',flush=True)

decoded = {}
for name in ['sprint.native.json'] + [f'runtime-{label}/{file}.json'
        for label in labels for file in ('report','parameters')]:
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
facts = []
for key,path,ranges in (
    ('source:Private/AlsCharacterMovementComponent.cpp',als/'Private/AlsCharacterMovementComponent.cpp',[(333,341),(372,411)]),
    ('Engine/Source/Runtime/Engine/Private/Components/CharacterMovementComponent.cpp',
     ue/'Engine/Source/Runtime/Engine/Private/Components/CharacterMovementComponent.cpp',[(6442,6457),(8162,8186)]),
    ('src/als/character/movement/dynamics.rs',Path('src/als/character/movement/dynamics.rs'),[(282,311),(543,586)]),
    ('src/als/character/movement/motion.rs',Path('src/als/character/movement/motion.rs'),[(3620,3644),(3762,3808)]),
):
    data = path.read_bytes()
    lines = data.decode().splitlines()
    facts.append(dict(key=key,sha256=hashlib.sha256(data).hexdigest(),
        excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b])+'\n') for a,b in ranges]))
save('source-facts.json',facts)
save('final-source.json',dict(sha256=hashlib.sha256((root/'cache_control.rs').read_bytes()).hexdigest()))
paths = [parent/name for name in (
    '484/playground_observed.rs','484/foot_observer.rs','505/observer.py','505/velocity_observer.rs',
    '509/observer.py','509/lean_observer.rs','510/observer.py','510/acceleration_observer.rs',
    '512/observer.py','512/sprint_observer.rs','512/sprint.request.json','512/sprint.native.json.gz',
    '513/observer.py','513/profile.rs','514/summary.json','491/reader-migration.json','441c/library-identities.json')]
external = [dict(path=str(path.resolve().relative_to(Path.cwd())),bytes=path.stat().st_size,
                 sha256=hashlib.sha256(path.read_bytes()).hexdigest()) for path in paths]
save('summary.json',dict(
    gate='PASS_D07_TWO_STEP_ACTUAL_WALKING_TRANSACTION',
    findings=['BF144-D07'],new_differences=0,total_differences=283,
    steps=2,ordinal=26,native_speed=source_speed,target_current_speed=normal['velocity'][0],
    target_cached_speed=changed['velocity'][0],native_displacement=source_displacement,
    target_current_displacement=motion_normal,target_cached_displacement=motion_cached,
    preplan_only_ignored=True,target_sprint_clip_time=clips,root_changed_bones=len(changed_bones),
    restored_witness_exact=True,restored_first_difference=restored_diff,
    screenshots=0,performance_acceptance='paused',decoded=decoded,external_source_files=external))
