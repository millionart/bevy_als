import gzip
import hashlib
import json
import math
import os
import re
import struct
from pathlib import Path

root=Path(__file__).resolve().parent
prior=root.parent/'534'
labels=('clamp-off','clamp-on')
average=.0215

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

def event(variant,kind):
    row,=[value for value in variant['events'] if value['kind']==kind]
    return row

target_average=f32(f32(f32(.019)+f32(.024))*.5)
default_arc=.30-math.sqrt(.30**2-.15**2)
assert default_arc>average
assert read(root/'build-clamp.report.json')['success']
assert read(root/'wall.native.json.reader.report.json')['success']
assert read(root/'compile-clamp.report.json')['success']
assert read(root/'compile-midscale.report.json')['success']
assert not read(root/'midscale-functional.json')['pass']
midscale=load('clamp-off-midscale','report')
assert midscale['faults'] and midscale['temporal_anomalies']
assert all(row['kind']=='segment_length_spike' and abs(row['current']/row['previous']-.3)<.01
           for row in midscale['temporal_anomalies'])

native_path=root/'wall.native.json'
cases={case['name']:case['frames'] for case in read(native_path)['character_animation_probe']['cases']}

def normalized_state(row):
    return re.sub(r'/Temp/Untitled_\d+\.Untitled:', '/Temp/Untitled.Untitled:',
                  json.dumps(row['source_state_after_tick'],sort_keys=True))

observed,disabled=[cases[name] for name in ('clamp-on','clamp-on-unobserved')]
for a,b in zip(observed,disabled,strict=True):
    for key in ('movement_before_tick','movement_after_tick','actor_transform','actor_velocity_cm_s',
                'getter_after_tick','mesh_local_pose'):
        assert a[key]==b[key]
    assert normalized_state(a)==normalized_state(b)
    assert 'walking_first_floor' not in b

native={}
for label in labels:
    row=cases[label][40]
    floor=row['walking_first_floor']
    broad,narrow,merged=[floor[key] for key in ('broad','narrow_before_limit','merged')]
    assert row['movement_before_tick']['movement_mode_name']=='MOVE_Walking'
    assert row['movement_after_tick']['movement_mode_name']=='MOVE_Walking'
    assert row['mode_events']==[] and row['landed_events']==[]
    assert abs(floor['capsule_radius_cm']-9)<1e-5 and abs(floor['capsule_half_height_cm']-27)<1e-5
    assert floor['walking_range'] and floor['perch_requested'] and floor['perch_valid']
    assert broad['walkable'] and narrow['walkable'] and merged['walkable']
    assert all(not item['line_trace'] and not item['hit']['penetrating'] for item in (broad,narrow,merged))
    assert broad['hit']['component']!=narrow['hit']['component']=='AuditLandingDeck'
    assert merged['hit']==broad['hit'] and broad['sweep_dist_cm']>0
    assert floor['narrow_total_cm']<floor['max_perch_cm']
    avg=f32(f32(f32(1.9)+f32(2.4))*.5)
    comparison=f32(f32(avg-broad['sweep_dist_cm'])+narrow['sweep_dist_cm'])
    enabled=label=='clamp-on'
    assert (comparison>=floor['max_perch_cm'])==enabled
    assert merged['sweep_dist_cm']==(avg if enabled else broad['sweep_dist_cm'])
    expected=27.1 if enabled else 27.1+avg-broad['sweep_dist_cm']
    position=row['actor_transform']['translation']
    assert position['x']==-1399 and position['y']==-1500 and abs(position['z']-expected)<2e-6
    native[label]=dict(floor=floor,comparison_cm=comparison,position_cm=position,clamped=enabled)

