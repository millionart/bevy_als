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

assert load('wall.native.json.reader.report.json')['success']
assert load('wall.package.json')['sha256']==json.loads((parent/'519/wall.package.json').read_bytes())['sha256']
prior = json.loads(gzip.decompress((parent/'519/wall.native.json.gz').read_bytes()))
previous = {case['name']:case['frames'][49] for case in prior['character_animation_probe']['cases']}
source = load('wall.native.json')['character_animation_probe']['cases']
native = {}
for case in source:
    name=case['name']
    row=case['frames'][49]
    a,b=row['movement_before_tick'],row['movement_after_tick']
    assert a==previous[name]['movement_before_tick']
    assert row['world_delta_seconds']==f32(.06)
    assert a['movement_mode_name']=='MOVE_Walking' and b['movement_mode_name']=='MOVE_Falling'
    assert a['velocity_cm_s']['z']==0
    assert b['acceleration_cm_s2']['x']==a['max_acceleration_cm_s2']
    velocity=[b['velocity_cm_s'][key]/100 for key in ('x','z','y')]
    time=b['velocity_cm_s']['z']/row['gravity_cm_s2']
    if name=='unblocked':
        assert time==f32(.06)*.5
    else:
        assert .033<time<.042 and abs(velocity[0]+velocity[2])<1e-5
    native[name]=dict(air_time=time,velocity=velocity,walking_cache=a['max_acceleration_cm_s2']/100)
assert native['wall-162']['air_time']>native['wall-166']['air_time']>native['wall-170']['air_time']
print('Verified actual UE wall and unblocked multi-step time budgets',flush=True)

labels=('wall-162','wall-166','wall-170','wall-162-cached','wall-162-restored','unblocked',
        'wall-162-aligned-x','wall-162-aligned-walking','wall-162-aligned-restored',
        'wall-162-aligned-air','wall-162-final-restored')
for tag in ('observed','alignment','aligned-air'):
    assert load(f'compile-{tag}.report.json')['success']
target={}
stages={}
runs={}
for label in labels:
    for name in (f'{label}-functional-process.json',f'runtime-{label}/process.report.json'):
        assert load(name)['success']
    gate=load(f'{label}-functional.json')
    assert gate['pass'] and gate['images']==[] and gate['validation_scope']=='functional'
    p=f'runtime-{label}'
    data=load(f'{p}/parameters.json')
    runs[label]={k:table(rows) for k,rows in data.items()}
    stage={key:load(f'{p}/{key}.json') for key in (
        'stage-before','stage-after','vertical-before','vertical-after-transaction',
        'vertical-integration','vertical-raw','vertical-published')}
    before,after,tx,coeff,raw_body,published=(stage[key] for key in (
        'stage-before','stage-after','vertical-after-transaction','vertical-integration',
        'vertical-raw','vertical-published'))
    assert before['mode']=='Grounded' and before['substeps']==after['substeps']==2
    assert before['delta']==f32(.06)
    assert stage['vertical-before']['velocity'][1]==0 and before['support']==[0,0,0]
    assert after['mode']==tx['mode']==raw_body['mode']==published['mode']=='Air'
    assert tx['collision_velocity']==after['collision_velocity']==published['velocity']
    assert coeff['linear_increment']==[0,0,0] and raw_body['velocity']==[0,0,0]
    assert published['position']==after['position']
    assert runs[label]['after'][49]['velocity']==published['velocity']
    assert runs[label]['after'][49]['graph_error'] is None
    assert runs[label]['dynamics'][48]['delta']==runs[label]['dynamics'][49]['delta']==f32(.06)
    v=published['velocity']
    time=-v[1]/f32(9.81)
    if label=='unblocked':
        assert abs(time-.03)<1e-6 and abs(v[2])<1e-5
    else:
        assert .033<time<.042 and abs(v[0]+v[2])<.002
    target[label]=dict(air_time=time,velocity=v,walking_input=after['walking_last_input'],
                       fresh_acceleration=before['fresh_acceleration'],cached_acceleration=before['cached_acceleration'])
    stages[label]=stage
assert target['wall-162']['air_time']>target['wall-166']['air_time']>target['wall-170']['air_time']>target['unblocked']['air_time']

before=stages['wall-162']['stage-before']
cache=stages['wall-162-cached']['stage-before']
assert before['cached_acceleration']==cache['walking_acceleration']>before['fresh_acceleration']
assert before['planned_velocity']==cache['planned_velocity']
assert target['wall-162']['walking_input']!=target['wall-162-cached']['walking_input']

