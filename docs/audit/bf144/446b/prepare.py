import copy
import hashlib
import json
import math
from pathlib import Path

r=Path(__file__).resolve().parent
reader='tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp'
(r/'reader-before-sha256.json').write_text(json.dumps(dict(sha256=hashlib.sha256(Path(reader).read_bytes()).hexdigest()),indent=2)+'\n')
data=json.loads(Path('docs/audit/bf144/446/runtime-restored/parameters.json').read_text())
rows=data['animation_before']
assert len(rows)==319 and rows[0]['arena_frame']==0
state=rows[0]['history_input']
q=state['local_rotation']
forward=[-2*(q[0]*q[2]+q[3]*q[1]),-1+2*(q[0]*q[0]+q[1]*q[1])]
yaw=math.degrees(math.atan2(forward[1],forward[0]))
position=state['actor_world_translation']
frames=[]
for row in rows[1:]:
    h=row['history_input']
    v=h['desired_world_motion']
    frames.append(dict(delta_seconds=row['frame_delta'],rendered=True,control_rotation_degrees=dict(pitch=0,yaw=-math.degrees(h['raw_view_yaw'])-90,roll=0),desired_gait=h['desired_gait'],desired_stance=h['desired_stance'],movement_input_world=dict(x=v[0],y=v[2],z=v[1])))
assert len(frames)==318
case=dict(name='actual-positive-history318-with-initial-pose',dynamic_base=False,local_controller=True,spawn_height_cm=position[1]*100,spawn_xy_cm=[position[0]*100,position[2]*100],spawn_yaw_degrees=yaw,frames=frames)
control=dict(name='far-shifted-floor-and-quarter-turn',dynamic_base=False,local_controller=True,spawn_height_cm=92.15,spawn_xy_cm=[4200,-3100],spawn_yaw_degrees=90,frames=[dict(delta_seconds=0.03333333507180214,rendered=True)for _ in range(2)])
base=dict(gravity_cm_s2=-981,initial_engine_frame=100,capture_asset_player_ticks=True,cases=[case,control])
(r/'request.json').write_text(json.dumps(base,indent=2)+'\n')
(r/'pose-control.request.json').write_text(json.dumps(dict(base,cases=[control]),indent=2)+'\n')
for name,change in [('over-limit',None),('bad-xy',None),('bad-yaw',None)]:
    bad=copy.deepcopy(base)
    if name=='over-limit':bad['cases'][0]['frames'].append(copy.deepcopy(frames[-1]))
    elif name=='bad-xy':bad['cases'][0]['spawn_xy_cm']=[0]
    else:bad['cases'][0]['spawn_yaw_degrees']='90'
    (r/(name+'.request.json')).write_text(json.dumps(bad,indent=2)+'\n')
(r/'legacy.request.json').write_bytes(Path('docs/audit/bf144/445a/legacy.request.json').read_bytes())
(r/'input-provenance.json').write_text(json.dumps(dict(target_report='docs/audit/bf144/446/runtime-restored/parameters.json',zero_initialization_query_excluded=True,positive_frames=318,source_point_cm=[position[0]*100,position[2]*100,position[1]*100],source_yaw_degrees_from_stored_forward=yaw,coordinate_basis='source X=runtime X; source Y=runtime Z; source Z=runtime Y; directions have no length scaling',control_yaw_formula='source_degrees=-runtime_radians*180/pi-90',full_bitwise_quaternion_or_all_parameters_equivalence_claimed=False),indent=2)+'\n')
print('Prepared 318 positive source frames, nonzero pose and shifted-floor controls',flush=True)
