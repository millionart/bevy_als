import gzip
import hashlib
import json
import math
import os
import re
import struct
from pathlib import Path

root=Path(__file__).resolve().parent
prior=root.parent/'531'
labels=('near','mid','far','mid-stop-guard','near-stop-guard','mid-restored')
offsets={'near':2.30,'mid':2.48,'far':2.64}

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

assert read(root/'build-offset.report.json')['success']
assert read(root/'wall.native.json.reader.report.json')['success']
assert read(root/'compile-offsets.report.json')['success']
native_path=root/'wall.native.json'
cases={case['name']:case['frames'] for case in read(native_path)['character_animation_probe']['cases']}
observed,disabled=[cases[name] for name in ('mid','mid-unobserved')]

def normalized_state(row):
    return re.sub(r'/Temp/Untitled_\d+\.Untitled:', '/Temp/Untitled.Untitled:',
                  json.dumps(row['source_state_after_tick'],sort_keys=True))

for a,b in zip(observed,disabled,strict=True):
    for key in ('movement_before_tick','movement_after_tick','actor_transform','actor_velocity_cm_s',
                'getter_after_tick','mesh_local_pose'):
        assert a[key]==b[key]
    assert normalized_state(a)==normalized_state(b)
    assert ('landed_events' in a)==(a['frame'] in (49,50,51))
    assert 'landed_events' not in b and 'blocking_hits' not in b

native={}
previous=read(root.parent/'526'/'summary.json')['native']['edge-in'][0]['hits'][0]
origins=read(root/'native-origins.json')
for label in offsets:
    case=cases[label]
    frame=case[49]
    assert frame['movement_before_tick']['movement_mode_name']=='MOVE_Falling'
    first=frame['blocking_hits'][0]
    assert all(first[key]==previous[key] for key in previous if key not in ('floor_at_hit','floor_at_move_stop'))
    assert not first['normals_equal'] and first['edge_within_at_move_stop']
    point=first['move_stop_cm']
    assert origins[label]==[point['x']/100,2+point['z']/100,point['y']/100]
    floor=first['floor_at_move_stop']
    broad,narrow,merged=[floor[key] for key in ('broad','perch','merged')]
    assert broad['blocking'] and broad['walkable'] and not broad['line_trace']
    assert broad['hit']['component']=='AuditWall'
    assert floor['perch_requested'] and floor['perch_radius_cm']==15
    assert floor['perch_valid']==(label=='near')
    assert narrow['blocking']==narrow['walkable']==(label=='near')
    if label=='near':
        assert narrow['hit']['component']=='AuditLandingDeck'
    assert merged['blocking'] and not merged['line_trace']
    assert merged['walkable']==(label=='near') and merged['hit']==broad['hit']
    assert floor['valid_landing_spot']==(label=='near')
    modes=[case[i]['movement_after_tick']['movement_mode_name'] for i in (49,50,51)]
    landings=[[hit['component'] for hit in case[i]['landed_events']] for i in (49,50,51)]
    if label=='near':
        assert modes==['MOVE_Walking']*3 and landings==[['AuditWall'],[],[]]
    else:
        assert modes==['MOVE_Falling','MOVE_Falling','MOVE_Walking'] and landings==[[],[],['AuditWall']]
    native[label]=dict(first_hit=first,modes=modes,landings=landings)

