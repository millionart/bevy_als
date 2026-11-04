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

native_report = json.loads(gzip.decompress((parent/'516/late.native.json.gz').read_bytes()))
native_case = next(row for row in native_report['character_animation_probe']['cases'] if row['name']=='late-ledge-100')
n = native_case['frames'][49]
n_next = native_case['frames'][50]
a,b = n['movement_before_tick'],n['movement_after_tick']
assert a['movement_mode_name']=='MOVE_Walking' and b['movement_mode_name']=='MOVE_Falling'
assert a['walkable_floor'] and not b['walkable_floor']
assert a['velocity_cm_s']['z']==b['velocity_cm_s']['z']==0
native_delta = (b['last_update_location_cm']['x']-a['last_update_location_cm']['x'])/100
assert abs(native_delta-b['velocity_cm_s']['x']/100*n['world_delta_seconds'])<1e-12
assert n_next['actor_velocity_cm_s']['z']==n_next['gravity_cm_s2']*n_next['world_delta_seconds']
print('Verified native full horizontal step and zero transition vertical speed',flush=True)

labels=('baseline','no-spring','restored','supported')
stages={}
runtime={}
assert load('compile-observed.report.json')['success']
for label in labels:
    for name in (f'{label}-functional-process.json',f'runtime-{label}/process.report.json'):
        assert load(name)['success']
    gate=load(f'{label}-functional.json')
    assert gate['pass'] and gate['images']==[] and gate['validation_scope']=='functional'
    path=f'runtime-{label}'
    fixture=load(f'{path}/fixture.json')
    assert fixture['half_width_cm']==(150 if label=='supported' else 100)
    runtime[label]={key:table(rows) for key,rows in load(f'{path}/parameters.json').items()}
    stage={key:load(f'{path}/vertical-{key}.json') for key in (
        'before','after-transaction','after-motor','integration','raw','published')}
    pre,tx,motor,coeff,out,post=(stage[k] for k in (
        'before','after-transaction','after-motor','integration','raw','published'))
    assert pre['mode']==tx['mode']==out['mode']=='Grounded'
    assert pre['substeps']==1 and pre['support']==tx['sensor_base_velocity']==[0,0,0]
    assert pre['velocity'][1]==tx['velocity'][1]==motor['velocity'][1]==0
    assert tx['boost_y']==motor['boost'][1]==0
    assert tx['collision_velocity'] is None
    assert f32(f32(tx['float_height']-tx['proximity'])*tx['strength'])==tx['spring']
    gravity=tx['tracker_gravity'][1]
    if label=='supported':
        assert pre['motor_acceleration'][1]!=tx['original_y']
        assert tx['original_y']==-gravity and post['mode']=='Grounded'
    else:
        assert tx['original_y']==pre['motor_acceleration'][1]==f32(tx['spring']-gravity)
        assert post['mode']=='Air'
        assert post['velocity'][1]==out['velocity'][1]
    assert tx['neutralized']==(label=='no-spring')
    assert tx['effective_y']==(-gravity if label=='no-spring' else tx['original_y'])
    assert motor['acceleration'][1]==tx['effective_y']
    frame_dt=runtime[label]['after'][49]['velocity_blend']['delta']
    capped_dt=f32(33_333_333*1e-9)
    assert frame_dt>capped_dt
    ratio=f32(frame_dt/capped_dt)
    expected_increment=f32(f32(f32(motor['acceleration'][1]+gravity)*coeff['substep_seconds'])*ratio)
    assert coeff['linear_damping_rhs']==1 and coeff['substeps']==6
    assert coeff['linear_increment'][1]==expected_increment
    v=coeff['initial_solver_velocity'][1]
    y=motor['position'][1]
    for _ in range(coeff['substeps']):
        v=f32(v+coeff['linear_increment'][1])
        y=f32(y+f32(v*coeff['substep_seconds']))
    assert out['velocity'][1]==v and out['position'][1]==y
    row=runtime[label]['after'][49]
    assert row['velocity']==post['velocity'] and row['gameplay']==post['mode']
    assert row['position']==post['position']
    stages[label]=stage

base,control,restored=(stages[label] for label in ('baseline','no-spring','restored'))
assert base==restored
assert raw('runtime-baseline/parameters.json')==raw('runtime-restored/parameters.json')
assert base['before']==control['before']
assert {k:v for k,v in base['after-transaction'].items() if k not in ('effective_y','neutralized')} == {
        k:v for k,v in control['after-transaction'].items() if k not in ('effective_y','neutralized')}
