import copy
import json
import math
import sys
from pathlib import Path

r=Path(__file__).resolve().parent
INPUTS=['animation.delta_seconds','animation.delta_seconds_without_time_dilation','locomotion_state.input_yaw_angle_world_space','locomotion_state.target_yaw_angle_world_space','locomotion_state.rotation_yaw','locomotion_state.yaw_velocity_world_space','view_state.yaw_angle','view_state.pitch_angle','movement_base.delta_yaw']
STATES=['head_state.pitch_angle','head_state.yaw_angle','head_state.yaw_velocity','head_state.yaw_amount']
BOOLS=['view_mode.first_person','rotation_mode.velocity_direction','locomotion_state.b_has_input','movement_base.b_has_relative_rotation']


def validate(data):
    before=data['animation_before'];after=data['animation_after']
    assert len(before)==len(after)==319
    for a,b in zip(before,after,strict=True):
        x=a['head_values'];y=b['head_values']
        for key in INPUTS:
            assert math.isfinite(x['floats'][key]['resolved'])
        for key in BOOLS:assert isinstance(x['booleans'][key]['resolved'],bool)
        for key in STATES:assert math.isfinite(y['floats'][key]['resolved'])
        pins={v['key']:v['value']for v in y['actual_look_pins']}
        assert pins['head_state.yaw_amount']==y['floats']['head_state.yaw_amount']['resolved']
        assert pins['head_state.pitch_angle']==y['floats']['head_state.pitch_angle']['resolved']
    return dict(rows_per_phase=319,head_input_output_rows=638,actual_look_pin_rows=319)


if __name__=='__main__':
    label=sys.argv[1]
    if label=='self-test':
        head=dict(floats={key:dict(resolved=0.0)for key in INPUTS+STATES},booleans={key:dict(resolved=False)for key in BOOLS},actual_look_pins=[dict(key='head_state.yaw_amount',value=0.0),dict(key='head_state.pitch_angle',value=0.0)])
        data=dict(animation_before=[dict(head_values=copy.deepcopy(head))for _ in range(319)],animation_after=[dict(head_values=copy.deepcopy(head))for _ in range(319)])
        validate(data)
        for kind in range(4):
            bad=copy.deepcopy(data)
            if kind==0:del bad['animation_before'][0]['head_values']
            elif kind==1:bad['animation_before'][0]['head_values']['floats']['view_state.yaw_angle']['resolved']=float('nan')
            elif kind==2:bad['animation_after'][0]['head_values']['actual_look_pins']=[]
            else:bad['animation_after'][0]['head_values']['actual_look_pins'][0]['value']=1
            try:validate(bad)
            except(AssertionError,KeyError):continue
            raise AssertionError(kind)
        result=dict(rejected=4)
    else:result=validate(json.loads((r/f'runtime-{label}/parameters.json').read_text()))
    (r/f'{label}-check.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result),flush=True)