target,traces,floors,parameters={},{},{},{}
for label in labels:
    fixture=label.split('-')[0]
    functional=read(root/f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images']==[]
    assert read(file(label,'process.report'))['success']
    assert math.isclose(load(label,'barrier')['top_m'],2.16,abs_tol=2e-7)
    assert math.isclose(load(label,'deck')['top'],1.973,abs_tol=2e-7)
    assert math.isclose(load(label,'deck')['center'][0],-15+offsets[fixture],abs_tol=1e-6)
    traces[label]=trace=load(label,'initial-observed')
    assert trace['wall']=='AuditWalkOffBarrier' and trace['second_hit'] is None
    before=load(label,'contact-before-49')
    after=load(label,'contact-after-49')
    assert before['mode']=='Air' and not before['has_basis_displacement'] and before['manifolds']==[]
    assert before['requested_mode'] is None and before['generated_contact']
    assert before['consumed_contact']==(label!='mid-stop-guard')
    assert before['position']==trace['single_hit_position']==after['position']
    assert before['planned_velocity']==trace['single_hit_velocity']==after['velocity']
    floors[label]=floor=load(label,'floor-provenance')
    assert floor['phases']['native-stop']['origin']==[f32(x) for x in origins[fixture]]
    phases={}
    for phase,stage in floor['phases'].items():
        baseline=stage['variants']['none']
        first=[entry for entry in baseline['events'] if entry['kind']=='capsule_qualification'][0]
        assert first['valid'] and first['hit']['entity']=='AuditWalkOffBarrier'
        assert stage['variants']['perch']['result']['entity']=='AuditWalkOffBarrier'
        assert 0<stage['origin'][1]-stage['variants']['perch']['result']['center']<.024
        outcome=event(baseline,'perch_outcome')
        valid=fixture=='near' or (fixture=='mid' and phase=='post')
        if valid:
            assert outcome['hit']['entity']=='AuditLandingDeck'
            assert outcome['hit']['valid'] and outcome['hit']['within_distance']
            selected=event(baseline,'selected')
            assert not selected['line'] and selected['hit']['entity']=='AuditWalkOffBarrier'
            assert stage['public']==baseline['result']==stage['variants']['perch']['result']
        else:
            assert outcome['hit'] is None and baseline['result'] is None and stage['public'] is None
        phases[phase]=dict(origin=stage['origin'],public=stage['public'],perch=outcome['hit'],
                           broad_without_perch=stage['variants']['perch']['result'])
    assert phases['post']['origin']==before['position']
    if fixture=='far':
        assert before['floor'] is None
    else:
        assert phases['post']['public']==dict(entity=before['floor']['name'],
                                             center=before['floor']['center'],impact=before['floor']['impact'])
    parameters[label]=read(file(label,'parameters'))
    rows={row['ordinal']:row for row in parameters[label]['after']}
    modes=[rows[i]['gameplay'] for i in (49,50,51)]
    assert modes[0]==after['mode']
    expected=['Air','Air','Grounded'] if label in ('far','mid-stop-guard') else ['Grounded']*3
    assert modes==expected
    for i in (49,50,51):
        assert rows[i]['graph_error'] is None
        assert rows[i]['graph_tag']=='Als.LocomotionMode.'+('InAir' if rows[i]['gameplay']=='Air' else 'Grounded')
    target[label]=dict(before=before,after=after,phases=phases,modes=modes)

for label in ('mid-stop-guard','mid-restored'):
    assert traces[label]==traces['mid'] and floors[label]==floors['mid']
assert target['mid']['before']==(target['mid-stop-guard']['before']|{'consumed_contact':True})
assert target['mid-stop-guard']['after']['transitions']==[]
assert target['mid']['after']['transitions']==[['LocomotionState(Air)','LocomotionState(Grounded)']]
assert target['mid-restored']==target['mid']
assert traces['near-stop-guard']==traces['near'] and floors['near-stop-guard']==floors['near']
assert target['near-stop-guard']==target['near']

restoration={}
for key in parameters['mid']:
    old,new=[{row['ordinal']:row for row in parameters[label][key]} for label in ('mid','mid-restored')]
    assert old.keys()==new.keys()
    restoration[key]=next((i for i in sorted(old) if old[i]!=new[i]),None)
    assert restoration[key] is None or restoration[key]>51

decoded={}
for path in [native_path]+[file(label,name) for label in labels for name in ('parameters','report','floor-provenance')]:
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
    (cmc,ue/cmc,[(5061,5076),(7272,7311),(7460,7527)]),
    (motion,Path(motion),[(3909,3933),(4074,4120)]),
):
    data=path.read_bytes()
    lines=data.decode().splitlines()
    facts.append(dict(key=key,**identity(data),excerpts=[
        dict(start=a,end=b,text='\n'.join(lines[a-1:b])+'\n') for a,b in ranges]))
save('source-facts.json',facts)

inputs={row['path'] for row in read(prior/'summary.json')['external_source_files']}
inputs.update(str(path.relative_to(Path.cwd())) for path in (
    prior/'prepare_reader.py',prior/'summary.json',prior/'perch_query.inc',prior/'observer.py',
    root.parent/'526'/'wall.request.json',root.parent/'526'/'summary.json',
))
save('summary.json',dict(
    gate='PASS_VALID_WIDE_PERCH_FAILURE_AND_LATE_FLOOR_PHASE_CONTROL',
    new_differences=0,total_differences=291,related_difference='BF144-D292',ordinals=[49,50,51],
    native=native,target=target,native_observer_disabled_motion_exact_frames=len(observed),
    restored_first_late_parameter_difference=restoration,decoded=decoded,screenshots=0,
    performance_acceptance='paused',
    external_source_files=[dict(path=name,**identity(Path(name).read_bytes())) for name in sorted(inputs)]))
print('Verified broad support invalidation and the late floor qualification control',flush=True)
