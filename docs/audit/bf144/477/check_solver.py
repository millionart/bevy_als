import gzip
import json
import sys
from pathlib import Path

sys.path.insert(0, 'tools/animgraph_authoring')
from ue_control_rig_contract import compile_contract
from ue_control_rig_target import compile_target

r = Path(__file__).resolve().parent
data = json.loads(Path('docs/audit/bf144/476/inputs.json').read_bytes())
inputs = json.loads((r / 'inputs.json').read_bytes())
request = json.loads((r / 'solver.request.json').read_bytes())
native_path = r / 'solver.native.json'
native = json.loads(native_path.read_bytes() if native_path.exists() else gzip.decompress(native_path.with_suffix('.json.gz').read_bytes()))
assert compile_target(compile_contract(native))['program']['instructions'] == data['definition']['program']['instructions']
samples = native['geometry_query_probe']['samples']
assert len(samples) == len(request['samples']) == 1324
source = [entry for entry in inputs['source'] if data['source'][entry['row']]['weight'] == 1]
assert len(source) == 1324
checks = []
for entry, expected, sample in zip(source, request['samples'], samples):
    assert expected == sample['input']
    row = data['source'][entry['row']]
    value = sample['two_bone_ik']
    assert [value['length_a_cm'], value['length_b_cm']] == entry['lengths_cm']
    fields = dict(joint_cm=row['after'][1]['translation'], effector_cm=row['after'][2]['translation'], root_rotation=row['after'][0]['rotation'], joint_rotation=row['after'][1]['rotation'])
    matches = {key: value[key] == target for key, target in fields.items()}
    checks.append(dict(dataset=row['dataset'], case=row['case'], query=row['query'], pc=row['pc'], lengths_cm=entry['lengths_cm'], matches=matches, expected=fields, actual={key:value[key] for key in fields}))
exact = sum(all(row['matches'].values()) for row in checks)
report = dict(gate='NATIVE_AUTO_LENGTH_COMPLETE_FULL_WEIGHT_SOLVER_CONSUMPTION', calls=len(checks), exact=exact, zero_weight_not_solved=2, partial_length_only=6, checks=checks)
(r / 'solver-check.json').write_text(json.dumps(report, separators=(',', ':')) + '\n')
assert exact == 1324, 'Native independent automatic lengths must preserve exact source full-weight A/B rotation and B/C position'
print('Qualified1324 native full-weight solver consumers with exact independent lengths, both A/B rotations and B/C positions', flush=True)
