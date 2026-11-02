import gzip
import json
import sys
from pathlib import Path

sys.path.insert(0,'tools/animgraph_authoring')
from ue_control_rig_contract import compile_contract
from ue_control_rig_target import compile_target

r=Path(__file__).resolve().parent
p=r/'core.native.json'
native=json.loads(p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix('.json.gz').read_bytes()))
base=json.loads(Path('docs/audit/bf144/476/inputs.json').read_bytes())
assert compile_target(compile_contract(native))['program']['instructions']==base['definition']['program']['instructions']
inputs=json.loads((r/'inputs.json').read_bytes())
request=json.loads((r/'core.request.json').read_bytes())
samples=native['geometry_query_probe']['samples']
assert len(samples)==len(request['samples'])==1330
full=0
partial=0
for row in inputs['source']:
    if row['weight']<=1e-8:
        assert 'core_sample'not in row
        continue
    index=row['core_sample']
    actual=samples[index]
    assert actual['input']==request['samples'][index]
    value=actual['two_bone_ik']
    assert [value['length_a_cm'],value['length_b_cm']]==row['lengths']
    vector=lambda v:[v[k]for k in 'xyz']
    row['expected_core']=dict(joint=vector(value['joint_cm']),end=vector(value['effector_cm']),rotations=[[value[key][k]for k in 'xyzw']for key in ['root_rotation','joint_rotation']])
    if row['weight']==1:
        assert row['expected_core']['rotations']==[t['rotation']for t in row['expected_after'][:2]]
        assert row['expected_core']['joint']==row['expected_after'][1]['translation']
        assert row['expected_core']['end']==row['expected_after'][2]['translation']
        full+=1
    else:
        partial+=1
assert full==1324 and partial==6
(r/'inputs.json').write_text(json.dumps(inputs,separators=(',',':'))+'\n')
(r/'native-check.json').write_text(json.dumps(dict(gate='PASS_FRESH_COMPLETE_PRE_WEIGHT_NATIVE_SOLVER_INPUTS',native_calls=1330,full_actual_post_preserved=1324,partial_core_inputs_qualified=6,weight_zero_no_solver_claim=2,fresh_rig_instructions_match=True),indent=2)+'\n')
print('Qualified fresh1330 native cores,1324 original full posts and6 exact partial pre-weight solver inputs',flush=True)
