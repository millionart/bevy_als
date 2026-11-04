import gzip
import hashlib
import json
import math
import os
import re
import struct
from pathlib import Path

root=Path(__file__).resolve().parent
prior=root.parent/'538'
labels=('grazing-verified','source-gap','restored')

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

assert read(root/'build-grazing.report.json')['success']
assert read(root/'wall.native.json.reader.report.json')['success']
assert read(root/'compile-grazing.report.json')['success']
assert read(root/'compile-conflicting-position.report.json')['success']
assert read(root/'prepare_gap.report.json')['success']

native_path=root/'wall.native.json'
cases={case['name']:case['frames'] for case in read(native_path)['character_animation_probe']['cases']}

def normalized_state(row):
    return re.sub(r'/Temp/Untitled_\d+\.Untitled:', '/Temp/Untitled.Untitled:',
                  json.dumps(row['source_state_after_tick'],sort_keys=True))

for a,b in zip(cases['grazing'],cases['grazing-unobserved'],strict=True):
    for key in ('movement_before_tick','movement_after_tick','actor_transform','actor_velocity_cm_s',
                'getter_after_tick','mesh_local_pose'):
        assert a[key]==b[key]
    assert normalized_state(a)==normalized_state(b)
    assert 'actual_current_floor' not in b and 'mode_events' not in b

native=cases['grazing'][40]
forward,down=native['blocking_hits']
assert forward['component']=='AuditStepFin' and not forward['walkable_hit'] and not forward['penetrating']
assert down['component']!='AuditStepFin' and down['walkable_hit'] and not down['penetrating']
assert native['movement_after_tick']['movement_mode_name']=='MOVE_Walking'
assert native['mode_events']==[]
source=read(root/'source-gap.json')
floor=down['reused_down_hit_floor']
actual=native['actual_current_floor']
assert floor['walkable'] and actual['walkable']
assert actual['hit']==floor['hit'] and actual['trace_start_cm']==down['trace_start_cm']
assert source['gap_m']==floor['distance_cm']/100 and source['current_floor_gap_m']==actual['distance_cm']/100
assert source['move_stop_cm']==down['move_stop_cm']
assert .0104<source['gap_m']<.0106 and abs(source['current_floor_gap_m']-.0215)<1e-8
native_p=native['actor_transform']['translation']
assert abs(native_p['z']-(source['move_stop_cm']['z']+actual['distance_cm']-floor['distance_cm']))<2e-6
source_position=[native_p['x']/100,2+native_p['z']/100,native_p['y']/100]

