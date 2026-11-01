import gzip
import json
from pathlib import Path

r=Path(__file__).resolve().parent
report=json.loads(gzip.decompress(Path('docs/audit/bf144/446f/restored.native.json.gz').read_bytes()))
source=report['character_animation_probe']['cases'][0]
def fields(snapshot,name):return next(v for v in snapshot if v['name']==name)['fields']
def number(items,name):return next(v['numeric_value']for v in items if v['name']==name)
def boolean(items,name):return next(v['exported_text']for v in items if v['name']==name)=='True'
properties=report['blueprint_cdo_properties']
assert next(v['exported_text']for v in properties if v['name']=='ViewMode')=='(TagName="Als.ViewMode.ThirdPerson")'
assert next(v['exported_text']for v in properties if v['name']=='DesiredRotationMode')=='(TagName="Als.RotationMode.ViewDirection")'
assert not boolean(properties,'bDesiredAiming')
rows=[]
for index,frame in enumerate(source['frames']):
    state=frame['source_state_after_tick']
    previous=source['initialization']['source_state']if index==0 else source['frames'][index-1]['source_state_after_tick']
    head=fields(previous,'HeadState');after=fields(state,'HeadState');view=fields(state,'ViewState');loco=fields(state,'LocomotionState')
    assert not boolean(fields(state,'MovementBase'),'bHasRelativeRotation')
    body=fields(state,'LocomotionState');rotation=next(v['fields']for v in body if v['name']=='RotationWorldSpace')
    rows.append(dict(source_frame=index,target_query=index+1,
        previous=dict(initialization_required=boolean(head,'bInitializationRequired'),switching_look_sides=boolean(head,'bSwitchingLookSides'),pitch_angle=number(head,'PitchAngle'),yaw_angle=number(head,'YawAngle'),yaw_velocity=number(head,'YawVelocity'),yaw_amount=number(head,'YawAmount')),
        input=dict(delta_seconds=frame['world_delta_seconds'],delta_seconds_without_time_dilation=frame['world_delta_seconds'],first_person=False,velocity_direction=False,has_input=boolean(loco,'bHasInput'),input_yaw_world=number(loco,'InputYawAngleWorldSpace'),target_yaw_world=number(loco,'TargetYawAngleWorldSpace'),body_yaw_world=number(rotation,'Yaw'),body_yaw_velocity_world=number(loco,'YawVelocityWorldSpace'),view_yaw_angle=number(view,'YawAngle'),view_pitch_angle=number(view,'PitchAngle'),movement_base_relative_yaw=0),
        output=dict(initialization_required=boolean(after,'bInitializationRequired'),switching_look_sides=boolean(after,'bSwitchingLookSides'),pitch_angle=number(after,'PitchAngle'),yaw_angle=number(after,'YawAngle'),yaw_velocity=number(after,'YawVelocity'),yaw_amount=number(after,'YawAmount'))))
assert len(rows)==318
(r/'native-head.json').write_text(json.dumps(dict(source='docs/audit/bf144/446f/restored.native.json.gz',scope='Default third-person ViewDirection, no aiming or mode commands, no relative base rotation; unscaled delta is unused by this branch',rows=rows),indent=2)+'\n')
print('Extracted typed native Head inputs, previous states and outputs for318 positive updates',flush=True)
