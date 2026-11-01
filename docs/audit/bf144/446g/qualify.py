import json
import struct
from pathlib import Path

r=Path(__file__).resolve().parent
f32=lambda v:struct.unpack('<f',struct.pack('<f',v))[0]
data=json.loads((r/'runtime-restored/parameters.json').read_text())
motor={v['app_frame']:v for v in data['physics_motors']}
coeff={v['app_frame']:v for v in data['physics_coefficients']}
first=data['animation_before'][0]['curve_frame_input']['app_frame']
m=motor[first];c=coeff[first]
assert m['toggle']=='SenseOnly'and m['gravity_scale']==0
assert m['motor_acceleration'][1]!=0 and c['integration']['linear_increment']==[0,0,0]
step=f32(c['substep_delta_ns']*1e-9)
wrong_applied=f32(m['motor_acceleration'][1]*step)
assert wrong_applied!=c['integration']['linear_increment'][1]
inactive=[(row['curve_frame_input']['app_frame'],coeff[row['curve_frame_input']['app_frame']])for row in data['animation_before']if coeff[row['curve_frame_input']['app_frame']]['body']is None]
assert len(inactive)==1 and inactive[0][1]['sleeping']
result=dict(gate='PASS_ACTUAL_MOTOR_APPLICATION_AND_ABSENT_SOLVER_NEGATIVE_BOUNDARIES',sense_only_first_frame=first,generated_motor_y=m['motor_acceleration'][1],actual_applied_increment_y=0,wrong_enabled_assumption_increment_y=wrong_applied,sleeping_absent_solver_frames=[v[0]for v in inactive],no_zero_solver_fabricated=True)
(r/'qualification-check.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result),flush=True)
