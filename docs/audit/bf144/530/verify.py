import gzip
import hashlib
import json
import math
import os
import re
import struct
from pathlib import Path

root = Path(__file__).resolve().parent
prior = root.parent/'528'
labels = ('line','no-blocker','line-capsule-only','no-blocker-capsule-only','line-restored')

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

def selected(variant):
    event, = [row for row in variant['events'] if row['kind']=='selected']
    return event

assert read(root/'build-line.report.json')['success']
assert read(root/'wall.native.json.reader.report.json')['success']
assert read(root/'compile-uncontrolled.report.json')['success']
assert read(root/'compile-capsule-control.report.json')['success']
native_path = root/'wall.native.json'
cases = {case['name']:case['frames'] for case in read(native_path)['character_animation_probe']['cases']}
observed, disabled = [cases[name] for name in ('line','line-unobserved')]

def normalized_state(row):
    value = json.dumps(row['source_state_after_tick'],sort_keys=True)
    return re.sub(r'/Temp/Untitled_\d+\.Untitled:', '/Temp/Untitled.Untitled:', value)

for a,b in zip(observed,disabled,strict=True):
    for key in ('movement_before_tick','movement_after_tick','actor_transform','actor_velocity_cm_s',
                'getter_after_tick','mesh_local_pose'):
        assert a[key]==b[key]
    assert normalized_state(a)==normalized_state(b)
    assert ('landed_events' in a)==(a['frame'] in (49,50,51))
    assert 'landed_events' not in b and 'blocking_hits' not in b

native = {}
for label in ('line','no-blocker'):
    case=cases[label]
    frame=case[49]
    assert frame['movement_before_tick']['movement_mode_name']=='MOVE_Falling'
    first=frame['blocking_hits'][0]
    assert first['component']=='AuditWall' and not first['penetrating'] and not first['walkable_hit']
    assert not first['normals_equal'] and first['edge_within_at_move_stop']
    assert first['normal']['z']>.001 and 0<first['time']<1
    floor=first['floor_at_move_stop']
    assert floor['walkable'] and floor['valid_landing_spot'] and floor['component']=='AuditLandingDeck'
    assert floor['floor_hit']['below_lower_hemisphere'] and floor['floor_hit']['edge_within']
    assert floor['line_trace']==(label=='line')
    native[label]=dict(
        modes=[case[i]['movement_after_tick']['movement_mode_name'] for i in (49,50,51)],
        first_hit=first,landed_events=[case[i]['landed_events'] for i in (49,50,51)])
first, control = [native[label]['first_hit'] for label in ('line','no-blocker')]
assert all(first[key]==control[key] for key in first if key not in ('floor_at_hit','floor_at_move_stop'))
assert native['line']['modes']==['MOVE_Falling','MOVE_Falling','MOVE_Walking']
assert native['line']['landed_events'][:2]==[[],[]]
assert [hit['component'] for hit in native['line']['landed_events'][2]]==['AuditWall']
assert native['no-blocker']['modes']==['MOVE_Walking']*3
assert [hit['component'] for hit in native['no-blocker']['landed_events'][0]]==['AuditLandingDeck']
point=first['move_stop_cm']
origins=read(root/'native-origins.json')
assert origins['line']==origins['no-blocker']==[point['x']/100,2+point['z']/100,point['y']/100]

