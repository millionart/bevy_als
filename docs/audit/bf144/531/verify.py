import gzip
import hashlib
import json
import math
import os
import re
import struct
from pathlib import Path

root=Path(__file__).resolve().parent
prior=root.parent/'530'
labels=('perch','no-blocker','perch-line-only','perch-merge-guard','perch-restored','no-blocker-merge-guard')

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

assert read(root/'build-perch.report.json')['success']
assert read(root/'wall.native.json.reader.report.json')['success']
assert read(root/'compile-perch.report.json')['success']
native_path=root/'wall.native.json'
cases={case['name']:case['frames'] for case in read(native_path)['character_animation_probe']['cases']}
observed,disabled=[cases[name] for name in ('perch','perch-unobserved')]

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
for label in ('perch','no-blocker'):
    case=cases[label]
    frame=case[49]
    assert frame['movement_before_tick']['movement_mode_name']=='MOVE_Falling'
    first=frame['blocking_hits'][0]
    assert first['component']=='AuditWall' and not first['penetrating'] and not first['walkable_hit']
    assert not first['normals_equal'] and first['edge_within_at_move_stop']
    floor=first['floor_at_move_stop']
    assert floor['perch_requested'] and floor['perch_valid'] and floor['perch_radius_cm']==15
    assert floor['walkable'] and floor['valid_landing_spot']
    native[label]=dict(first_hit=first, floor_at_stop=floor,
                       modes=[case[i]['movement_after_tick']['movement_mode_name'] for i in (49,50,51)],
                       landed_events=[case[i]['landed_events'] for i in (49,50,51)])

first,control=[native[label]['first_hit'] for label in ('perch','no-blocker')]
previous=read(prior/'summary.json')['native']['line']['first_hit']
assert all(first[key]==control[key]==previous[key] for key in first if key not in ('floor_at_hit','floor_at_move_stop'))
floor=native['perch']['floor_at_stop']
broad,perch,merged=[floor[key] for key in ('broad','perch','merged')]
assert broad['blocking'] and not broad['walkable'] and not broad['line_trace']
assert broad['hit']['component']=='AuditFloorBlocker'
assert perch['blocking'] and perch['walkable'] and not perch['line_trace']
assert perch['hit']['component']=='AuditLandingDeck'
assert floor['line_trace'] and merged['line_trace'] and merged['walkable']
assert merged['hit']['component']=='AuditLandingDeck'
for key in ('impact_point_cm','location_cm','time'):
    assert merged['hit'][key]==broad['hit'][key]
assert merged['hit']['impact_point_cm']!=perch['hit']['impact_point_cm']
for key in ('normal','impact_normal'):
    assert merged['hit'][key]==perch['hit'][key]
assert merged['sweep_dist_cm']==broad['sweep_dist_cm']
assert merged['line_dist_cm']==max(broad['sweep_dist_cm'],f32(1.9))
assert native['perch']['modes']==['MOVE_Falling','MOVE_Falling','MOVE_Walking']
assert native['perch']['landed_events'][:2]==[[],[]]
assert [hit['component'] for hit in native['perch']['landed_events'][2]]==['AuditWall']
assert native['no-blocker']['modes']==['MOVE_Walking']*3
assert [hit['component'] for hit in native['no-blocker']['landed_events'][0]]==['AuditWall']
assert not native['no-blocker']['floor_at_stop']['merged']['line_trace']

