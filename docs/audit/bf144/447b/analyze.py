import gzip
import json
import sys
from pathlib import Path

r=Path(__file__).resolve().parent
label=sys.argv[1]
target=json.loads((r/f'runtime-{label}/parameters.json').read_text())
source=json.loads(gzip.decompress(Path('docs/audit/bf144/446f/restored.native.json.gz').read_bytes()))['character_animation_probe']['cases'][0]
query=79;frame=78
before=target['animation_before'][query]['head_values']
after=target['animation_after'][query]['head_values']
fields=source['frames'][frame]['source_state_after_tick']
def numbers(name):return {v['name']:v['numeric_value']for v in next(v for v in fields if v['name']==name)['fields']if 'numeric_value'in v}
view=numbers('ViewState');head=numbers('HeadState')
target_view=before['floats']['view_state.yaw_angle']['resolved']
target_amount=after['floats']['head_state.yaw_amount']['resolved']
pins={v['key']:v['value']for v in after['actual_look_pins']}
assert target_view==44.999996185302734 and view['YawAngle']==45
assert target_amount==pins['head_state.yaw_amount']
assert target_amount!=head['YawAmount']
result=dict(gate='PASS_ACTUAL_HEAD_INPUT_STATE_AND_LOOK_CONSUMER_OBSERVATION',source_frame=frame,target_query=query,target_ordinal=0,
    source_view_yaw_angle=view['YawAngle'],target_view_yaw_angle=target_view,source_head_state=head,
    target_head_state={key:v['resolved']for key,v in after['floats'].items()if key.startswith('head_state.')},
    actual_look_pins=pins,input_values_bitwise_equal=False,kernel_cause_confirmed=False,
    source_settings_yaw_half_life=0.10000000149011612,initial_target_state_missing_before_first_query=True,
    common_input_head_kernel_replay_completed=False,new_differences=[],total_confirmed_differences=271)
(r/f'analysis-{label}.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result),flush=True)
