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
source_cases = {int(c['name'].split('-')[-1]):c for c in native_report['character_animation_probe']['cases']}
native = {}
for width,case in source_cases.items():
    rows=case['frames']
    a,b=rows[48:50]
    assert a['world_delta_seconds']==f32(.06)
    assert a['movement_before_tick']['movement_mode_name']=='MOVE_Walking'
    mode=a['movement_after_tick']['movement_mode_name']
    vertical=a['actor_velocity_cm_s']['z']/100
    if width<=80:
        assert mode=='MOVE_Falling' and abs(vertical+.2943)<1e-7
    elif width in (85,90):
        assert mode=='MOVE_Falling' and vertical==0
        assert abs(b['actor_velocity_cm_s']['z']/100+.327)<1e-7
    else:
        assert width==100 and mode=='MOVE_Walking' and vertical==0
        assert b['movement_after_tick']['movement_mode_name']=='MOVE_Falling'
        assert b['actor_velocity_cm_s']['z']==0
    native[width]=dict(mode_48=mode,vertical_48=vertical,vertical_49=b['actor_velocity_cm_s']['z']/100)
print('Verified the native first-step, final-step and grounded cases',flush=True)

labels=('ledge-75','ledge-80','ledge-85','ledge-90','ledge-100','start-aligned','step-aligned','restored')
runs={}
stages={}
for label in labels:
    for name in (f'{label}-functional-process.json',f'runtime-{label}/process.report.json'):
        assert load(name)['success']
    gate=load(f'{label}-functional.json')
    assert gate['pass'] and gate['images']==[] and gate['validation_scope']=='functional'
    path=f'runtime-{label}'
    fixture=load(f'{path}/fixture.json')
    profile=load(f'{path}/profile.json')
    width=int(label.split('-')[-1]) if label.startswith('ledge-') else 80
    assert fixture['half_width_cm']==width
    assert profile['effective']['responsive']==profile['original']['normal']
    before,after=load(f'{path}/stage-before.json'),load(f'{path}/stage-after.json')
    assert before['mode']=='Grounded' and before['substeps']==after['substeps']==2
    assert before['delta']==after['delta']==f32(.06)
    assert before['support']==[0,0,0]
    runs[label]={key:table(rows) for key,rows in load(f'{path}/parameters.json').items()}
    row=runs[label]['after'][48]
    assert row['gameplay']==after['mode'] and row['velocity']==after['collision_velocity']
    assert row['graph_error'] is None
    stages[label]=dict(origin=fixture['origin'],before=before,after=after)
assert load('compile-observed.report.json')['success']

target = {}
for label,stage in stages.items():
    a,b=(runs[label]['after'][i] for i in (48,49))
    air_time=-a['velocity'][1]/f32(9.81)
    if label in ('ledge-75','start-aligned','step-aligned'):
        assert a['gameplay']=='Air' and abs(air_time-.03)<1e-7
    elif label=='ledge-100':
        assert a['gameplay']=='Grounded' and air_time==0
    else:
        assert a['gameplay']=='Air' and air_time==0
    if label in ('ledge-85','ledge-90','ledge-80','restored'):
        assert b['gameplay']=='Air'
        assert abs(b['velocity'][1]+.327)<1e-7
    target[label]=dict(mode_48=a['gameplay'],vertical_48=a['velocity'][1],
                       vertical_49=b['velocity'][1],air_time_48=air_time)
assert target['ledge-100']['vertical_49']>0.3

def switch_x(stage):
    out=stage['after']
    air_time=-out['collision_velocity'][1]/f32(9.81)
    air_move=(out['collision_velocity'][0]+out['walking_last_input'][0])*.5*air_time
    return out['position'][0]-stage['origin'][0]-air_move

