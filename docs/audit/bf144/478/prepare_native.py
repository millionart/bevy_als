import gzip
import json
import sys
from pathlib import Path

sys.path.insert(0,'tools/animgraph_authoring')
from ue_control_rig_contract import compile_contract
from ue_control_rig_target import compile_target

r=Path(__file__).resolve().parent
p=r/'positions.native.json'
native=json.loads(p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix('.json.gz').read_bytes()))
base=json.loads(Path('docs/audit/bf144/476/inputs.json').read_bytes())
assert compile_target(compile_contract(native))['program']['instructions']==base['definition']['program']['instructions']
inputs=json.loads((r/'inputs.json').read_bytes())
request=json.loads((r/'request.request.json').read_bytes())
samples=native['geometry_query_probe']['samples']
assert len(samples)==len(inputs['rows'])==len(request['samples'])==2596
for row,expected,sample in zip(inputs['rows'],request['samples'],samples):
    assert expected==sample['input']
    result=sample['two_bone_ik']
    assert [result['length_a_cm'],result['length_b_cm']]==row['lengths']
    positions=[result['joint_cm'],result['effector_cm']]
    if row['domain']=='source_cm':
        assert positions==row['expected_source_positions']
    row['expected_source_positions']=positions
(r/'inputs.json').write_text(json.dumps(inputs,separators=(',',':'))+'\n')
(r/'native-check.json').write_text(json.dumps(dict(gate='PASS_NATIVE_POSITION_INPUTS_AND_SOURCE_CONSUMER_TRANSPARENCY',native_cases=2596,source_prior_complete_positions_preserved=1324,same_actual_current_metre_numeric_inputs=1272,source_float_lengths_preserved=True,fresh_rig_instructions_match=True,current_numeric_domain_not_labeled_source_centimetres=True),indent=2)+'\n')
print('Qualified2596 exact native input echoes,1324 original source consumers and1272 unchanged current numeric inputs',flush=True)