target={}
for label in labels:
    functional=read(root/f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images']==[]
    assert load(label,'process.report')['success']
    scaled=load(label,'scaled-start')
    assert scaled['scale']==[f32(.3)]*3
    assert math.dist(scaled['half_extents'],[.09,.27,.09])<1e-7
    assert scaled['position']==[f32(-13.99),f32(2.271),-15]
    assert scaled['position'][1]-scaled['half_extents'][1]>2
    step=load(label,'walking-step')
    assert step['position']==scaled['position'] and step['velocity']==step['base_velocity']==[0,0,0]
    assert step['first_step_velocity']==[0,0,0]
    phase=step['start_floor']
    normal,disabled=[phase['variants'][key] for key in ('none','clamp')]
    perch=event(normal,'perch_outcome')
    assert perch['hit']['valid'] and perch['hit']['within_distance']
    broad=next(value for value in normal['events'] if value['kind']=='capsule_qualification')
    assert broad['hit']['entity']=='AuditWalkOffPlatform' and broad['floor_distance_m']>0
    selected=event(normal,'selected')
    unclamped=event(disabled,'selected')
    assert selected['hit']==unclamped['hit']==broad['hit'] and not selected['line']
    assert unclamped['floor_distance_m']==broad['floor_distance_m']
    comparison=f32(f32(target_average-broad['floor_distance_m'])+perch['hit']['distance_m'])
    enabled=label=='clamp-on'
    assert (comparison>=perch['max_distance_m'])==enabled
    assert selected['floor_distance_m']==(target_average if enabled else broad['floor_distance_m'])
    after=load(label,'contact-after-40')
    assert after['mode']=='Grounded' and after['velocity']==[0,0,0] and after['transitions']==[]
    native_position=native[label]['position_cm']
    assert math.dist(after['position'],[native_position['x']/100,2+native_position['z']/100,native_position['y']/100])<6e-5
    if not enabled:
        height_delta=after['position'][1]-(2+native_position['z']/100)
        floor_delta=native[label]['floor']['broad']['sweep_dist_cm']/100-broad['floor_distance_m']
        assert 3e-5<height_delta<4e-5 and abs(height_delta-floor_delta)<1e-6
    target[label]=dict(scaled=scaled,broad_distance_m=broad['floor_distance_m'],
                       comparison_m=comparison,perch=perch,selected=selected,unclamped=unclamped,
                       transaction=load(label,'walking-transaction'),after=after)

decoded={}
paths=[native_path,file('clamp-off-midscale','report')]+[
    file(label,name) for label in labels for name in ('parameters','report','walking-step','walking-transaction')
]
for path in paths:
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
facts=[]
for key,path,ranges in (
    (cmc,ue/cmc,[(7272,7311),(7493,7526),(6300,6339)]),
    (motion,Path(motion),[(3909,3933),(3599,3618),(4010,4074)]),
):
    data=path.read_bytes()
    lines=data.decode().splitlines()
    facts.append(dict(key=key,**identity(data),excerpts=[
        dict(start=a,end=b,text='\n'.join(lines[a-1:b])+'\n') for a,b in ranges]))
save('source-facts.json',facts)

inputs={row['path'] for row in read(prior/'summary.json')['external_source_files']}
inputs.update(str(path.relative_to(Path.cwd())) for path in (
    prior/'prepare_reader.py',prior/'observer.py',prior/'summary.json',prior/'perch_distance.inc',
    root.parent/'533'/'walking_probe.rs',root.parent/'533'/'mode_observer.inc',
))
save('summary.json',dict(
    gate='PASS_SCALED_PERCH_AVERAGE_FLOOR_CLAMP_AND_ACTUAL_WALKING',
    new_differences=0,total_differences=291,ordinal=40,native=native,target=target,
    default_positive_capsule_edge_arc_lower_bound_m=default_arc,average_floor_gap_m=average,
    native_observer_disabled_motion_exact_frames=len(observed),
    decoded=decoded,screenshots=0,performance_acceptance='paused',
    external_source_files=[dict(path=name,**identity(Path(name).read_bytes())) for name in sorted(inputs)]))
print('Verified the real scaled perch clamp and its Walking height consumer',flush=True)