fixture=read(root/'fixture-input.json')
target,traces,floors,parameters={},{},{},{}
for label in labels:
    functional=read(root/f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images']==[]
    assert load(label,'process.report')['success']
    has_perch=label.startswith('perch')
    assert math.isclose(load(label,'barrier')['top_m'],2.205,abs_tol=2e-7)
    assert math.isclose(load(label,'deck')['top'],fixture['deck_top_m'],abs_tol=2e-7)
    assert math.isclose(load(label,'deck')['center'][0],-15+fixture['deck_x_m'],abs_tol=1e-6)
    if has_perch:
        assert load(label,'floor-blocker')['center']==[f32(x) for x in fixture['blocker_center_m']]
    else:
        assert not file(label,'floor-blocker').exists()
    traces[label]=trace=load(label,'initial-observed')
    assert trace['wall']=='AuditWalkOffBarrier' and trace['second_hit'] is None
    before=load(label,'contact-before-49')
    after=load(label,'contact-after-49')
    assert before['mode']=='Air' and not before['has_basis_displacement'] and before['manifolds']==[]
    assert before['requested_mode'] is None and before['generated_contact']
    assert before['consumed_contact']==(label!='perch-merge-guard')
    assert before['position']==trace['single_hit_position']==after['position']
    assert before['planned_velocity']==trace['single_hit_velocity']==after['velocity']
    assert before['floor']['name']=='AuditLandingDeck' and 0<before['floor']['gap']<.024
    assert before['floor']['impact']<before['floor']['lower_hemisphere']
    assert after['mode']==('Air' if label=='perch-merge-guard' else 'Grounded')
    assert after['has_base']==(after['mode']=='Grounded')
    floors[label]=trace_floor=load(label,'floor-provenance')
    post=trace_floor['phases']['post']
    assert post['origin']==before['position']
    assert post['public']==dict(entity=before['floor']['name'],center=before['floor']['center'],impact=before['floor']['impact'])
    assert post['variants']['none']['result']==post['public']
    assert not event(post['variants']['none'],'selected')['line']
    if has_perch:
        for phase in trace_floor['phases'].values():
            baseline=phase['variants']['none']
            replacement=event(baseline,'perch_replacement')
            selected=event(baseline,'selected')
            assert event(baseline,'center_ray')['hit'] is None
            assert math.isclose(event(baseline,'perch')['radius_m'],.15,abs_tol=1e-6)
            assert replacement['broad']['entity']=='AuditFloorBlocker'
            assert replacement['perch']['entity']=='AuditLandingDeck'
            assert selected['hit']==replacement['perch'] and not selected['line']
            assert selected['floor_distance_m']==replacement['broad_distance_m']
            assert phase['variants']['perch']['result'] is None
            assert phase['variants']['center']['result']==phase['public']
    else:
        assert not any(row['kind']=='perch_replacement' for row in post['variants']['none']['events'])
    target[label]=dict(before=before,after=after)
    parameters[label]=read(file(label,'parameters'))
    row=next(row for row in parameters[label]['after'] if row['ordinal']==49)
    assert row['gameplay']==after['mode'] and row['graph_error'] is None
    assert row['graph_tag']=='Als.LocomotionMode.'+('InAir' if row['gameplay']=='Air' else 'Grounded')

for label in ('perch-line-only','perch-merge-guard','perch-restored'):
    assert traces[label]==traces['perch'] and floors[label]==floors['perch']
assert target['perch-line-only']==target['perch-restored']==target['perch']
assert target['perch']['before']==(target['perch-merge-guard']['before']|{'consumed_contact':True})
assert target['perch-merge-guard']['after']['transitions']==[]
assert target['perch']['after']['transitions']==[['LocomotionState(Air)','LocomotionState(Grounded)']]
assert traces['no-blocker']==traces['no-blocker-merge-guard']
assert floors['no-blocker']==floors['no-blocker-merge-guard']
assert target['no-blocker']==target['no-blocker-merge-guard']

restoration={}
for key in parameters['perch']:
    old,new=[{row['ordinal']:row for row in parameters[label][key]} for label in ('perch','perch-restored')]
    assert old.keys()==new.keys()
    restoration[key]=next((i for i in sorted(old) if old[i]!=new[i]),None)
    assert restoration[key] is None or restoration[key]>50

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
    (cmc,ue/cmc,[(583,605),(5061,5076),(7272,7311),(7460,7527)]),
    (motion,Path(motion),[(3909,3933),(4074,4120)]),
):
    data=path.read_bytes()
    lines=data.decode().splitlines()
    facts.append(dict(key=key,**identity(data),excerpts=[
        dict(start=a,end=b,text='\n'.join(lines[a-1:b])+'\n') for a,b in ranges]))
save('source-facts.json',facts)

inputs={row['path'] for row in read(prior/'summary.json')['external_source_files']}
inputs.update(str(path.relative_to(Path.cwd())) for path in (
    prior/'prepare_reader.py',prior/'wall.request.json',prior/'summary.json',prior/'fixture-input.json',
    prior/'native-origins.json',prior/'native_blocker.inc',prior/'target_blocker.inc',prior/'observer.py',
))
save('summary.json',dict(
    gate='PASS_PERCH_MERGED_LINE_PROVENANCE_AND_SELECTIVE_LANDING_CONTROL',
    findings=['BF144-D294'],new_differences=1,total_differences=291,ordinal=49,
    native=native,target=target,native_observer_disabled_motion_exact_frames=len(observed),
    target_post_perch_replacement=event(floors['perch']['phases']['post']['variants']['none'],'perch_replacement'),
    restored_first_late_parameter_difference=restoration,decoded=decoded,screenshots=0,
    performance_acceptance='paused',
    external_source_files=[dict(path=name,**identity(Path(name).read_bytes())) for name in sorted(inputs)]))
print('Verified the source perch merge and the independent live landing control',flush=True)
