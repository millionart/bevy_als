import copy
import json
from pathlib import Path

r=Path(__file__).resolve().parent
request=json.loads(Path('docs/audit/bf144/460/request.json').read_text())
rows=json.loads(Path('docs/audit/bf144/461/inputs.json').read_text())['rows'];args={(x['query'],x['pc']):x['source_arguments_raw']for x in rows}
for side,case in enumerate(request['cases']):
 pc=[31,40][side]
 for query,frame in enumerate(case['frames']):
  values=args[(query if query else 1,pc)];foot=values[0]
  frame['reference_ray_start_cm']=[foot[0],foot[1],values[2]]
  frame['reference_ray_end_cm']=[foot[0],foot[1],-values[3]]
  frame['reference_ray_vm_space']=True
(r/'request.json').write_text(json.dumps(request,separators=(',',':'))+'\n')
for name,field,value in [('bad-type','reference_ray_vm_space','true'),('bad-parent','mesh_to_world',None)]:
 fixture=copy.deepcopy(request);fixture['cases']=fixture['cases'][:1];fixture['cases'][0]['frames']=fixture['cases'][0]['frames'][:1];frame=fixture['cases'][0]['frames'][0]
 if value is None:frame.pop(field)
 else:frame[field]=value
 (r/(name+'.request.json')).write_text(json.dumps(fixture,separators=(',',':'))+'\n')
print('Prepared636 actual source local rays plus two initialization references and strict protocol controls',flush=True)
