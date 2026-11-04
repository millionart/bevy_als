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

assert load('build-barrier.report.json')['success']
assert load('compile-observed.report.json')['success']
assert load('wall.native.json.reader.report.json')['success']
assert load('wall.package.json')['package'].endswith('_519')
native = {}
cases = {case['name']:case['frames'] for case in load('wall.native.json')['character_animation_probe']['cases']}
reference = cases['unblocked'][49]['movement_before_tick']
assert reference['movement_mode_name']=='MOVE_Walking' and reference['walkable_floor']
assert reference['velocity_cm_s']['z']==0
for label,rows in cases.items():
    row,next_row = rows[49:51]
    before,after = row['movement_before_tick'],row['movement_after_tick']
    assert before==reference
    assert after['movement_mode_name']=='MOVE_Falling' and not after['walkable_floor']
    dt = row['world_delta_seconds']
    assert dt==f32(1/30)
    pre,post=before['last_update_location_cm'],after['last_update_location_cm']
    distance=math.hypot(post['x']-pre['x'],post['y']-pre['y'])/100
    vertical=after['velocity_cm_s']['z']/100
    gravity=row['gravity_cm_s2']/100
    air_time=vertical/gravity
    next_vertical=next_row['movement_after_tick']['velocity_cm_s']['z']/100
    assert abs(next_vertical-(vertical+gravity*next_row['world_delta_seconds']))<1e-10
    if label=='unblocked':
        assert air_time==0
        assert abs(distance-after['velocity_cm_s']['x']/100*dt)<1e-12
    else:
        assert .005<air_time<.012 and distance<native['unblocked']['distance']
    native[label]=dict(distance=distance,vertical=vertical,air_time=air_time,next_vertical=next_vertical,
        pre_velocity=before['velocity_cm_s'],post_velocity=after['velocity_cm_s'],delta=dt)
assert native['wall-162']['air_time']>native['wall-166']['air_time']>native['wall-170']['air_time']
print('Verified actual native wall refunds against the zero-time control',flush=True)

labels=('wall-162','wall-166','wall-170','wall-162-no-spring','wall-162-restored')
target={}
stages={}
runs={}
prior_baseline=json.loads(gzip.decompress((parent/'518/runtime-baseline/parameters.json.gz').read_bytes()))
for label in labels:
    for name in (f'{label}-functional-process.json',f'runtime-{label}/process.report.json'):
        assert load(name)['success']
    gate=load(f'{label}-functional.json')
    assert gate['pass'] and gate['images']==[] and gate['validation_scope']=='functional'
    path=f'runtime-{label}'
    report=load(f'{path}/parameters.json')
    runs[label]={key:table(rows) for key,rows in report.items()}
    stage={name:load(f'{path}/vertical-{name}.json') for name in
           ('before','after-transaction','after-motor','integration','raw','published')}
    pre,tx,motor,integ,out,post=(stage[key] for key in (
        'before','after-transaction','after-motor','integration','raw','published'))
    barrier=load(f'{path}/barrier.json')
    assert barrier['wall_x_cm']==int(label.split('-')[1]) and barrier['yaw_source_degrees']==45
    assert barrier['size']==[.04,4,2]
    assert pre['substeps']==1 and pre['support']==tx['sensor_base_velocity']==[0,0,0]
    assert pre['mode']==tx['mode']==out['mode']=='Grounded' and post['mode']=='Air'
    assert tx['collision_velocity'] is None
    assert pre['velocity'][1]==motor['velocity'][1]==0
    assert tx['neutralized']==(label.endswith('no-spring'))
    assert tx['original_y']==f32(tx['spring']-tx['tracker_gravity'][1])
    assert tx['effective_y']==(-tx['tracker_gravity'][1] if tx['neutralized'] else tx['original_y'])
    assert tx['boost_y']==0
    assert post['velocity'][1]==out['velocity'][1]
    v=integ['initial_solver_velocity'][1]
    for _ in range(integ['substeps']):
        v=f32(v+integ['linear_increment'][1])
    assert v==post['velocity'][1]
    if tx['neutralized']:
        assert v==0 and integ['linear_increment'][1]==0
    else:
        assert v==0.30395272374153137
    row=runs[label]['after'][49]
    assert row['gameplay']==post['mode'] and row['velocity']==post['velocity']
    assert row['position']==post['position'] and row['graph_error'] is None
    desired=math.hypot(pre['planned_move'][0],pre['planned_move'][2])
    distance=math.hypot(post['position'][0]-pre['position'][0],post['position'][2]-pre['position'][2])
    assert distance<desired
    remaining=row['velocity_blend']['delta']*(1-distance/desired)
    assert .004<remaining<.012
    target[label]=dict(desired_distance=desired,actual_distance=distance,source_rule_time=remaining,
        published_vertical=v,next_vertical=runs[label]['after'][50]['velocity'][1])
    for key,rows in runs[label].items():
        assert [row for i,row in rows.items() if i<49]==[
            row for row in prior_baseline[key] if row['ordinal']<49]
    stages[label]=stage
