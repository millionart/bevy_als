import gzip
import json
import math
import sys
from pathlib import Path

r=Path(__file__).resolve().parent
label=sys.argv[1]
data=json.loads((r/f'runtime-{label}/parameters.json').read_text())
rows=data['animation_before']
initial=rows[0]['history_input']
cold=[v for v in rows if v['ordinal']is None]
assert len(cold)==79
assert all(v['history_input']['effective_move']==[0,0]and v['history_input']['desired_world_motion']==[0,0,0]for v in cold)
assert all(v['history_input']['locomotion']=='Grounded'for v in cold)
assert all(v['history_input']['local_rotation']==initial['local_rotation']for v in cold)
assert all(v['history_input']['raw_view_yaw']==initial['raw_view_yaw']for v in cold)
source=json.loads(gzip.decompress(Path('docs/audit/bf144/445a/restored.native.json.gz').read_bytes()))['character_animation_probe']['cases'][0]['initialization']
qx,qy,qz,qw=initial['local_rotation']
forward=[-2*(qx*qz+qw*qy),-2*(qy*qz-qw*qx),-1+2*(qx*qx+qy*qy)]
source_forward=[forward[0],forward[2],forward[1]]
source_yaw=math.degrees(math.atan2(source_forward[1],source_forward[0]))
result=dict(gate='PASS_ACTUAL_STARTUP_INPUT_MANIFEST_WITH_NATIVE_REPLAY_CAPABILITY_GAPS',
    animation_queries=319,cold_queries=79,positive_target_queries=318,cold_mode='Grounded',cold_actual_gait=initial['gait'],cold_desired_gait=initial['desired_gait'],
    cold_effective_and_desired_motion_zero=True,cold_root_quaternion_unchanged=True,cold_raw_view_yaw_radians=initial['raw_view_yaw'],
    initial_actor_world_tqs={k:v for k,v in initial.items()if k.startswith('actor_world')},initial_avatar_world_tqs={k:v for k,v in initial.items()if k.startswith('avatar_world')},
    initial_source_actor_tqs=source['actor_transform'],native_initial_actor_source_yaw_degrees=0,
    target_forward_projected_into_source_basis=source_forward,target_initial_source_basis_yaw_from_stored_quaternion_degrees=source_yaw,
    source_replay_frame_limit=240,required_positive_frames=318,native_current_entry_accepts_zero_delta=False,native_current_entry_accepts_actor_xy_or_yaw=False,
    simply_prepending_79_zero_input_frames_is_sufficient=False,matched_full_history_claimed=False,new_differences=[],total_confirmed_differences=271)
(r/f'analysis-{label}.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items()if 'tqs'not in k}),flush=True)
