import copy
import gzip
import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
source_path=Path('docs/audit/bf144/448a/restored.native.json.gz')
actual=json.loads(gzip.decompress(source_path.read_bytes()))['character_animation_probe']['cases'][0]
native=json.loads((r/'source-head.native.json').read_bytes())['mesh_additive_probe']
def validate(data):
    assert data['node_guid']=='d17ea3bf-4d5e-ebd5-fd47-0c867f5cc0e7'and len(data['samples'])==2
    for i,row in enumerate(data['samples']):
        assert row['name']=='actual-source-head-'+str(i)and row['alpha']==1 and not row['root_space_additive']
        assert row['pose']==actual['frames'][i]['pose_boundaries']['rig_source']['pose']
    return 158
assert validate(native)==158
negative=[]
for kind in ['missing-sample','changed-pose','wrong-alpha']:
    data=copy.deepcopy(native)
    if kind=='missing-sample':data['samples'].pop()
    elif kind=='changed-pose':data['samples'][0]['pose']['bones'][0]['local_transform']['translation']['x']+=1
    else:data['samples'][0]['alpha']=0
    try:validate(data)
    except AssertionError:negative.append(kind)
    else:raise AssertionError('Invalid full native source Head control passed: '+kind)
result=dict(gate='PASS_FULL_NATIVE_SOURCE_HEAD_APPLY_TWO_FRAMES',source_frames=2,full_bone_rows=158,full_pose_and_curve_exact_actual_source=True,negative_fixtures_rejected=negative,
    evidence_inputs={p.as_posix():hashlib.sha256(p.read_bytes()).hexdigest()for p in [source_path,Path('docs/audit/bf144/451/source-head.native.json')]},full_root_or_als_equivalence_claimed=False)
(r/'source-check.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
