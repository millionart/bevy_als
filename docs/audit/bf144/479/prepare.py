import copy
import gzip
import json
from pathlib import Path

r=Path(__file__).resolve().parent
base=json.loads(Path('docs/audit/bf144/478/inputs.json').read_bytes())
requests=json.loads(Path('docs/audit/bf144/478/request.request.json').read_bytes())
prior=json.loads(gzip.decompress(Path('docs/audit/bf144/478/positions.native.json.gz').read_bytes()))['geometry_query_probe']['samples']
data=json.loads(Path('docs/audit/bf144/476/inputs.json').read_bytes())
assert len(base['rows'])==len(requests['samples'])==len(prior)==2596
rows=[]
samples=[]
for index,(row,request,old)in enumerate(zip(base['rows'],requests['samples'],prior)):
    assert old['input']==request
    if row['domain']=='source_cm':
        original=data['source'][row['row']]
        args=original['arguments_after_native_call']
        primary=[args[4][k]for k in 'XYZ']
        secondary=[args[5][k]for k in 'XYZ']
        assert request['two_bone_ik']['orientation']==dict(primary_axis=primary,secondary_axis=secondary,secondary_weight=1)
    else:
        original=data['current'][row['row']]
        args=original['arguments_before']
        project=lambda v:[v[0],v[2],v[1]]
        primary=project(args[4]['Vector'])
        secondary=project(args[5]['Vector'])
        assert args[6]==dict(F32=1.0)
    for weight in [1,0]:
        sample=copy.deepcopy(request)
        sample['two_bone_ik']['orientation']=dict(primary_axis=primary,secondary_axis=secondary,secondary_weight=weight)
        output=dict(row)
        output.update(input_index=index,rotation=sample['rotation_xyzw'],primary=primary,secondary=secondary,secondary_weight=weight)
        rows.append(output)
        samples.append(sample)
assert len(rows)==5192
(r/'inputs.json').write_text(json.dumps(dict(rows=rows),separators=(',',':'))+'\n')
(r/'orientation.request.json').write_text(json.dumps(dict(samples=samples),separators=(',',':'))+'\n')
print('Prepared5192 actual-default and primary-only orientation calls; same positions and float lengths retained',flush=True)