fixture=read(root/'fixture-input.json')
target,traces,floors,parameters={},{},{},{}
for label in labels:
    functional=read(root/f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images']==[]
    assert load(label,'process.report')['success']
    line=label.startswith('line')
    assert math.isclose(load(label,'barrier')['top_m'],2.205,abs_tol=2e-7)
    assert math.isclose(load(label,'deck')['top'],fixture['deck_top_m'],abs_tol=2e-7)
    assert math.isclose(load(label,'deck')['center'][0],-15+fixture['deck_x_m'],abs_tol=1e-6)
    if line:
        blocker=load(label,'floor-blocker')
        assert blocker['center']==[f32(x) for x in fixture['blocker_center_m']]
        assert blocker['size']==[f32(x) for x in fixture['blocker_size_m']]
    else:
        assert not file(label,'floor-blocker').exists()
    traces[label]=trace=load(label,'initial-observed')
    assert trace['wall']=='AuditWalkOffBarrier' and trace['second_hit'] is None
    before=load(label,'contact-before-49')
    after=load(label,'contact-after-49')
    assert before['mode']=='Air' and not before['has_basis_displacement'] and before['manifolds']==[]
    assert before['requested_mode'] is None and before['generated_contact']
    assert before['consumed_contact']==(label!='line-capsule-only')
    assert before['position']==trace['single_hit_position']==after['position']
    assert before['planned_velocity']==trace['single_hit_velocity']==after['velocity']
    assert before['floor']['name']=='AuditLandingDeck' and 0<before['floor']['gap']<.024
    assert before['floor']['impact']<before['floor']['lower_hemisphere']
    assert after['mode']==('Air' if label=='line-capsule-only' else 'Grounded')
    assert after['has_base']==(after['mode']=='Grounded')
    floors[label]=floor=load(label,'floor-provenance')
    post=floor['phases']['post']
    assert post['origin']==before['position']
    assert post['public']==dict(entity=before['floor']['name'],center=before['floor']['center'],impact=before['floor']['impact'])
    assert post['variants']['none']['result']==post['public']
    assert selected(post['variants']['none'])['line']==line
    if line:
        for phase in floor['phases'].values():
            baseline=phase['variants']['none']
            assert selected(baseline)['line']
            capsule, = [event for event in baseline['events'] if event['kind']=='capsule_qualification']
            assert capsule['hit']['entity']=='AuditFloorBlocker' and not capsule['valid']
            assert .706 < capsule['hit']['normal'][1] < .708
            ray, = [event for event in baseline['events'] if event['kind']=='center_ray']
            assert ray['hit']['entity']=='AuditLandingDeck'
            assert not selected(phase['variants']['center'])['line']
            assert phase['variants']['center']['result']!=phase['public']
    target[label]=dict(before=before,after=after)
    parameters[label]=read(file(label,'parameters'))
    row=next(row for row in parameters[label]['after'] if row['ordinal']==49)
    assert row['gameplay']==after['mode'] and row['graph_error'] is None
    assert row['graph_tag']=='Als.LocomotionMode.'+('InAir' if row['gameplay']=='Air' else 'Grounded')

assert traces['line']==traces['line-capsule-only']==traces['line-restored']
assert floors['line']==floors['line-capsule-only']==floors['line-restored']
assert target['line']['before']==(target['line-capsule-only']['before']|{'consumed_contact':True})
assert target['line']['after']['transitions']==[['LocomotionState(Air)','LocomotionState(Grounded)']]
assert target['line-capsule-only']['after']['transitions']==[]
assert target['line-restored']==target['line']
assert traces['no-blocker-capsule-only']==traces['no-blocker']
assert floors['no-blocker-capsule-only']==floors['no-blocker']
assert target['no-blocker-capsule-only']==target['no-blocker']

restoration={}
for key in parameters['line']:
    old,new=[{row['ordinal']:row for row in parameters[label][key]} for label in ('line','line-restored')]
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
    (cmc,ue/cmc,[(5061,5076),(7163,7199),(7374,7424)]),
    (motion,Path(motion),[(3846,3870),(3889,3933),(4074,4120)]),
):
    data=path.read_bytes()
    lines=data.decode().splitlines()
    facts.append(dict(key=key,**identity(data),excerpts=[
        dict(start=a,end=b,text='\n'.join(lines[a-1:b])+'\n') for a,b in ranges]))
save('source-facts.json',facts)

inputs={row['path'] for row in read(root.parent/'529'/'summary.json')['external_source_files']}
inputs.update(str(path.relative_to(Path.cwd())) for path in (
    prior/'prepare_reader.py',prior/'wall.request.json',prior/'summary.json',prior/'floor_hit.inc',
    root.parent/'529'/'observer.py',root.parent/'529'/'floor_trace.rs',
))
save('summary.json',dict(
    gate='PASS_NATIVE_LINE_REJECTION_AND_TARGET_LINE_CONSUMER_ABLATION',findings=['BF144-D293'],
    new_differences=1,total_differences=290,ordinal=49,native=native,target=target,
    native_observer_disabled_motion_exact_frames=len(observed),
    source_line_passes_other_landing_qualifiers=True,target_alternate_floor_when_center_ray_disabled=True,
    restored_first_late_parameter_difference=restoration,decoded=decoded,screenshots=0,
    performance_acceptance='paused',
    external_source_files=[dict(path=name,**identity(Path(name).read_bytes())) for name in sorted(inputs)]))
print('Verified native line rejection, live capsule-only consumer control and restoration',flush=True)
