import importlib.util
import json
import math
from pathlib import Path
r=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('source_model',Path('docs/audit/bf144/441b/source_model.py'));m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
actors=json.loads((r/'kernel-inputs.json').read_text());kernel=json.loads((r/'kernel-speeds.json').read_text())['rows'];samples=[];table=[]
def bounds(value,scale):
    walk,run,sprint=175/scale,375/scale,650/scale
    return([run,sprint],[2,3])if value>run else([walk,run],[1,2])if value>walk else([0,walk],[0,1])
for actor,k in zip(actors,kernel):
    x,y,z=actor['relative_velocity_consumed'];assert y==0
    cm=[x*100,-z*100,0];speed=m.f(math.sqrt(cm[0]*cm[0]+cm[1]*cm[1]));projected=m.f(speed/100);direct=k['meter_speed'];table.append({'ordinal':actor['ordinal'],'cm_float_speed_model':speed,'projected_meter_float_speed':projected,'direct_meter_float_speed':direct,'actual_target_gait':actor['gait_amount']})
    for label,value,scale,point in [('cm',speed,1,cm),('projected',projected,100,None),('direct',direct,100,None)]:
        a,b=bounds(value,scale);v={'name':f'{label}-{actor["ordinal"]}','rotation_xyzw':[0,0,0,1],'float_range_map':{'input':value,'input_range':a,'output_range':b}}
        if point is not None:v['scaled_distance']={'a':point,'b':[0,0,0],'scale':[1,1,1]}
        samples.append(v)
(r/'mapping.request.json').write_text(json.dumps({'samples':samples},separators=(',',':'))+'\n');(r/'mapping-inputs.json').write_text(json.dumps(table,separators=(',',':'))+'\n');print('Prepared720 native range-map controls and240 native CM-float lengths from real consumed vectors',flush=True)
