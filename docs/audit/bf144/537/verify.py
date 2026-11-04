import gzip
import hashlib
import json
import math
import os
import re
import struct
from pathlib import Path

root=Path(__file__).resolve().parent
prior=root.parent/'536'
labels=('miss','supported')

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

assert read(root/'build-miss.report.json')['success']
assert read(root/'wall.native.json.reader.report.json')['success']
assert read(root/'compile-miss.report.json')['success']
native_path=root/'wall.native.json'
cases={case['name']:case['frames'] for case in read(native_path)['character_animation_probe']['cases']}

def normalized_state(row):
    return re.sub(r'/Temp/Untitled_\d+\.Untitled:', '/Temp/Untitled.Untitled:',
                  json.dumps(row['source_state_after_tick'],sort_keys=True))

for a,b in zip(cases['miss'],cases['miss-unobserved'],strict=True):
    for key in ('movement_before_tick','movement_after_tick','actor_transform','actor_velocity_cm_s',
                'getter_after_tick','mesh_local_pose'):
        assert a[key]==b[key]
    assert normalized_state(a)==normalized_state(b)
    assert 'mode_events' not in b and 'actual_current_floor' not in b

native={}
for label in labels:
    frame=cases[label][40]
    before=frame['movement_before_tick']
    assert before['movement_mode_name']=='MOVE_Walking' and before['walkable_floor']
    assert before['velocity_cm_s']=={'x':650,'y':0,'z':0}
    assert frame['world_delta_seconds']==f32(.05)
    hits=frame['blocking_hits']
    wall=hits[0]
    assert wall['component']=='AuditStepFin' and not wall['penetrating'] and not wall['walkable_hit']
    assert abs(wall['trace_start_cm']['x']+1398)<1e-9
    assert abs(wall['trace_end_cm']['x']-wall['trace_start_cm']['x']-31.25)<1e-5
    end=frame['actor_transform']['translation']
    assert abs(end['x']-wall['trace_end_cm']['x'])<2e-6 and end['y']==-1500
    if label=='miss':
        assert len(hits)==1 and not frame['actual_current_floor']['blocking']
        assert frame['movement_after_tick']['movement_mode_name']=='MOVE_Falling'
        switch,=frame['mode_events']
        assert switch['previous']=='MOVE_Walking' and switch['current']=='MOVE_Falling'
        assert switch['velocity_cm_s']=={'x':625,'y':0,'z':0}
        replay=switch['down_replay']
        assert replay['end_cm']==switch['location_cm']==end
        assert not replay['blocking'] and replay['hit_count']==0
        assert replay['start_cm']['x']==end['x'] and replay['start_cm']['y']==end['y']
        assert abs(replay['start_cm']['z']-end['z']-49.8)<2e-6
        up=f32(45-before['floor_distance_cm'])
        assert abs(replay['start_cm']['z']-92.15-up)<2e-6
        assert frame['actor_velocity_cm_s']['z']==0
    else:
        assert len(hits)==2 and hits[1]['walkable_hit']
        assert hits[1]['component']!='AuditStepFin'
        assert hits[1]['reused_down_hit_floor']['walkable']
        assert frame['actual_current_floor']['walkable']
        assert frame['movement_after_tick']['movement_mode_name']=='MOVE_Walking' and frame['mode_events']==[]
    native[label]=dict(position_m=mapped(end),first_hit=wall,hits=hits,mode_events=frame['mode_events'],
                       velocity_cm_s=frame['actor_velocity_cm_s'])

a,b=[native[label]['first_hit'] for label in labels]
assert all(a[key]==b[key] for key in a if key not in ('floor_at_hit','floor_at_move_stop'))

target={}
for label in labels:
    functional=read(root/f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images']==[]
    assert load(label,'process.report')['success']
    fin=load(label,'step-fin')
    assert math.dist(fin['center'],[-13.855,2.08,-14.73])<2e-6
    assert math.dist(fin['size'],[.02,.20,.02])<1e-7
    replay=load(label,'step-down-replay')
    assert replay['start']==[f32(-13.98),f32(2.9215),-15]
    assert replay['requested']==load(label,'stage-before')['planned_move']
    actual,without=[replay['variants'][key] for key in ('actual','no-floor-guard')]
    assert actual==without
    assert named(actual,'forward')['hit']['entity']=='AuditStepFin'
    down=named(actual,'down_request')
    sweep=named(actual,'capsule')
    assert abs(down['start'][1]-down['end'][1]-.498)<2e-7
    transaction=load(label,'walking-transaction')
    post=load(label,'contact-after-40')
    assert transaction['position'][0]==actual['result']['advanced'][0]
    if label=='miss':
        assert sweep['hit'] is None and actual['result']['next_y'] is None
        assert all(row['kind']!='landing_check' for row in actual['events'])
        assert actual['result']['remaining'][2]<-.04
        assert transaction['position'][1]==replay['start'][1]
        assert post['mode']=='Air'
        source_down=native['miss']['mode_events'][0]['down_replay']
        assert math.dist(down['start'],mapped(source_down['start_cm']))<3e-4
        assert math.dist(down['end'],mapped(source_down['end_cm']))<3e-4
        assert .07<post['position'][1]-native[label]['position_m'][1]<.08
        assert post['position'][2]<-15.05
    else:
        assert sweep['hit']['entity']=='AuditWalkOffPlatform'
        assert named(actual,'landing_check')['valid']
        assert transaction['position'][1]==actual['result']['next_y']
        assert post['mode']=='Grounded'
    target[label]=dict(replay=replay,transaction=transaction,post=post)

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
primitive='Engine/Source/Runtime/Engine/Private/Components/PrimitiveComponent.cpp'
motion='src/als/character/movement/motion.rs'
facts=[]
for key,path,ranges in (
    (cmc,ue/cmc,[(7582,7633),(7683,7697),(7745,7775)]),
    (primitive,ue/primitive,[(3346,3355)]),
    (motion,Path(motion),[(3574,3597),(3813,3832)]),
):
    data=path.read_bytes()
    lines=data.decode().splitlines()
    facts.append(dict(key=key,**identity(data),excerpts=[
        dict(start=a,end=b,text='\n'.join(lines[a-1:b])+'\n') for a,b in ranges]))
save('source-facts.json',facts)

inputs={row['path'] for row in read(prior/'summary.json')['external_source_files']}
inputs.update(str(path.relative_to(Path.cwd())) for path in (
    prior/'prepare_reader.py',prior/'observer.py',prior/'wall.request.json',
    prior/'step_probe.rs',prior/'step_down_floor.inc',prior/'summary.json',
))
save('summary.json',dict(
    gate='PASS_ACTUAL_STEPDOWN_MISS_COMMITS_SOURCE_BUT_REJECTS_TARGET',
    new_differences=1,total_differences=293,findings=['BF144-D296'],ordinal=40,
    native=native,target=target,native_observer_disabled_motion_exact_frames=len(cases['miss']),
    supported_target_post_lateral_residual_m=target['supported']['post']['position'][2]-native['supported']['position_m'][2],
    decoded=decoded,screenshots=0,performance_acceptance='paused',
    external_source_files=[dict(path=name,**identity(Path(name).read_bytes())) for name in sorted(inputs)]))
print('Verified actual StepDown miss and the independent early target rejection',flush=True)