assert base['published']['velocity'][1]>0.3 and control['published']['velocity'][1]==0
for key in ('position','velocity'):
    assert base['published'][key][::2]==control['published'][key][::2]
assert abs(runtime['no-spring']['after'][50]['velocity'][1]-n_next['actor_velocity_cm_s']['z']/100)<1e-7
old=json.loads(gzip.decompress((parent/'517/runtime-ledge-100/parameters.json.gz').read_bytes()))
assert runtime['baseline']['after'][49]==table(old['after'])[49]
save('vertical-boundary.json',dict(native_velocity=[b['velocity_cm_s']['x']/100,0,0],target=stages))
print('Verified spring-only causal control, supported guard and exact restoration',flush=True)

decoded={}
for name in [f'runtime-{label}/{file}.json' for label in labels for file in ('report','parameters')]:
    data=raw(name)
    path=root/name
    archive=path.with_suffix('.json.gz')
    if path.exists():
        archive.write_bytes(gzip.compress(data,compresslevel=6,mtime=0))
    assert gzip.decompress(archive.read_bytes())==data
    decoded[name]=dict(bytes=len(data),sha256=hashlib.sha256(data).hexdigest())
    print('Verified gzip '+name,flush=True)
env=json.loads(Path('tools/env.local.json').read_bytes())
als=Path(env['ue_project']).parent/'Plugins/ALS-Refactored/Source/ALS'
ue=Path(os.environ['ALS_UE_SOURCE_ROOT'])
tnua=next((Path.home()/'.cargo/git/checkouts').glob('bevy-tnua-*/9f83188'))
facts=[]
for key,path,ranges in (
    ('source:Private/AlsCharacterMovementComponent.cpp',als/'Private/AlsCharacterMovementComponent.cpp',[(521,557)]),
    ('Engine/Source/Runtime/Engine/Private/Components/CharacterMovementComponent.cpp',
     ue/'Engine/Source/Runtime/Engine/Private/Components/CharacterMovementComponent.cpp',[(5427,5457)]),
    ('Tnua/src/builtins/walk.rs',tnua/'src/builtins/walk.rs',[(407,424),(623,646)]),
    ('Tnua/avian3d/src/lib.rs',tnua/'avian3d/src/lib.rs',[(419,452)]),
    ('src/als/character/movement/motion.rs',Path('src/als/character/movement/motion.rs'),[(3817,3834),(3981,3987),(4066,4105)]),
    ('src/als/character/movement/source_physics_timing.rs',Path('src/als/character/movement/source_physics_timing.rs'),[(12,13),(81,97)]),
):
    data=path.read_bytes()
    lines=data.decode().splitlines()
    facts.append(dict(key=key,sha256=hashlib.sha256(data).hexdigest(),
        excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b])+'\n') for a,b in ranges]))
save('source-facts.json',facts)
save('final-source.json',dict(sha256=hashlib.sha256((root/'vertical_observer.rs').read_bytes()).hexdigest()))
paths=[parent/name for name in (
    '484/playground_observed.rs','484/foot_observer.rs','505/observer.py','505/velocity_observer.rs',
    '509/observer.py','509/lean_observer.rs','510/observer.py','510/acceleration_observer.rs',
    '512/observer.py','512/sprint_observer.rs','513/observer.py','513/profile.rs',
    '516/observer.py','516/ledge_observer.rs','516/late.native.json.gz',
    '517/runtime-ledge-100/parameters.json.gz','491/reader-migration.json','441c/library-identities.json')]
external=[dict(path=str(path.resolve().relative_to(Path.cwd())),bytes=path.stat().st_size,
               sha256=hashlib.sha256(path.read_bytes()).hexdigest()) for path in paths]
save('summary.json',dict(
    gate='PASS_D288_SINGLE_STEP_WALK_OFF_VERTICAL_MOTOR',difference='BF144-D288',
    new_differences=1,total_differences=285,ordinal=49,native_vertical=0,
    baseline_vertical=base['published']['velocity'][1],controlled_vertical=control['published']['velocity'][1],
    spring_acceleration=base['after-transaction']['spring'],integrated_substeps=6,
    restored_parameters_exact=True,screenshots=0,performance_acceptance='paused',decoded=decoded,external_source_files=external))