base,control,restored=(stages[label] for label in ('wall-162','wall-162-no-spring','wall-162-restored'))
assert base==restored
assert raw('runtime-wall-162/parameters.json')==raw('runtime-wall-162-restored/parameters.json')
assert base['before']==control['before']
assert {k:v for k,v in base['after-transaction'].items() if k not in ('effective_y','neutralized')}=={
    k:v for k,v in control['after-transaction'].items() if k not in ('effective_y','neutralized')}
for key in ('position','velocity'):
    assert base['published'][key][::2]==control['published'][key][::2]
assert abs(target['wall-162-no-spring']['next_vertical']+.327)<1e-7
assert native['wall-162']['vertical'] < -0.09
assert native['wall-162']['next_vertical'] < -0.42
save('actual-comparison.json',dict(native=native,target=target,wall_162={
    'baseline':base,'no_spring':control,'restored':restored}))
print('Verified target partial move, spring ablation, lost air budget and restoration',flush=True)

decoded={}
for name in ['wall.native.json']+[f'runtime-{label}/{file}.json'
        for label in labels for file in ('report','parameters')]:
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
facts=[]
for key,path,ranges in (
    ('source:Private/AlsCharacterMovementComponent.cpp',als/'Private/AlsCharacterMovementComponent.cpp',[(410,442),(546,558)]),
    ('Engine/Source/Runtime/Engine/Private/Components/CharacterMovementComponent.cpp',
     ue/'Engine/Source/Runtime/Engine/Private/Components/CharacterMovementComponent.cpp',[(5427,5457)]),
    ('src/als/character/movement/motion.rs',Path('src/als/character/movement/motion.rs'),[(3817,3834),(4066,4105)]),
):
    data=path.read_bytes()
    lines=data.decode().splitlines()
    facts.append(dict(key=key,sha256=hashlib.sha256(data).hexdigest(),
        excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b])+'\n') for a,b in ranges]))
save('source-facts.json',facts)
save('final-source.json',dict(sha256=hashlib.sha256((root/'wall_fixture.inc').read_bytes()).hexdigest()))
paths=[parent/name for name in (
    '484/playground_observed.rs','484/foot_observer.rs','505/observer.py','505/velocity_observer.rs',
    '509/observer.py','509/lean_observer.rs','510/observer.py','510/acceleration_observer.rs',
    '512/observer.py','512/sprint_observer.rs','513/observer.py','513/profile.rs',
    '516/observer.py','516/ledge_observer.rs','516/prepare_reader.py','516/late.request.json',
    '518/observer.py','518/vertical_observer.rs','518/runtime-baseline/parameters.json.gz',
    '491/reader-migration.json','441c/library-identities.json')]
external=[dict(path=str(path.resolve().relative_to(Path.cwd())),bytes=path.stat().st_size,
               sha256=hashlib.sha256(path.read_bytes()).hexdigest()) for path in paths]
save('summary.json',dict(
    gate='PASS_D289_PARTIAL_WALK_OFF_AIR_TIME',difference='BF144-D289',independent_of='BF144-D288',
    new_differences=1,total_differences=286,ordinal=49,native=native,target=target,
    restored_parameters_exact=True,screenshots=0,performance_acceptance='paused',
    decoded=decoded,external_source_files=external))
