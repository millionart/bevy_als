import ast
import gzip
import hashlib
import json
import math
import sys
from pathlib import Path
r=Path(__file__).resolve().parent
sys.path.insert(0,str(Path('tools/tests').resolve()))
import test_character_movement_probe as movement
import test_vector_curve_probe as vector
from source_model import f,gait,rich
read=lambda name:json.loads((r/name).read_text())
curves=read('curves.native.json');rows=curves['blend_profile_probe']['samples'];schema=vector.validate(curves,read('curves.request.json'));assert len(rows)==3
for axis,row in enumerate(rows):
    assert row['custom_curve_axis']==axis
    assert row['custom_curve_pre_extrap']==row['custom_curve_post_extrap']==4
    for value in row['custom_curve_time_samples']:assert rich(row['custom_curve_keys'],value['time'])==value['value'],(axis,value)
print('Source model exact to all81 fresh native CurveVector samples',flush=True)
compressed=(r/'character.native.json.gz').read_bytes();raw=gzip.decompress(compressed);storage=read('storage-manifest.json')['source_character'];assert hashlib.sha256(raw).hexdigest()==storage['raw_sha256'];assert hashlib.sha256(compressed).hexdigest()==storage['compressed_sha256']
character=json.loads(raw);request=read('character.request.json');continuity=movement.validate(character,request);case=character['character_animation_probe']['cases'][0];assert case['initial_engine_frame']==100 and len(case['frames'])==240
compact=[]
for i,(frame,step)in enumerate(zip(case['frames'],request['cases'][0]['frames'])):
    before=frame['movement_before_tick'];after=frame['movement_after_tick'];assert before['movement_mode_name']==after['movement_mode_name']=='MOVE_Walking'
    v=before['velocity_cm_s'];assert v['z']==0;speed=f(math.sqrt(v['x']*v['x']+v['y']*v['y']));amount=gait(speed);dynamics=[rich(row['custom_curve_keys'],amount)for row in rows]
    state=next(p for p in frame['source_state_after_tick']if p['name']=='LocomotionState');values={p['name']:p.get('numeric_value')for p in state['fields']};assert dynamics[0]==values['MaxAcceleration']==after['max_acceleration_cm_s2'];assert dynamics[1]==values['MaxBrakingDeceleration'];assert before['max_acceleration_cm_s2']==2000
    expected={k:step['movement_input_world'][k]*before['max_acceleration_cm_s2']for k in ['x','y','z']};assert after['acceleration_cm_s2']==expected
    compact.append({'frame':i,'before_velocity_cm_s':v,'after_velocity_cm_s':after['velocity_cm_s'],'gait_amount':amount,'curve_values_cm_s2_and_friction':dynamics,'cached_max_acceleration_cm_s2':before['max_acceleration_cm_s2'],'applied_acceleration_cm_s2':after['acceleration_cm_s2'],'native_anim_max_acceleration_cm_s2':values['MaxAcceleration'],'native_anim_braking_cm_s2':values['MaxBrakingDeceleration']})
print('New source240 native parameter snapshots and cached acceleration exact; all481 movement snapshots continuous',flush=True)
source=Path('tools/export/fit_ground_dynamics_curves.py').read_text();tree=ast.parse(source);nodes=[n for n in tree.body if isinstance(n,ast.FunctionDef)and n.name in ['basis','solve_tangents']];assert len(nodes)==2;scope={};exec(compile(ast.Module(body=nodes,type_ignores=[]),'original-fit-functions','exec'),scope)
sample={x['time']:x['value']for x in rows[0]['custom_curve_time_samples']};fit=scope['solve_tangents'](sample[2],sample[3],sample[2.25],sample[2.75]);native=rows[0]['custom_curve_keys'][2]['leave_tangent'];assert f(fit[0]/100)==f(-36.423724)!=f(native/100)
report={'gate':'PASS_NEW_SOURCE240_PARAMETERS_NATIVE_CURVES_FIT_AND_CONTINUITY','native_curve_samples_exact':81,'native_parameter_frames_exact':240,'movement_schema':continuity,'vector_schema':schema,'schema_self_tests':{'movement':movement.self_test(),'vector':vector.self_test()},'fit':{'native_leave_tangent_cm':native,'native_leave_tangent_si_f32':f(native/100),'fitted_from_fullprecision_native_samples_cm':fit,'fitted_leave_tangent_si_f32':f(fit[0]/100),'target_literal_si_f32':f(-36.423724)},'source_keys':[x['custom_curve_keys']for x in rows],'parameter_frames':compact,'source_runtime':'new controlled diagnostic source world; not a recovery of the deleted prior Root or target240 traces','old12018_identity_files':'missing','screenshots':0,'performance_acceptance':'paused'}
(r/'source-parameters.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({k:v for k,v in report.items()if k not in ['parameter_frames','source_keys']}),flush=True)
