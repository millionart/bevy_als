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
labels=('near','edge')

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

def named_event(variant,kind):
    row,=[value for value in variant['events'] if value['kind']==kind]
    return row

def mapped_position(value):
    return [value['x']/100,2+value['z']/100,value['y']/100]

assert read(root/'build-step-down.report.json')['success']
assert read(root/'wall.native.json.reader.report.json')['success']
assert read(root/'compile-step-down.report.json')['success']
assert not read(root/'observer-format-anchor.report.json')['success']
native_path=root/'wall.native.json'
cases={case['name']:case['frames'] for case in read(native_path)['character_animation_probe']['cases']}

def normalized_state(row):
    return re.sub(r'/Temp/Untitled_\d+\.Untitled:', '/Temp/Untitled.Untitled:',
                  json.dumps(row['source_state_after_tick'],sort_keys=True))

for a,b in zip(cases['edge'],cases['edge-unobserved'],strict=True):
    for key in ('movement_before_tick','movement_after_tick','actor_transform','actor_velocity_cm_s',
                'getter_after_tick','mesh_local_pose'):
        assert a[key]==b[key]
    assert normalized_state(a)==normalized_state(b)
    assert 'actual_current_floor' not in b and 'mode_events' not in b

native={}
for label in labels:
    frame=cases[label][40]
    assert frame['movement_before_tick']['movement_mode_name']=='MOVE_Walking'
    assert frame['world_delta_seconds']==f32(.03)
    wall,down=frame['blocking_hits']
    assert wall['component']==down['component']=='AuditLandingDeck'
    assert not wall['walkable_hit'] and down['walkable_hit'] and not down['penetrating']
    assert down['trace_start_cm']['x']==down['trace_end_cm']['x']
    assert down['trace_start_cm']['y']==down['trace_end_cm']['y']
    assert down['trace_start_cm']['z']>down['trace_end_cm']['z']
    reused=down['reused_down_hit_floor']
    assert reused['blocking'] and not reused['line_trace']
    assert reused['trace_start_cm']==down['trace_start_cm'] and reused['trace_end_cm']==down['trace_end_cm']
    assert reused['hit']['location_cm']==down['location_cm']
    assert 1.04 < reused['distance_cm'] < 1.06
    accepted=label=='near'
    assert reused['walkable']==accepted
    fresh=down['floor_at_move_stop']
    assert fresh['broad']['walkable'] and fresh['walkable']==accepted
    assert frame['movement_after_tick']['movement_mode_name']==('MOVE_Walking' if accepted else 'MOVE_Falling')
    actual=frame['actual_current_floor']
    if accepted:
        assert actual['walkable'] and actual['hit']==reused['hit']
        assert actual['trace_start_cm']==down['trace_start_cm'] and actual['trace_end_cm']==down['trace_end_cm']
        assert abs(actual['distance_cm']-2.15)<1e-6
        assert frame['mode_events']==[]
    else:
        assert fresh['perch_requested'] and not fresh['perch_valid'] and not fresh['perch']['blocking']
        assert fresh['merged']['blocking'] and not fresh['merged']['walkable']
        switch,=frame['mode_events']
        assert switch['previous']=='MOVE_Walking' and switch['current']=='MOVE_Falling'
        assert switch['location_cm']==down['move_stop_cm']==frame['actor_transform']['translation']
        assert switch['velocity_cm_s']['z']==0
        assert not actual['blocking']
    native[label]=dict(wall=wall,step_down=down,actual_floor=actual,events=frame['mode_events'],
                       position_m=mapped_position(frame['actor_transform']['translation']))

