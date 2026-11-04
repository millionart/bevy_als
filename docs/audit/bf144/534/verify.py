import gzip
import hashlib
import json
import math
import os
import re
import struct
from pathlib import Path

root=Path(__file__).resolve().parent
prior=root.parent/'533'
labels=('depth-473','depth-475','depth-47403','depth-47403-repeat')

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

def normalized_state(row):
    return re.sub(r'/Temp/Untitled_\d+\.Untitled:', '/Temp/Untitled.Untitled:',
                  json.dumps(row['source_state_after_tick'],sort_keys=True))

def check_observation(observed,disabled):
    for a,b in zip(observed,disabled,strict=True):
        for key in ('movement_before_tick','movement_after_tick','actor_transform','actor_velocity_cm_s',
                    'getter_after_tick','mesh_local_pose'):
            assert a[key]==b[key]
        assert normalized_state(a)==normalized_state(b)
        assert 'walking_first_floor' not in b and 'mode_events' not in b

assert read(root/'build-distance.report.json')['success']
assert read(root/'compile-distance.report.json')['success']
assert read(root/'wall.package.json')==read(root/'boundary.package.json')
for name in ('wall','boundary','geometry'):
    assert read(root/f'{name}.native.json.reader.report.json')['success']
cases={}
for name in ('wall','boundary'):
    cases.update({case['name']:case['frames'] for case in read(root/f'{name}.native.json')['character_animation_probe']['cases']})
for label in ('depth-475','depth-47403'):
    check_observation(cases[label],cases[label+'-unobserved'])

point=read(root/'walking-first-cm.json')
native={}
for label in labels[:3]:
    row=cases[label][40]
    first=row['walking_first_floor']
    assert row['movement_before_tick']['movement_mode_name']=='MOVE_Walking'
    assert first['walking_range'] and first['broad']['walkable'] and first['perch_requested']
    assert first['perch_radius_cm']==15 and 47.399<first['max_perch_cm']<47.402
    narrow=first['narrow_before_limit']
    assert narrow['blocking'] and narrow['walkable'] and not narrow['line_trace']
    assert narrow['hit']['component']=='AuditLandingDeck'
    assert first['narrow_total_cm']==f32(first['above_base_cm']+narrow['sweep_dist_cm'])
    accepted=label=='depth-473'
    assert first['perch_valid']==first['perch']['walkable']==accepted
    assert (first['narrow_total_cm']<=first['max_perch_cm'])==accepted
    mode=row['movement_after_tick']['movement_mode_name']
    assert mode==('MOVE_Walking' if accepted else 'MOVE_Falling')
    if accepted:
        assert row['mode_events']==[]
    else:
        switch,=row['mode_events']
        assert switch['location_cm']==point and switch['velocity_cm_s']['z']==0
        assert switch['previous']=='MOVE_Walking' and switch['current']=='MOVE_Falling'
    native[label]=dict(first_floor=first,mode=mode,position_cm=row['actor_transform']['translation'],
                       velocity_cm_s=row['actor_velocity_cm_s'],events=row['mode_events'])

