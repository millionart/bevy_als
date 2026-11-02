import gzip
import json
from pathlib import Path

r=Path(__file__).resolve().parent
data=json.loads(Path('docs/audit/bf144/476/inputs.json').read_bytes())
lengths=json.loads(Path('docs/audit/bf144/477/inputs.json').read_bytes())
source_request=json.loads(Path('docs/audit/bf144/477/solver.request.json').read_bytes())
source_samples=json.loads(gzip.decompress(Path('docs/audit/bf144/477/solver.native.json.gz').read_bytes()))['geometry_query_probe']['samples']
source=[entry for entry in lengths['source']if data['source'][entry['row']]['weight']==1]
assert len(source)==len(source_request['samples'])==len(source_samples)==1324
rows=[]
samples=[]
for entry,request,prior in zip(source,source_request['samples'],source_samples):
    assert request==prior['input']
    ik=request['two_bone_ik']
    assert not ik['stretch']
    rows.append(dict(domain='source_cm',row=entry['row'],root=ik['root_cm'],effector=ik['effector_cm'],pole=ik['pole_cm'],lengths=ik['lengths_cm'],expected_source_positions=[prior['two_bone_ik']['joint_cm'],prior['two_bone_ik']['effector_cm']]))
    samples.append(request)
for entry in lengths['current']:
    row=data['current'][entry['row']]
    args=row['arguments_before']
    assert args[10]==dict(Bool=False)and args[13]==dict(F32=1.0)
    project=lambda v:[v[0],v[2],v[1]]
    quaternion=lambda v:[-v[0],-v[2],-v[1],v[3]]
    root=project(row['bones_before'][0][1]['translation'])
    effector=project(args[3]['List'][1]['Vector'])
    pole=project(args[7]['Vector'])
    ik=dict(root_cm=root,effector_cm=effector,pole_cm=pole,lengths_cm=entry['lengths_m'],stretch=False,stretch_ratios=[args[11]['F32'],args[12]['F32']])
    samples.append(dict(rotation_xyzw=quaternion(row['bones_before'][0][1]['rotation']),two_bone_ik=ik))
    rows.append(dict(domain='current_m',row=entry['row'],root=root,effector=effector,pole=pole,lengths=entry['lengths_m']))
assert len(rows)==2596
(r/'request.request.json').write_text(json.dumps(dict(samples=samples),separators=(',',':'))+'\n')
(r/'inputs.json').write_text(json.dumps(dict(rows=rows),separators=(',',':'))+'\n')
print('Prepared1324 source centimetre and1272 actual current metre-numeric position inputs with unchanged float lengths',flush=True)
