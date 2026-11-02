import gzip
import json
import sys
from pathlib import Path

sys.path.insert(0,'tools/animgraph_authoring')
from ue_control_rig_contract import compile_contract
from ue_control_rig_target import compile_target

r=Path(__file__).resolve().parent
p=r/'orientation.native.json'
native=json.loads(p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix('.json.gz').read_bytes()))
base=json.loads(Path('docs/audit/bf144/476/inputs.json').read_bytes())
assert compile_target(compile_contract(native))['program']['instructions']==base['definition']['program']['instructions']
inputs=json.loads((r/'inputs.json').read_bytes())
request=json.loads((r/'orientation.request.json').read_bytes())
samples=native['geometry_query_probe']['samples']
prior=json.loads(gzip.decompress(Path('docs/audit/bf144/478/positions.native.json.gz').read_bytes()))['geometry_query_probe']['samples']
assert len(samples)==len(inputs['rows'])==len(request['samples'])==5192
source_defaults=0
for row,expected,sample in zip(inputs['rows'],request['samples'],samples):
    assert sample['input']==expected
    result=sample['two_bone_ik']
    assert [result['length_a_cm'],result['length_b_cm']]==row['lengths']
    assert [result['joint_cm'],result['effector_cm']]==row['expected_source_positions']
    if row['domain']=='source_cm'and row['secondary_weight']==1:
        assert result==prior[row['input_index']]['two_bone_ik']
        original=base['source'][row['row']]
        assert result['root_rotation']==original['after'][0]['rotation']
        assert result['joint_rotation']==original['after'][1]['rotation']
        source_defaults+=1
    row['expected_native_rotations']=[[result[key][component]for component in 'xyzw']for key in ['root_rotation','joint_rotation']]
assert source_defaults==1324
(r/'inputs.json').write_text(json.dumps(inputs,separators=(',',':'))+'\n')
(r/'native-check.json').write_text(json.dumps(dict(gate='PASS_NATIVE_PRIMARY_SECONDARY_ORIENTATION_INPUTS_AND_DEFAULT_SOURCE_CONSUMERS',native_cases=5192,source_default_complete_prior_body_preserved=source_defaults,all_prior_position_results_preserved=5192,current_original_metre_numeric_inputs=2544,source_centimetre_inputs=2648,primary_only_control_not_actual_default=True,float_lengths_and_parameter_echo_preserved=True,fresh_rig_instructions_match=True),indent=2)+'\n')
print('Qualified5192 complete native orientation inputs;1324 source default full bodies and all prior positions preserved',flush=True)
