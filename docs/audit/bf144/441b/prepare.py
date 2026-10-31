import copy
import json
import struct
from pathlib import Path
r=Path(__file__).resolve().parent
f=lambda x:struct.unpack('f',struct.pack('f',x))[0]
frames=[]
for i in range(240):
    frames.append({'delta_seconds':f(1/30),'rendered':True,'control_rotation_degrees':{'pitch':0,'yaw':0,'roll':0},'desired_gait':'Als.Gait.Running','desired_stance':'Als.Stance.Standing','movement_input_world':{'x':int(20<=i<60 or 120<=i<160),'y':0,'z':0}})
request={'gravity_cm_s2':-980,'initial_engine_frame':100,'cases':[{'name':'recovery-independent-running-240','local_controller':True,'dynamic_base':False,'spawn_height_cm':92.15,'frames':frames}]}
(r/'character.request.json').write_text(json.dumps(request,separators=(',',':'))+'\n')
base=json.loads(Path('tools/tests/fixtures/vector_curve.request.json').read_text())
times=sorted({i/8 for i in range(25)}|{1.1131258010864258,1.7916673421859741})
for row in base['samples']:row['custom_curve_times']=times
(r/'curves.request.json').write_text(json.dumps(base,indent=2)+'\n')
print('Prepared fresh source Running240 and three axes at27 common times; engine frame100 and diagnostic gravity-980 explicit',flush=True)