target={}
for label in labels:
    functional=read(root/f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images']==[]
    assert load(label,'process.report')['success']
    assert abs(load(label,'deck')['top']-2.18)<2e-7
    assert math.isclose(load(label,'deck')['center'][0],-12.7 if label=='near' else -12.63,abs_tol=1e-6)
    replay=load(label,'step-down-replay')
    stage=load(label,'stage-before')
    assert replay['start']==[f32(-14.01),f32(2.9215),-15]
    assert replay['requested']==stage['planned_move']
    actual,ablated=[replay['variants'][key] for key in ('actual','no-floor-guard')]
    assert actual['events']==ablated['events']
    assert named_event(actual,'forward')['hit']['entity']=='AuditLandingDeck'
    check=named_event(actual,'landing_check')
    assert check['down_hit']['entity']=='AuditLandingDeck'
    assert check['down_hit']['normal'][1]>.71 and check['position'][1]>replay['start'][1]
    transaction=load(label,'walking-transaction')
    after=load(label,'contact-after-40')
    assert transaction['position'][0]==actual['result']['advanced'][0]
    assert after['mode']=='Grounded' and after['transitions']==[]
    if label=='near':
        assert check['valid'] and actual['result']==ablated['result']
        assert transaction['position'][1]==actual['result']['next_y']
        assert math.dist(after['position'],native[label]['position_m'])<2e-6
    else:
        assert not check['valid'] and actual['result']['next_y'] is None
        assert transaction['position'][1]==replay['start'][1]
        assert ablated['result']['next_y']==check['position'][1]
        assert ablated['result']['advanced']==actual['result']['advanced']
        assert ablated['result']['remaining'][0]>.06 and abs(actual['result']['remaining'][0])<1e-7
        outcome,=[row for row in check['floor_events'] if row['kind']=='perch_outcome']
        assert outcome['hit'] is None
        assert any(row['kind']=='capsule_qualification' and row['valid'] for row in check['floor_events'])
        assert load(label,'contact-before-40')['floor']['name']=='AuditLandingDeck'
        retained=transaction['end_floor']['variants']['none']
        assert retained['result']['entity']=='AuditLandingDeck'
        assert named_event(retained,'perch_outcome')['hit']['entity']=='AuditWalkOffPlatform'
        assert .08<native[label]['position_m'][1]-after['position'][1]<.10
    target[label]=dict(replay=replay,transaction=transaction,after=after)

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
plugin=Path(json.loads(Path('tools/env.local.json').read_bytes())['ue_project']).parent/'Plugins/ALS-Refactored/Source/ALS'
cmc='Engine/Source/Runtime/Engine/Private/Components/CharacterMovementComponent.cpp'
motion='src/als/character/movement/motion.rs'
facts=[]
for key,path,ranges in (
    (cmc,ue/cmc,[(7068,7094),(7679,7772)]),
    ('source:Private/AlsCharacterMovementComponent.cpp',plugin/'Private/AlsCharacterMovementComponent.cpp',[(427,469),(510,555)]),
    (motion,Path(motion),[(3506,3537),(3574,3618),(3803,3825)]),
):
    data=path.read_bytes()
    lines=data.decode().splitlines()
    facts.append(dict(key=key,**identity(data),excerpts=[
        dict(start=a,end=b,text='\n'.join(lines[a-1:b])+'\n') for a,b in ranges]))
save('source-facts.json',facts)

inputs={row['path'] for row in read(prior/'summary.json')['external_source_files']}
inputs.update(str(path.relative_to(Path.cwd())) for path in (
    prior/'prepare_reader.py',prior/'observer.py',prior/'wall.request.json',prior/'perch_distance.inc',
    prior/'walking-first-cm.json',root.parent/'533'/'walking_probe.rs',
))
save('summary.json',dict(
    gate='PASS_ACTUAL_STEPDOWN_FLOOR_REUSE_AND_BLOCKING_UNWALKABLE_DIVERGENCE',
    new_differences=1,total_differences=292,findings=['BF144-D295'],ordinal=40,
    native=native,target=target,native_observer_disabled_motion_exact_frames=len(cases['edge']),
    decoded=decoded,screenshots=0,performance_acceptance='paused',
    external_source_files=[dict(path=name,**identity(Path(name).read_bytes())) for name in sorted(inputs)]))
print('Verified actual StepDown reuse and the isolated target floor rejection',flush=True)