target={}
for label in labels:
    functional=read(root/f'{label}-functional.json')
    assert functional['pass'] and not functional['issues'] and functional['images']==[]
    assert load(label,'process.report')['success']
    fin=load(label,'step-fin')
    assert abs(fin['center'][1]+fin['size'][1]/2-2.173)<2e-7
    replay=load(label,'step-down-replay')
    actual_step=replay['variants']['actual']
    assert named(actual_step,'forward')['hit']['entity']=='AuditStepFin'
    assert named(actual_step,'landing_check')['valid']
    down_hit=named(actual_step,'capsule')
    assert down_hit['hit']['entity']=='AuditWalkOffPlatform'
    landing=named(actual_step,'landing_check')
    clearance=landing['position'][1]-(down_hit['origin'][1]-down_hit['hit']['distance_m'])
    assert abs(clearance-.024)<3e-7
    transaction=load(label,'walking-transaction')
    assert transaction['position'][1]==actual_step['result']['next_y']==landing['position'][1]
    solver_control=load(label,'solver-control')
    entry=load(label,'solver-entry')
    exit=load(label,'solver-exit')
    assert not solver_control['masked_fin'] and len(exit['steps'])==6
    assert all(step['velocity']==entry['velocity'] for step in exit['steps'])
    assert abs(exit['velocity'][2])<1e-6 and exit['velocity'][1]==0
    gap=load(label,'gap-control')
    assert gap['floor']=='AuditWalkOffPlatform' and gap['initial_position']==exit['position']
    assert gap['before_gap_m']==f32(gap['initial_position'][1]-gap['floor_center'])
    assert .0237<gap['before_gap_m']<.0239
    before=load(label,'contact-before-40')
    after=load(label,'contact-after-40')
    assert before['position']==gap['position'] and before['floor']['name']=='AuditWalkOffPlatform'
    assert after['mode']=='Grounded' and after['transitions']==[]
    assert abs(after['position'][0]-source_position[0])<2e-6
    assert abs(after['position'][2]-source_position[2])<1e-6
    residual=after['position'][1]-source_position[1]
    if label=='source-gap':
        assert gap['source_gap_m']==f32(source['gap_m']) and abs(gap['after_gap_m']-source['gap_m'])<2e-7
        assert before['floor']['gap']<.019 and .0002<residual<.0003
        assert abs(after['position'][1]-(before['floor']['center']+.0215))<2e-6
    else:
        assert gap['source_gap_m'] is None and gap['position']==gap['initial_position']
        assert before['floor']['gap']<=.024 and after['position']==gap['position']
        assert .0024<residual<.0026
    target[label]=dict(replay=replay,transaction=transaction,solver_control=solver_control,
                       entry=entry,exit=exit,gap=gap,before=before,after=after,height_residual_m=residual)

baseline=target['grazing-verified']
assert target['restored']==baseline
for key in ('replay','transaction','solver_control','entry','exit'):
    assert target['source-gap'][key]==baseline[key]
assert target['source-gap']['after']['velocity']==baseline['after']['velocity']

assert raw(file("grazing-verified","parameters"))==raw(file("restored","parameters"))
restoration={}
for key in load('grazing-verified','parameters'):
    old,new=[{row['ordinal']:row for row in load(label,'parameters')[key]} for label in ('grazing-verified','restored')]
    assert old.keys()==new.keys()
    restoration[key]=next((i for i in sorted(old) if old[i]!=new[i]),None)
    assert restoration[key] is None or restoration[key]>40

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
    (cmc,ue/cmc,[(6296,6349),(7683,7697),(7745,7765)]),
    (primitive,ue/primitive,[(3095,3099)]),
    (motion,Path(motion),[(3520,3526),(3574,3598),(4009,4031),(4067,4077)]),
):
    data=path.read_bytes()
    lines=data.decode().splitlines()
    facts.append(dict(key=key,**identity(data),excerpts=[
        dict(start=a,end=b,text='\n'.join(lines[a-1:b])+'\n') for a,b in ranges]))
save('source-facts.json',facts)

inputs={row['path'] for row in read(prior/'summary.json')['external_source_files']}
inputs.update(str(path.relative_to(Path.cwd())) for path in (
    prior/'summary.json',prior/'observer.py',prior/'solver_probe.rs',
    root.parent/'537'/'prepare_reader.py',root.parent/'537'/'wall.request.json',
    root.parent/'537'/'target_fin.inc',root.parent/'537'/'native_fin.inc',root.parent/'537'/'down_miss.inc',
))
save('summary.json',dict(
    gate='PASS_GRAZING_STEPUP_HEIGHT_AND_NATIVE_GAP_CONTROL',findings=['BF144-D298'],
    new_differences=1,total_differences=295,ordinal=40,source_position_m=source_position,
    source_gap=source,native_observer_disabled_motion_exact_frames=len(cases['grazing']),
    target=target,restored_first_late_parameter_difference=restoration,restored_parameters_byte_exact=True,decoded=decoded,
    screenshots=0,performance_acceptance='paused',
    external_source_files=[dict(path=name,**identity(Path(name).read_bytes())) for name in sorted(inputs)]))
print('Verified the natural grazing step and source-gap-only height control',flush=True)
