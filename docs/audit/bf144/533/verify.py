import gzip
import hashlib
import json
import math
import os
import re
import struct
from pathlib import Path

root=Path(__file__).resolve().parent
prior=root.parent/'532'
labels=('width-100','width-108','width-125')
frames=(40,41,42)

def raw(path):
    return path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix('.json.gz').read_bytes())

def read(path):
    return json.loads(raw(path))

def identity(data):
    return dict(bytes=len(data),sha256=hashlib.sha256(data).hexdigest())

def save(name,result):
    (root/name).write_text(json.dumps(result,indent=2)+'\n')

def file(label,name):
    return root/f'runtime-{label}'/f'{name}.json'

def load(label,name):
    return read(file(label,name))

def f32(value):
    return struct.unpack('<f',struct.pack('<f',value))[0]

def mapped_position(value):
    return [value['x']/100,2+value['z']/100,value['y']/100]

def mapped_velocity(value):
    return [value['x']/100,value['z']/100,value['y']/100]

def check_cancelled_floor(floor):
    assert floor['walking_range'] and floor['perch_requested'] and floor['perch_radius_cm']==15
    assert not floor['perch_valid'] and not floor['perch']['blocking']
    assert floor['broad']['walkable'] and floor['broad']['hit']['component']!='None'
    assert floor['merged']['blocking'] and not floor['merged']['walkable']
    assert floor['query_movement_mode']=='MOVE_Falling'
    broad,merged=floor['broad']['hit'],floor['merged']['hit']
    for key in ('component','impact_point_cm','normal','impact_normal'):
        assert merged[key]==broad[key]
    assert floor['merged']['sweep_dist_cm']==floor['broad']['sweep_dist_cm']
    assert max(abs(merged['location_cm'][key]-broad['location_cm'][key]) for key in ('x','y','z'))<2e-6

assert read(root/'build-walking.report.json')['success']
assert read(root/'wall.native.json.reader.report.json')['success']
assert read(root/'compile-walking.report.json')['success']
native_path=root/'wall.native.json'
cases={case['name']:case['frames'] for case in read(native_path)['character_animation_probe']['cases']}
obs,disabled=[cases[name] for name in ('width-100','width-100-unobserved')]

def normalized_state(row):
    return re.sub(r'/Temp/Untitled_\d+\.Untitled:', '/Temp/Untitled.Untitled:',
                  json.dumps(row['source_state_after_tick'],sort_keys=True))

for a,b in zip(obs,disabled,strict=True):
    for key in ('movement_before_tick','movement_after_tick','actor_transform','actor_velocity_cm_s',
                'getter_after_tick','mesh_local_pose'):
        assert a[key]==b[key]
    assert normalized_state(a)==normalized_state(b)
    assert ('mode_events' in a)==(a['frame'] in frames)
    assert 'mode_events' not in b

native={}
for label in labels:
    case=cases[label]
    frame=case[40]
    before=frame['movement_before_tick']
    assert before['movement_mode_name']=='MOVE_Walking' and before['walkable_floor']
    assert mapped_velocity(before['velocity_cm_s'])==[6,0,0]
    assert frame['world_delta_seconds']==f32(.06)
    rows=[dict(mode=case[i]['movement_after_tick']['movement_mode_name'],
               position_m=mapped_position(case[i]['actor_transform']['translation']),
               velocity_mps=mapped_velocity(case[i]['actor_velocity_cm_s']),
               events=case[i]['mode_events'],landings=case[i]['landed_events']) for i in frames]
    if label=='width-125':
        assert rows[0]['mode']=='MOVE_Walking' and rows[0]['events']==[]
        switch,=rows[1]['events']
    else:
        assert rows[0]['mode']=='MOVE_Falling'
        switch,=rows[0]['events']
    assert switch['previous']=='MOVE_Walking' and switch['current']=='MOVE_Falling'
    check_cancelled_floor(switch['walking_floor'])
    assert switch['velocity_cm_s']['z']==0
    assert not any(row['landings'] for row in rows)
    native[label]=dict(rows=rows,switch=switch)

first=native['width-100']
second=native['width-108']
wide=native['width-125']
step=f32(.06)*.5
first_switch=mapped_position(first['switch']['location_cm'])
assert abs(first_switch[0]-(-14.01+first['switch']['velocity_cm_s']['x']/100*step))<1e-8
assert abs(first['rows'][0]['velocity_mps'][1]+9.81*step)<1e-7
assert second['rows'][0]['position_m']==mapped_position(second['switch']['location_cm'])
assert second['rows'][0]['velocity_mps'][1]==0
assert abs(second['rows'][0]['position_m'][0]-(first_switch[0]+second['rows'][0]['velocity_mps'][0]*step))<1e-8
assert wide['rows'][0]['position_m'][0]==second['rows'][0]['position_m'][0]
assert wide['rows'][1]['velocity_mps'][1]==0 and wide['rows'][1]['position_m'][1]==wide['rows'][0]['position_m'][1]

def assert_cancelled_target(query):
    assert query['walking_range']
    baseline=query['variants']['none']
    assert baseline['result'] is None
    broad=next(row for row in baseline['events'] if row['kind']=='capsule_qualification')
    assert broad['valid'] and broad['hit']['entity']=='AuditWalkOffPlatform'
    outcome,=[row for row in baseline['events'] if row['kind']=='perch_outcome']
    assert outcome['hit'] is None
    assert query['variants']['perch']['result']['entity']=='AuditWalkOffPlatform'

