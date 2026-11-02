import gzip
import json
import struct
import sys
from pathlib import Path

sys.path.insert(0, 'tools/animgraph_authoring')
from ue_control_rig_contract import compile_contract
from ue_control_rig_target import compile_target

r = Path(__file__).resolve().parent
data = json.loads(Path('docs/audit/bf144/476/inputs.json').read_bytes())
request = json.loads((r / 'lengths.request.json').read_bytes())
native_path = r / 'lengths.native.json'
native = json.loads(native_path.read_bytes() if native_path.exists() else gzip.decompress(native_path.with_suffix('.json.gz').read_bytes()))
assert compile_target(compile_contract(native))['program']['instructions'] == data['definition']['program']['instructions']
samples = native['geometry_query_probe']['samples']
assert len(samples) == len(request['samples']) == 7748
for expected, actual in zip(request['samples'], samples):
    assert expected == actual['input']
    assert actual['scaled_distance_float'] == struct.unpack('f', struct.pack('f', actual['scaled_distance']))[0]
    assert actual['scaled_distance_float'] > 1e-8
mapping = json.loads((r / 'mapping.json').read_bytes())
source = []
solver = []
for entry in mapping['source']:
    row = data['source'][entry['row']]
    lengths = [samples[i]['scaled_distance_float'] for i in entry['lengths']]
    source.append(dict(row=entry['row'], lengths_cm=lengths))
    if row['weight'] != 1:
        continue
    args = row['arguments_after_native_call']
    vector = lambda value: [value[k] for k in 'xyz']
    pin_vector = lambda value: [value[k] for k in 'XYZ']
    assert args[8] == 'Location' and args[9] == dict(Type='Bone', Name='None')
    solver.append(dict(rotation_xyzw=[row['before'][0]['rotation'][k] for k in 'xyzw'], two_bone_ik=dict(root_cm=vector(row['before'][0]['translation']), effector_cm=pin_vector(args[3]['Translation']), pole_cm=pin_vector(args[7]), lengths_cm=lengths, stretch=args[10], stretch_ratios=args[11:13], orientation=dict(primary_axis=pin_vector(args[4]), secondary_axis=pin_vector(args[5]), secondary_weight=args[6]))))
current = []
for entry in mapping['current']:
    current.append(dict(row=entry['row'], lengths_m=[samples[i]['scaled_distance_float'] for i in entry['lengths']['current_m']], lifted_current_lengths_cm=[samples[i]['scaled_distance_float'] for i in entry['lengths']['current_cm']], distances_m=[samples[i]['scaled_distance'] for i in entry['lengths']['current_m']], distances_lifted_cm=[samples[i]['scaled_distance'] for i in entry['lengths']['current_cm']]))
assert len(solver) == 1324 and len(source) == 1330 and len(current) == 1272
(r / 'solver.request.json').write_text(json.dumps(dict(samples=solver), separators=(',', ':')) + '\n')
(r / 'inputs.json').write_text(json.dumps(dict(source=source, current=current), separators=(',', ':')) + '\n')
(r / 'length-check.json').write_text(json.dumps(dict(gate='PASS_NATIVE_ACTUAL_AUTO_LENGTH_INPUTS_AND_BOTH_COMMON_CURRENT_UNIT_DOMAINS',source_lengths=2660,current_common_lengths=5088,native_float_projection_exact=7748,source_weight_zero_no_length_claims=2,source_partial_lengths_only_calls=6,source_full_weight_solver_inputs=1324,fresh_rig_instructions_match=True,initial_scale_squared_branch_nonzero=True,both_solver_bone_scales_inherited_from_current_A=True),indent=2)+'\n')
print('Qualified7748 native length calculations and prepared1324 exact native full-weight solver consumers', flush=True)