depths={'depth-473':1.527,'depth-475':1.525,'depth-47403':1.52597,'depth-47403-repeat':1.52597}
target={}
for label in labels:
    functional=read(root/f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images']==[]
    assert load(label,'process.report')['success']
    deck=load(label,'deck')
    assert math.isclose(deck['top'],depths[label],abs_tol=2e-7)
    assert math.isclose(deck['center'][0],-12.95,abs_tol=2e-6)
    step=load(label,'walking-step')
    phase=step['first_floor']
    assert phase['origin']==[f32(point['x']/100),f32(2+point['z']/100),f32(point['y']/100)]
    assert step['velocity']==step['base_velocity']==[6,0,0]
    assert step['step_s']==f32(.03) and step['delta_s']==f32(.06)
    events=phase['variants']['none']['events']
    outcome,=[row for row in events if row['kind']=='perch_outcome']
    qualified,=[row for row in events if row['kind']=='capsule_qualification' and row['hit']['entity']=='AuditLandingDeck']
    cast,=[row for row in events if row['kind']=='capsule' and row['radius_m']<.16]
    assert qualified['valid'] and cast['hit']['entity']=='AuditLandingDeck'
    assert .149 < cast['radius_m'] < .151 and .059 < qualified['shrink_m'] < .061
    assert .47399<outcome['max_distance_m']<.47401
    total=f32(outcome['above_base_m']+outcome['hit']['distance_m'])
    accepted=label!='depth-475'
    assert outcome['hit']['within_distance']==(total<=outcome['max_distance_m'])==accepted
    assert (phase['variants']['none']['result'] is not None)==accepted
    assert phase['variants']['perch']['result']['entity']=='AuditWalkOffPlatform'
    transaction=load(label,'walking-transaction')
    after=load(label,'contact-after-40')
    assert transaction['mode']==after['mode']==('Grounded' if accepted else 'Air')
    assert transaction['position']==after['position']
    assert transaction['collision_velocity']==after['velocity']
    if accepted:
        assert after['velocity'][1]==0 and abs(after['position'][1]-(deck['top']+.9215))<8e-5
    else:
        assert abs(after['velocity'][1]+.2943)<1e-6
    target[label]=dict(total_m=total,outcome=outcome,narrow_cast=cast,shrink_m=qualified['shrink_m'],
                       first=step,transaction=transaction,after=after)

assert target['depth-47403']==target['depth-47403-repeat']
assert load('depth-47403','contact-after-41')==load('depth-47403-repeat','contact-after-41')
assert .46 < (2+native['depth-47403']['position_cm']['z']/100-target['depth-47403']['after']['position'][1]) < .48

geom_request=read(root/'geometry.request.json')['samples']
geom_reply=read(root/'geometry.native.json')['geometry_query_probe']['samples']
assert [row['input'] for row in geom_reply]==geom_request
geom=geom_reply[0]['capsule_sweep']
assert geom['hit'] and list(geom['normal'].values())==[0,0,1]
witness=target['depth-47403']
chaos_m=geom['distance_cm']/100
parry_m=witness['narrow_cast']['hit']['distance_m']
gap=chaos_m-parry_m
assert 6e-5<gap<8e-5
hit=witness['narrow_cast']['hit']
assert abs((hit['point2'][1]-hit['point'][1])-gap)<1e-6
corrected_total=f32(witness['outcome']['above_base_m']+f32(f32(chaos_m)-witness['shrink_m']))
assert witness['total_m']<witness['outcome']['max_distance_m']<corrected_total
assert native['depth-47403']['first_floor']['narrow_total_cm']>native['depth-47403']['first_floor']['max_perch_cm']

decoded={}
for path in [root/'wall.native.json',root/'boundary.native.json',root/'geometry.native.json'] + [
    file(label,name) for label in labels for name in ('parameters','report','walking-step','walking-transaction')
]:
    data=raw(path)
    archive=path.with_suffix('.json.gz')
    archive.write_bytes(gzip.compress(data,compresslevel=6,mtime=0))
    assert gzip.decompress(archive.read_bytes())==data
    decoded[str(path.relative_to(root))]=identity(data)
    if path.exists():
        path.unlink()
    print('Verified gzip '+str(path.relative_to(root)),flush=True)

ue=Path(os.environ['ALS_UE_SOURCE_ROOT'])
plugin=Path(json.loads(Path('tools/env.local.json').read_bytes())['ue_project']).parent/'Plugins/ALS-Refactored/Source/ALS'
cmc='Engine/Source/Runtime/Engine/Private/Components/CharacterMovementComponent.cpp'
motion='src/als/character/movement/motion.rs'
facts=[]
for key,path,ranges in (
    (cmc,ue/cmc,[(7272,7311),(7489,7526)]),
    ('source:Private/AlsCharacterMovementComponent.cpp',plugin/'Private/AlsCharacterMovementComponent.cpp',[(155,162),(512,531)]),
    (motion,Path(motion),[(3909,3933),(3599,3618)]),
):
    data=path.read_bytes()
    lines=data.decode().splitlines()
    facts.append(dict(key=key,**identity(data),excerpts=[
        dict(start=a,end=b,text='\n'.join(lines[a-1:b])+'\n') for a,b in ranges]))
save('source-facts.json',facts)

inputs={row['path'] for row in read(prior/'summary.json')['external_source_files']}
inputs.update(str(path.relative_to(Path.cwd())) for path in (
    prior/'prepare_reader.py',prior/'observer.py',prior/'walking_probe.rs',prior/'mode_observer.inc',
    prior/'wall.request.json',prior/'summary.json',
))
save('summary.json',dict(
    gate='PASS_WALKING_PERCH_DISTANCE_BOUNDARY_AND_COMMON_SWEEP',
    new_differences=0,total_differences=291,related_difference='BF144-D125',ordinal=40,
    native=native,target=target,chaos_narrow_distance_m=chaos_m,parry_narrow_distance_m=parry_m,
    common_input_raw_distance_delta_m=gap,target_total_with_native_distance_m=corrected_total,
    decoded=decoded,screenshots=0,performance_acceptance='paused',
    external_source_files=[dict(path=name,**identity(Path(name).read_bytes())) for name in sorted(inputs)]))
print('Verified the shared threshold and its narrow-capsule numeric consumer',flush=True)