matched=load('alignment-input.json')
source_case=next(case for case in source if case['name']=='wall-162')
original=source_case['frames'][49]['movement_before_tick']
origin=source_case['frames'][0]['actor_transform']['translation']['x']
assert matched==dict(relative_x=(original['last_update_location_cm']['x']-origin)/100,
    velocity_x=original['velocity_cm_s']['x']/100,cached_acceleration=original['max_acceleration_cm_s2']/100)
for label in ('wall-162-aligned-x','wall-162-aligned-walking','wall-162-aligned-air'):
    path=f'runtime-{label}'
    row=load(f'{path}/alignment.json')
    fixture=load(f'{path}/fixture.json')
    assert row['original_position']==before['position']
    assert row['aligned_position']==[
        f32(fixture['origin'][0]+f32(matched['relative_x'])),*row['original_position'][1:]]
    if label!='wall-162-aligned-x':
        assert row['walking_velocity']==[f32(matched['velocity_x']),0,0]
        assert row['walking_acceleration']==f32(matched['cached_acceleration'])
    if label=='wall-162-aligned-air':
        assert row['air_maximum']==f32(matched['cached_acceleration'])
    assert stages[label]['stage-before']==before
assert abs(target['wall-162']['air_time']-native['wall-162']['air_time'])>.0007
assert abs(target['wall-162-aligned-x']['air_time']-native['wall-162']['air_time'])<.00004
assert abs(target['wall-162-aligned-walking']['air_time']-native['wall-162']['air_time'])<.000003
assert target['wall-162-aligned-walking']['walking_input']==target['wall-162-aligned-air']['walking_input']
assert abs(target['wall-162-aligned-air']['air_time']-native['wall-162']['air_time'])<.00002
assert abs(target['wall-162-aligned-walking']['velocity'][0]-native['wall-162']['velocity'][0])>.03
assert abs(target['wall-162-aligned-air']['velocity'][0]-native['wall-162']['velocity'][0])<.0003
assert abs(target['wall-162-aligned-air']['velocity'][2]-native['wall-162']['velocity'][2])<.0001
restored={}
for label in ('wall-162-restored','wall-162-aligned-restored','wall-162-final-restored'):
    assert stages['wall-162']==stages[label]
    assert all(runs['wall-162'][k][49]==runs[label][k][49] for k in runs['wall-162'])
    restored[label]=raw('runtime-wall-162/parameters.json')==raw(f'runtime-{label}/parameters.json')
save('actual-comparison.json',dict(native=native,target=target,alignment=matched,restored_parameters_exact=restored))
print('Verified positive refunds, air wall response, planar alignment and cached air control',flush=True)

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
    ('source:Private/AlsCharacterMovementComponent.cpp',als/'Private/AlsCharacterMovementComponent.cpp',[(372,411),(546,558)]),
    ('Engine/Source/Runtime/Engine/Private/Components/CharacterMovementComponent.cpp',
     ue/'Engine/Source/Runtime/Engine/Private/Components/CharacterMovementComponent.cpp',[(4827,4858),(5427,5457)]),
    ('src/als/character/movement/dynamics.rs',Path('src/als/character/movement/dynamics.rs'),[(303,327)]),
    ('src/als/character/movement/motion.rs',Path('src/als/character/movement/motion.rs'),[(2752,2779),(3770,3807)]),
):
    data=path.read_bytes()
    lines=data.decode().splitlines()
    facts.append(dict(key=key,sha256=hashlib.sha256(data).hexdigest(),
        excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b])+'\n') for a,b in ranges]))
save('source-facts.json',facts)
save('final-source.json',dict(sha256=hashlib.sha256((root/'align_walking.rs').read_bytes()).hexdigest()))
paths=[parent/name for name in (
    '484/playground_observed.rs','484/foot_observer.rs','505/observer.py','505/velocity_observer.rs',
    '509/observer.py','509/lean_observer.rs','510/observer.py','510/acceleration_observer.rs',
    '512/observer.py','512/sprint_observer.rs','513/observer.py','513/profile.rs',
    '516/observer.py','516/ledge_observer.rs','518/observer.py','518/vertical_observer.rs',
    '519/observer.py','519/wall_fixture.inc','519/wall.request.json','519/wall.native.json.gz','519/wall.package.json',
    '491/reader-migration.json','441c/library-identities.json')]
external=[dict(path=str(path.resolve().relative_to(Path.cwd())),bytes=path.stat().st_size,
               sha256=hashlib.sha256(path.read_bytes()).hexdigest()) for path in paths]
save('summary.json',dict(
    gate='PASS_MULTI_STEP_WALL_WALK_OFF_REFUND_AND_COLLISION',findings=['BF144-D287','BF144-D289'],
    new_differences=0,total_differences=286,ordinal=49,native=native,target=target,
    restored_parameters_exact=restored,screenshots=0,performance_acceptance='paused',
    decoded=decoded,external_source_files=external))
