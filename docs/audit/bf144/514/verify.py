import gzip
import hashlib
import json
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

def save(name,value):
    (root/name).write_text(json.dumps(value,indent=2)+'\n')

def f32(value):
    return struct.unpack('<f',struct.pack('<f',value))[0]

def table(rows):
    return {row['ordinal']:row for row in rows}

native_report = json.loads(gzip.decompress((parent/'512/sprint.native.json.gz').read_bytes()))
source = native_report['character_animation_probe']['cases'][0]['frames'][26]
before = source['movement_before_tick']
after = source['movement_after_tick']
source_cache = before['max_acceleration_cm_s2']/100
source_fresh = after['max_acceleration_cm_s2']/100
source_speed = source['actor_velocity_cm_s']['x']/100
assert source_cache==after['acceleration_cm_s2']['x']/100==20
assert source_fresh<source_cache
native_sprint = json.loads((parent/'512/native-sprint.json').read_bytes())[26]
assert native_sprint['acceleration']==1.0
assert source['world_delta_seconds']==f32(1/30)

assert not load('compile-duplicate-imports.report.json')['success']
assert load('compile-preexisting-move-start.report.json')['success']
assert load('runtime-preexisting-move-start/process.report.json')['process_exit_code']==101
assert load('compile-observed.report.json')['success']
labels = ('baseline','cached','restored')
runs = {}
stages = {}
for label in labels:
    for name in (f'{label}-functional-process.json',f'runtime-{label}/process.report.json'):
        assert load(name)['success']
    functional = load(f'{label}-functional.json')
    assert functional['pass'] and functional['images']==[] and functional['validation_scope']=='functional'
    profile = load(f'runtime-{label}/profile.json')
    assert profile['effective']['responsive']==profile['original']['normal']
    data = load(f'runtime-{label}/parameters.json')
    runs[label] = {key:table(rows) for key,rows in data.items()}
    stages[label] = load(f'runtime-{label}/stage.json')
    stage = stages[label]
    assert stage['ordinal']==26 and stage['before_collision_transaction']
    assert stage['previous_acceleration']==source_cache and stage['delta']==source['world_delta_seconds']
    assert stage['prior_move_start'] != stage['initial_position']
    assert stage['applied']==(label=='cached')
    assert stage['recorded_plan']==stage['cached_plan' if label=='cached' else 'baseline_plan']
    assert stage['recorded_movement']==[f32(value*stage['delta']) for value in stage['recorded_plan']]
    actual = runs[label]['dynamics'][26]
    assert actual['dynamics']==actual['public']==stage['fresh']
    assert actual['planned_velocity']==stage['recorded_plan']
    assert actual['motion']==stage['desired'] and actual['max_speed']==stage['speed']
    animation = runs[label]['after'][26]
    assert animation['acceleration']['max_acceleration']==stage['fresh'][0]
    assert animation['acceleration']['max_braking']==stage['fresh'][1]
    assert animation['graph_error'] is None

def common(stage):
    return {key:value for key,value in stage.items() if key not in ('applied','recorded_plan','recorded_movement')}
assert common(stages['baseline'])==common(stages['cached'])==common(stages['restored'])
assert stages['baseline']==stages['restored']
baseline,cached,restored = (runs[label] for label in labels)
for domain in baseline:
    assert [row for i,row in baseline[domain].items() if i<26] == [
            row for i,row in cached[domain].items() if i<26]
    assert baseline[domain][26]==restored[domain][26]
old = json.loads(gzip.decompress((parent/'513/runtime-normal/parameters.json.gz').read_bytes()))
assert all(baseline[key][26]==table(old[key])[26] for key in baseline)
first_restored_difference = {domain:next((i for i,row in baseline[domain].items()
        if row!=restored[domain][i]),None) for domain in baseline}
assert all(i is None or i>26 for i in first_restored_difference.values())

a,b = baseline['after'][26],cached['after'][26]
stage = stages['baseline']
assert all(a[key]==b[key] for key in ('gait','desired_gait','gameplay','graph_tag','input','animation_frame'))
assert a['gameplay']=='Grounded' and a['gait'].endswith('.Sprinting')
vel_a,vel_b = (row['velocity'][0] for row in (a,b))
pos_a,pos_b = (row['position'][0] for row in (a,b))
assert abs(vel_b-source_speed)<0.0001 and abs(vel_a-source_speed)>0.1
assert abs((pos_b-pos_a) - (stages['cached']['recorded_movement'][0]-stage['recorded_movement'][0]))<1e-6
for row in (a,b):
    assert row['sprint']['time']==f32(2/30)
    assert row['sprint']['acceleration']==row['acceleration']['standing_acceleration']
    assert row['sprint']['mixed'][0]['alpha']==1
