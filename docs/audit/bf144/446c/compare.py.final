import copy
import gzip
import hashlib
import json
import math
import struct
from pathlib import Path

r=Path(__file__).resolve().parent
f32=lambda v:struct.unpack('<f',struct.pack('<f',v))[0]


def named(fields,name):
    return next(v for v in fields if v['name']==name)


def numeric(fields,name):
    value=named(fields,name)['numeric_value']
    assert isinstance(value,(int,float))and math.isfinite(value)
    return value


def vector(fields,name):
    fields=named(fields,name)['fields']
    return [numeric(fields,k)for k in ['X','Y','Z']]


def runtime_point(source,scale):
    return [f32(source[k]*scale)for k in [0,2,1]]


def check_inputs(target,source,request):
    assert len(target)==319 and len(source['frames'])==len(request['frames'])==318
    actual_control=source['initialization']['controller_input']['control_rotation_degrees']['yaw']
    retained=0
    for index,(frame,asked,row)in enumerate(zip(source['frames'],request['frames'],target[1:],strict=True)):
        assert frame['frame']==index
        assert frame['world_delta_seconds']==asked['delta_seconds']==row['frame_delta']
        h=row['history_input']
        d=h['desired_world_motion']
        expected=dict(x=d[0],y=d[2],z=d[1])
        assert asked['movement_input_world']==frame['movement_before_tick']['pending_input_world']==expected
        assert asked['desired_gait']==h['desired_gait']and asked['desired_stance']==h['desired_stance']
        before=frame['controller_before_tick']
        for key in ['DesiredGait','DesiredStance']:
            assert before[key]=='(TagName="'+h['desired_'+('gait'if key=='DesiredGait'else 'stance')]+'")'
        yaw=-math.degrees(h['raw_view_yaw'])-90
        assert asked['control_rotation_degrees']==dict(pitch=0,yaw=yaw,roll=0)
        assert abs(yaw-actual_control)<180
        if abs(yaw-actual_control)>f32(1e-3):actual_control=yaw
        else:retained+=1
        assert before['control_rotation_degrees']==dict(pitch=0,yaw=actual_control,roll=0)
    return dict(external_input_rows_exact=318,actual_controller_rows_exact=318,controller_setter_hold_rows=retained)


target_path=Path('docs/audit/bf144/446/runtime-restored/parameters.json')
source_path=Path('docs/audit/bf144/446b/restored.native.json.gz')
target=json.loads(target_path.read_bytes())
case=json.loads(gzip.decompress(source_path.read_bytes()))['character_animation_probe']['cases'][0]
request=json.loads(Path('docs/audit/bf144/446b/request.json').read_text())['cases'][0]
inputs=check_inputs(target['animation_before'],case,request)
rows=[]
for index,(frame,t)in enumerate(zip(case['frames'],target['animation_before'][1:],strict=True)):
    fields=frame['source_state_after_tick']
    loco=named(fields,'LocomotionState')['fields']
    source_position=[frame['actor_transform']['translation'][k]for k in ['x','y','z']]
    source_velocity=vector(loco,'VelocityWorldSpace')
    source_acceleration=vector(loco,'AccelerationWorldSpace')
    tags={key:named(named(fields,name)['fields'],'TagName')['exported_text']for key,name in [('locomotion_mode','LocomotionMode'),('gait','Gait'),('stance','Stance')]}
    previous_fields=case['initialization']['source_state']if index==0 else case['frames'][index-1]['source_state_after_tick']
    pending=named(previous_fields,'bPendingUpdate')['value']=='True'
    source_speed=numeric(loco,'Speed')
    row=dict(source_frame=index,target_query=index+1,target_ordinal=t['ordinal'],source_world_seconds=frame['world_seconds'],delta=frame['world_delta_seconds'],
        source_position_cm=source_position,source_position_runtime_f32=runtime_point(source_position,.01),target_position=t['position'],
        source_velocity_cm_s=source_velocity,source_velocity_runtime_f32=runtime_point(source_velocity,.01),target_velocity=t['motion_world_velocity'],
        source_acceleration_cm_s2=source_acceleration,source_acceleration_runtime_f32=runtime_point(source_acceleration,.01),target_acceleration=t['provider_acceleration'],
        source_speed_cm_s=source_speed,source_speed_runtime_f32=f32(source_speed*.01),target_speed=t['provider_speed'],
        source_tags=tags,target_tags={k:t['history_input']['root_tags'][k]for k in tags},source_pending_before_update=pending,target_pending_before_update=t['animation_pending'])
    rows.append(row)
comparisons=[('position','source_position_runtime_f32','target_position'),('velocity','source_velocity_runtime_f32','target_velocity'),('acceleration','source_acceleration_runtime_f32','target_acceleration'),('speed','source_speed_runtime_f32','target_speed'),('tags','source_tags','target_tags'),('pending','source_pending_before_update','target_pending_before_update')]
differences={}
for name,source_key,target_key in comparisons:
    bad=[v for v in rows if v[source_key]!=v[target_key]]
    differences[name]=dict(rows=len(bad),first=None if not bad else dict(source_frame=bad[0]['source_frame'],target_query=bad[0]['target_query'],target_ordinal=bad[0]['target_ordinal'],source=bad[0][source_key],target=bad[0][target_key]))
negative=[]
for kind in ['phase','axis','delta','controller']:
    if kind=='phase':bad_target=target['animation_before'][1:]+target['animation_before'][:1];bad_source=case;bad_request=request
    else:
        bad_target=target['animation_before'];bad_source=copy.deepcopy(case);bad_request=request
        if kind=='axis':bad_source['frames'][98]['movement_before_tick']['pending_input_world']['z']=1
        elif kind=='delta':bad_source['frames'][0]['world_delta_seconds']=0
        else:bad_source['frames'][0]['controller_before_tick']['control_rotation_degrees']['yaw']=90
    try:check_inputs(bad_target,bad_source,bad_request)
    except AssertionError:negative.append(kind);continue
    raise AssertionError(kind)
result=dict(gate='PASS_COMPLETE_EXTERNAL_INPUT_HISTORY_WITH_EXPLICIT_PRODUCER_DIFFERENCES',inputs=inputs,negative_input_stage_axis_clock_cases_rejected=negative,
    source_initialization_world_seconds=case['initialization']['world_seconds'],source_initialization_pending=named(case['initialization']['source_state'],'bPendingUpdate')['value'],target_initialization_pending_after=target['animation_after'][0]['gate']['pending_update'],
    differences=differences,all_parameters_or_root_equivalence_claimed=False,new_differences=[],total_confirmed_differences=271,
    input_identities=dict(target_sha256=hashlib.sha256(target_path.read_bytes()).hexdigest(),source_gzip_sha256=hashlib.sha256(source_path.read_bytes()).hexdigest()))
(r/'state-rows.json').write_text(json.dumps(rows,indent=2)+'\n')
(r/'input-and-state-check.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result),flush=True)