target={}
for label in labels:
    functional=read(root/f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images']==[]
    assert load(label,'process.report')['success']
    step_data=load(label,'walking-step')
    assert step_data['position']==[f32(-14.01),f32(2.9215),-15]
    assert step_data['velocity']==step_data['base_velocity']==[6,0,0]
    assert step_data['step_s']==step and step_data['delta_s']==f32(.06)
    assert step_data['start_floor']['variants']['none']['result']['entity']=='AuditWalkOffPlatform'
    assert abs(step_data['first_step_velocity'][0]-first['switch']['velocity_cm_s']['x']/100)<1e-6
    assert math.dist(step_data['first_floor']['origin'],first_switch)<2e-6
    transaction=load(label,'walking-transaction')
    post=load(label,'contact-after-40')
    assert transaction['mode']==post['mode'] and transaction['position']==post['position']
    assert transaction['collision_velocity']==post['velocity']
    assert math.dist(post['position'],native[label]['rows'][0]['position_m'])<3e-6
    assert math.dist(post['velocity'],native[label]['rows'][0]['velocity_mps'])<2e-5
    if label=='width-100':
        assert_cancelled_target(step_data['first_floor'])
    else:
        assert step_data['first_floor']['variants']['none']['result']['entity']=='AuditWalkOffPlatform'
    if label=='width-108':
        assert_cancelled_target(transaction['end_floor'])
    if label=='width-125':
        assert transaction['end_floor']['variants']['none']['result']['entity']=='AuditWalkOffPlatform'
    data=load(label,'parameters')
    rows={row['ordinal']:row for row in data['after']}
    for i,native_row in zip(frames,native[label]['rows'],strict=True):
        target_mode='Grounded' if native_row['mode']=='MOVE_Walking' else 'Air'
        assert rows[i]['gameplay']==target_mode and rows[i]['graph_error'] is None
        assert rows[i]['graph_tag']=='Als.LocomotionMode.'+('InAir' if target_mode=='Air' else 'Grounded')
    target[label]=dict(first_step=step_data,transaction=transaction,post=post,rows=[
        dict(mode=rows[i]['gameplay'],position=rows[i]['position'],velocity=rows[i]['velocity']) for i in frames])

source_row=cases['width-125'][41]
cached=source_row['movement_before_tick']['max_acceleration_cm_s2']/100
assert cached==source_row['movement_after_tick']['acceleration_cm_s2']['x']/100
dt=source_row['world_delta_seconds']
source_initial=wide['rows'][0]['velocity_mps'][0]
source_final=wide['rows'][1]['velocity_mps'][0]
assert abs(source_final-(source_initial+cached*dt))<1e-9
dynamics=next(row for row in load('width-125','parameters')['dynamics'] if row['ordinal']==41)
fresh=dynamics['dynamics'][0]
target_initial=dynamics['world_velocity_at_dynamics'][0]
target_planned=dynamics['planned_velocity'][0]
assert abs(target_planned-(target_initial+fresh*dt))<1e-6
target_final=target['width-125']['rows'][1]['velocity'][0]
assert target['width-125']['rows'][1]['velocity'][1]==0
assert .012 < target_final-source_final < .014
assert abs((target_final-source_final)-(fresh-cached)*dt)<3e-5

decoded={}
for path in [native_path]+[file(label,name) for label in labels for name in ('parameters','report','walking-step','walking-transaction')]:
    data=raw(path)
    archive=path.with_suffix('.json.gz')
    archive.write_bytes(gzip.compress(data,compresslevel=6,mtime=0))
    assert gzip.decompress(archive.read_bytes())==data
    decoded[str(path.relative_to(root))]=identity(data)
    if path.exists():
        path.unlink()
    print('Verified gzip '+str(path.relative_to(root)),flush=True)

ue=Path(os.environ['ALS_UE_SOURCE_ROOT'])
cmc='Engine/Source/Runtime/Engine/Private/Components/CharacterMovementComponent.cpp'
motion='src/als/character/movement/motion.rs'
dynamics_file='src/als/character/movement/dynamics.rs'
facts=[]
for key,path,ranges in (
    (cmc,ue/cmc,[(5427,5457),(5772,5861),(7272,7311)]),
    (motion,Path(motion),[(3599,3643),(3767,3802)]),
    (dynamics_file,Path(dynamics_file),[(282,327)]),
):
    data=path.read_bytes()
    lines=data.decode().splitlines()
    facts.append(dict(key=key,**identity(data),excerpts=[
        dict(start=a,end=b,text='\n'.join(lines[a-1:b])+'\n') for a,b in ranges]))
save('source-facts.json',facts)

inputs={row['path'] for row in read(prior/'summary.json')['external_source_files']}
inputs.update(str(path.relative_to(Path.cwd())) for path in (
    prior/'prepare_reader.py',prior/'observer.py',prior/'summary.json',
    root.parent/'520'/'wall.request.json',root.parent/'523'/'native_initial.inc',
))
save('summary.json',dict(
    gate='PASS_WALKING_PERCH_CANCELLATION_AND_REMAINING_TIME',new_differences=0,total_differences=291,
    ordinals=list(frames),native=native,target=target,native_observer_disabled_motion_exact_frames=len(obs),
    related_difference='BF144-D07',
    following_frame=dict(source_cached_acceleration_mps2=cached,target_fresh_acceleration_mps2=fresh,
                         source_velocity_mps=source_final,target_velocity_mps=target_final),
    decoded=decoded,screenshots=0,performance_acceptance='paused',
    external_source_files=[dict(path=name,**identity(Path(name).read_bytes())) for name in sorted(inputs)]))
print('Verified actual Walking perch cancellation, remaining time and next-frame acceleration',flush=True)
