import importlib.util
import json
import math
import struct
from pathlib import Path
r=Path(__file__).resolve().parent
f=lambda v:struct.unpack('f',struct.pack('f',v))[0]
spec=importlib.util.spec_from_file_location('basis',Path('tools/import/coordinate_basis.py'));b=importlib.util.module_from_spec(spec);spec.loader.exec_module(b)
assert b.source_direction_to_runtime([1,0,0])==[1,0,0]
assert b.source_direction_to_runtime([0,1,0])==[0,0,1]
assert b.source_direction_to_runtime([0,0,1])==[0,1,0]
axes=[[0,0],[0,1],[1,0],[1,1],[2,0],[f(.2),f(.3)],[f(.0002),f(-.0003)]]
angles=[0,15,30,45,60,90,135,180,-45,-60,-90]
samples=[]
for angle in angles:
    for i,axis in enumerate(axes):samples.append({'name':f'{angle}:{i}','rotation_xyzw':[0,0,0,1],'move_input':{'yaw_degrees':angle,'axes':axis}})
(r/'request.json').write_text(json.dumps({'samples':samples},indent=2)+'\n')
(r/'basis.json').write_text(json.dumps({'source_runtime_basis_columns':[[1,0,0],[0,0,1],[0,1,0]],'unitless_direction_scale':1,'source_forward_xy':['cos(theta)','sin(theta)',0],'runtime_mapped_source_forward':['cos(theta)',0,'sin(theta)'],'runtime_forward_yaw':['-sin(phi)',0,'-cos(phi)'],'physical_view_relation':'phi = -theta - pi/2','shared_operator_control':'runtime yaw=-source float radians; runtime input=(source Y,-source X); compares a physical rotation operator, not identical animation Forward/Right channels','source_input_axes_are_float32_values_widened_to_double':True},indent=2)+'\n')
print('Prepared77 source-native cases with complete handedness/forward-axis basis and identical input-axis precision',flush=True)