assert b['acceleration']['acceleration'][0]>a['acceleration']['acceleration'][0]
assert abs(b['acceleration']['acceleration'][0]-source_cache)<0.001
assert a['sprint']['acceleration']!=b['sprint']['acceleration']
clips = [row['sprint_leaves'][0]['clocks'][0]['relevant']['position_seconds'] for row in (a,b)]
assert clips[0]!=clips[1]
poses = [row['rig_poses']['root_result']['pose'] for row in (a,b)]
assert [bone['name'] for bone in poses[0]['bones']]==[bone['name'] for bone in poses[1]['bones']]
changed = [x['name'] for x,y in zip(poses[0]['bones'],poses[1]['bones'],strict=True) if x!=y]
assert len(changed)==73 and None not in changed
save('actual-comparison.json',dict(
    native=dict(previous_maximum=source_cache,refreshed_maximum=source_fresh,actual_speed=source_speed),
    target={label:dict(stage=stages[label],after=runs[label]['after'][26]) for label in labels},
    changed_root_bones=changed,restored_first_difference=first_restored_difference))
print('Verified actual collision consumption, animation reads, and exact witness restoration',flush=True)

decoded = {}
for name in [f'runtime-{label}/{file}.json'
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
source_facts = []
for key,path,ranges in (
    ('source:Private/AlsCharacterMovementComponent.cpp',als/'Private/AlsCharacterMovementComponent.cpp',[(292,301),(333,340)]),
    ('Engine/Source/Runtime/Engine/Private/Components/CharacterMovementComponent.cpp',
     ue/'Engine/Source/Runtime/Engine/Private/Components/CharacterMovementComponent.cpp',[(6442,6457),(8134,8137)]),
    ('src/als/character/movement/dynamics.rs',Path('src/als/character/movement/dynamics.rs'),[(170,178),(543,586)]),
    ('src/als/character/movement/motion.rs',Path('src/als/character/movement/motion.rs'),[(2550,2558),(3748,3767),(3811,3831)]),
    ('src/als/character/animation_instances/ab_als_provider.rs',Path('src/als/character/animation_instances/ab_als_provider.rs'),[(315,341)]),
):
    data = path.read_bytes()
    lines = data.decode().splitlines()
    source_facts.append(dict(key=key,sha256=hashlib.sha256(data).hexdigest(),
        excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b])+'\n') for a,b in ranges]))
save('source-facts.json',source_facts)
save('final-source.json',dict(sha256=hashlib.sha256((root/'cache_control.rs').read_bytes()).hexdigest()))
paths = [parent/name for name in (
    '484/playground_observed.rs','484/foot_observer.rs','505/observer.py','505/velocity_observer.rs',
    '509/observer.py','509/lean_observer.rs','510/observer.py','510/acceleration_observer.rs',
    '512/observer.py','512/sprint_observer.rs','512/native-sprint.json','512/sprint.native.json.gz',
    '513/observer.py','513/profile.rs','513/runtime-normal/parameters.json.gz',
    '491/reader-migration.json','441c/library-identities.json')]
external = [dict(path=str(path.resolve().relative_to(Path.cwd())),bytes=path.stat().st_size,
                 sha256=hashlib.sha256(path.read_bytes()).hexdigest()) for path in paths]
save('summary.json',dict(
    gate='PASS_D07_CACHED_ACCELERATION_ACTUAL_MOVEMENT_AND_ANIMATION',
    findings=['BF144-D07'],new_differences=0,total_differences=283,
    native_speed=source_speed,target_current_speed=vel_a,target_cached_speed=vel_b,
    native_cached_maximum=source_cache,native_refreshed_maximum=source_fresh,
    target_fresh_maximum=stage['fresh'][0],target_sprint_clip_time=clips,
    root_changed_bones=len(changed),restored_witness_exact=True,
    restored_first_difference=first_restored_difference,screenshots=0,performance_acceptance='paused',
    decoded=decoded,external_source_files=external))
