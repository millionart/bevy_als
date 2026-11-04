import gzip
import hashlib
import json
import math
import os
import re
import struct
from pathlib import Path

root=Path(__file__).resolve().parent
prior=root.parent/'537'
labels=('allow','opposed','high')

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

def named(variant,kind):
    row,=[value for value in variant['events'] if value['kind']==kind]
    return row

def mapped(value):
    return [value['x']/100,2+value['z']/100,value['y']/100]

def vertical_down(hit):
    start,end=hit['trace_start_cm'],hit['trace_end_cm']
    return abs(start['x']-end['x'])<1e-6 and abs(start['y']-end['y'])<1e-6 and start['z']-end['z']>49

assert read(root/'build-slopes.report.json')['success']
assert read(root/'wall.native.json.reader.report.json')['success']
assert read(root/'compile-slopes.report.json')['success']
native_path=root/'wall.native.json'
cases={case['name']:case['frames'] for case in read(native_path)['character_animation_probe']['cases']}

def normalized_state(row):
    return re.sub(r'/Temp/Untitled_\d+\.Untitled:', '/Temp/Untitled.Untitled:',
                  json.dumps(row['source_state_after_tick'],sort_keys=True))

for a,b in zip(cases['allow'],cases['allow-unobserved'],strict=True):
    for key in ('movement_before_tick','movement_after_tick','actor_transform','actor_velocity_cm_s',
                'getter_after_tick','mesh_local_pose'):
        assert a[key]==b[key]
    assert normalized_state(a)==normalized_state(b)
    assert 'candidate_down_replay' not in b

native={}
for label in labels:
    frame=cases[label][40]
    assert frame['world_delta_seconds']==f32(.05)
    assert frame['movement_before_tick']['movement_mode_name']=='MOVE_Walking'
    assert frame['movement_before_tick']['velocity_cm_s']=={'x':650,'y':0,'z':0}
    first=frame['blocking_hits'][0]
    assert first['component']=='AuditStepFin' and not first['walkable_hit']
    candidate=frame['candidate_down_replay']
    hit=candidate['hit']
    assert hit['component']=='AuditStepSlope' and hit['blocking'] and not hit['penetrating']
    assert not hit['walkable'] and hit['edge_within']
    assert abs(hit['impact_normal']['z']-.5)<1e-5
    assert abs(hit['location_cm']['z']-(94 if label=='high' else 88))<1e-4
    assert (candidate['direction_dot']<0)==(label=='opposed')
    assert candidate['above_old']==(label=='high')
    actual_down=[hit for hit in frame['blocking_hits'] if vertical_down(hit)]
    if label=='allow':
        down,=actual_down
        assert down['component']=='AuditStepSlope' and not down['walkable_hit']
        assert down['impact_normal']==hit['impact_normal']
        assert math.dist(mapped(down['location_cm']),mapped(hit['location_cm']))<1e-6
        switch,=frame['mode_events']
        assert switch['current']=='MOVE_Falling'
        assert switch['location_cm']==down['move_stop_cm']==frame['actor_transform']['translation']
    else:
        assert actual_down==[]
    native[label]=dict(first_hit=first,candidate=candidate,actual_down=actual_down,
                       position=mapped(frame['actor_transform']['translation']),
                       mode=frame['movement_after_tick']['movement_mode_name'])

start_ends=[(row['first_hit']['trace_start_cm'],row['first_hit']['trace_end_cm']) for row in native.values()]
assert start_ends[0]==start_ends[1]==start_ends[2]
target={}
for label in labels:
    functional=read(root/f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images']==[]
    assert load(label,'process.report')['success']
    geometry=load(label,'step-slope')
    assert geometry['yaw_deg']==(15 if label=='opposed' else 0)
    assert math.isclose(geometry['contact_height_m'],2.94 if label=='high' else 2.88,abs_tol=2e-7)
    trace=load(label,'step-down-replay')
    assert trace['requested']==load(label,'stage-before')['planned_move']
    actual,no_floor,no_angle,neither=[trace['variants'][key] for key in (
        'actual','no-floor-guard','no-angle-guard','no-angle-or-floor-guard')]
    assert actual==no_floor
    assert named(actual,'forward')['hit']['entity']=='AuditStepFin'
    down=named(actual,'down_qualification')['hit']
    assert down['entity']=='AuditStepSlope' and abs(down['normal'][1]-.5)<1e-4
    assert actual['result']['next_y'] is None and abs(actual['result']['remaining'][2])>.04
    assert all(row['kind']!='landing_check' for row in actual['events'])
    landing=named(no_angle,'landing_check')
    assert landing['valid'] is False and no_angle['result']==actual['result']
    assert neither['events']==no_angle['events']
    assert neither['result']['next_y']==landing['position'][1]
    assert abs(neither['result']['remaining'][2])<1e-6
    transaction=load(label,'walking-transaction')
    assert transaction['position']==actual['result']['advanced']
    assert transaction['position'][1]==trace['start'][1]
    after=load(label,'contact-after-40')
    assert after['mode']=='Air'
    if label=='allow':
        assert .10<after['position'][1]-native[label]['position'][1]<.11
    target[label]=dict(trace=trace,transaction=transaction,after=after)

decoded={}
for path in [native_path]+[file(label,name) for label in labels for name in (
    'parameters','report','walking-step','walking-transaction','step-down-replay')]:
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
    (cmc,ue/cmc,[(7582,7633),(7683,7743)]),
    (motion,Path(motion),[(3574,3598),(3814,3829)]),
):
    data=path.read_bytes()
    lines=data.decode().splitlines()
    facts.append(dict(key=key,**identity(data),excerpts=[
        dict(start=a,end=b,text='\n'.join(lines[a-1:b])+'\n') for a,b in ranges]))
save('source-facts.json',facts)

inputs={row['path'] for row in read(prior/'summary.json')['external_source_files']}
inputs.update(str(path.relative_to(Path.cwd())) for path in (
    prior/'summary.json',prior/'observer.py',prior/'prepare_reader.py',prior/'wall.request.json',
    prior/'native_fin.inc',prior/'target_fin.inc',prior/'down_miss.inc',
))
save('summary.json',dict(
    gate='PASS_UNWALKABLE_STEPDOWN_DIRECTION_HEIGHT_AND_TARGET_GUARD_ORDER',
    findings=['BF144-D299'],new_differences=1,total_differences=296,ordinal=40,
    native=native,target=target,native_observer_disabled_motion_exact_frames=len(cases['allow']),
    decoded=decoded,screenshots=0,performance_acceptance='paused',
    external_source_files=[dict(path=name,**identity(Path(name).read_bytes())) for name in sorted(inputs)]))
print('Verified the native allowed unwalkable step and both independent rejection conditions',flush=True)