ncase=source_cases[80]
n=ncase['frames'][48]
before,after=n['movement_before_tick'],n['movement_after_tick']
origin=ncase['frames'][0]['actor_transform']['translation']['x']/100
native_start=before['last_update_location_cm']['x']/100-origin
native_air=-after['velocity_cm_s']['z']/(-n['gravity_cm_s2'])
native_walk=before['velocity_cm_s']['x']/100+before['max_acceleration_cm_s2']/100*(n['world_delta_seconds']-native_air)
native_switch=after['last_update_location_cm']['x']/100-origin-(native_walk+after['velocity_cm_s']['x']/100)*.5*native_air
record=load('alignment-input.json')
assert record['native_start']==native_start and record['native_first_step_end']==native_switch
assert abs(record['target_first_step_end']-switch_x(stages['ledge-75']))<1e-7
base=stages['ledge-80']
for label,key in (('start-aligned','start_shift'),('step-aligned','first_step_shift')):
    state=stages[label]
    assert state['before']==base['before']
    for group,rows in runs['ledge-80'].items():
        assert [row for i,row in rows.items() if i<48]==[
            row for i,row in runs[label][group].items() if i<48]
    control=load(f'runtime-{label}/alignment.json')
    assert control['frame']==48 and control['label']==label
    assert control['before']==base['before']['position']
    assert control['requested']==f32(record[key])
    assert control['after']==[f32(control['before'][0]+control['requested']),*control['before'][1:]]
    actual_shift=control['after'][0]-control['before'][0]
    assert abs((switch_x(state)-switch_x(stages['ledge-75']))-actual_shift)<2e-6
    target[label]['switch_x']=switch_x(state)
assert abs(target['step-aligned']['switch_x']-native_switch)<2e-6
assert stages['restored']==base
assert all(rows[48]==runs['restored'][key][48] for key,rows in runs['ledge-80'].items())
restored_diff={key:next((i for i,row in rows.items() if row!=runs['restored'][key][i]),None)
               for key,rows in runs['ledge-80'].items()}
assert all(i is None or i>48 for i in restored_diff.values())
save('boundary.json',dict(native=native,target=target,alignment=record,restored_first_difference=restored_diff))
print('Verified live edge classification and the horizontal alignment counterfactual',flush=True)

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
facts=[]
for key,path,ranges in (
    ('source:Private/AlsCharacterMovementComponent.cpp',als/'Private/AlsCharacterMovementComponent.cpp',[(372,411),(510,558)]),
    ('Engine/Source/Runtime/Engine/Private/Components/CharacterMovementComponent.cpp',
     ue/'Engine/Source/Runtime/Engine/Private/Components/CharacterMovementComponent.cpp',[(5427,5457),(8162,8186)]),
    ('src/als/character/movement/dynamics.rs',Path('src/als/character/movement/dynamics.rs'),[(303,327)]),
    ('src/als/character/movement/motion.rs',Path('src/als/character/movement/motion.rs'),[(3597,3644),(3770,3807)]),
):
    data=path.read_bytes()
    lines=data.decode().splitlines()
    facts.append(dict(key=key,sha256=hashlib.sha256(data).hexdigest(),
        excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b])+'\n') for a,b in ranges]))
save('source-facts.json',facts)
save('final-source.json',dict(sha256=hashlib.sha256((root/'alignment.rs').read_bytes()).hexdigest()))
binaries=[Path(f'tools/tmp/bf144/{number}/playground_observed') for number in (516,517)]
if all(path.exists() for path in binaries):
    save('binary-identities.json',{str(path):hashlib.sha256(path.read_bytes()).hexdigest() for path in binaries})
paths=[parent/name for name in (
    '484/playground_observed.rs','484/foot_observer.rs','505/observer.py','505/velocity_observer.rs',
    '509/observer.py','509/lean_observer.rs','510/observer.py','510/acceleration_observer.rs',
    '512/observer.py','512/sprint_observer.rs','513/observer.py','513/profile.rs',
    '516/observer.py','516/ledge_observer.rs','516/late.native.json.gz',
    '516/runtime-late-baseline/parameters.json.gz','491/reader-migration.json','441c/library-identities.json')]
external=[dict(path=str(path.resolve().relative_to(Path.cwd())),bytes=path.stat().st_size,
               sha256=hashlib.sha256(path.read_bytes()).hexdigest()) for path in paths]
save('summary.json',dict(
    gate='PASS_WALK_OFF_ZERO_REMAINDER_AND_POSITION_CONTROL',
    zero_remainder_widths_cm=[85,90],early_transition_width_cm=75,
    prestate_control_width_cm=80,prestate_shift=record['start_shift'],
    prestate_control_air_time=target['start-aligned']['air_time_48'],
    next_observation=dict(width_cm=100,frame=49,native_vertical=native[100]['vertical_49'],
                          target_vertical=target['ledge-100']['vertical_49']),
    restored_witness_exact=True,restored_first_difference=restored_diff,
    new_differences=0,total_differences=284,screenshots=0,performance_acceptance='paused',
    decoded=decoded,external_source_files=external))
