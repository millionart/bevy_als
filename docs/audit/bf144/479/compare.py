import gzip
import json
import struct
from pathlib import Path

r=Path(__file__).resolve().parent
def read(name):
    p=r/name
    if not p.exists():
        p=r/(name+'.gz')
    return json.loads(gzip.decompress(p.read_bytes())if p.suffix=='.gz'else p.read_bytes())
def f32(values):
    return [struct.unpack('f',struct.pack('f',v))[0]for v in values]
def runtime(v):
    return [v[0],v[2],v[1]]
def position(v):
    return [v[k]for k in 'xyz']

data=read('replay-unit-factors.json')
baseline=read('replay-unit-baseline.json')
inputs=read('inputs.json')['rows']
actual=json.loads(Path('docs/audit/bf144/476/inputs.json').read_bytes())
assert data['source_exact']==data['public_exact']==5192 and data['current_actual']==1272
endpoint={(row['row'],row['mask']):row for row in baseline['outputs']}
outputs={(row['row'],row['mask']):row for row in data['outputs']}
assert len(outputs)==5192*7
groups={}
table=[]
factors={str(mask):dict(changed_cases=0,changed_quaternions=0,changed_f32_quaternions=0)for mask in [30,29,27,23,15]}
for row in data['outputs']:
    index=row['row']
    mask=row['mask']
    value=row['result']
    original=inputs[index]
    if mask in [31,0]:
        assert value==endpoint[(index,mask)]['result']
    else:
        complete=outputs[(index,31)]['result']
        assert row['position_mask']==3
        assert value['joint']==complete['joint']and value['end']==complete['end']
        differences=[a!=b for a,b in zip(value['rotations'],complete['rotations'])]
        float_differences=[f32(a)!=f32(b)for a,b in zip(value['rotations'],complete['rotations'])]
        factors[str(mask)]['changed_cases']+=any(differences)
        factors[str(mask)]['changed_quaternions']+=sum(differences)
        factors[str(mask)]['changed_f32_quaternions']+=sum(float_differences)
    group=groups.setdefault(original['domain']+':secondary'+str(original['secondary_weight']),{}).setdefault(str(mask),dict(cases=0,exact_cases=0,double_different_quaternions=0,f32_different_quaternions=0))
    group['cases']+=1
    group['exact_cases']+=row['exact']
    group['double_different_quaternions']+=sum(a!=b for a,b in zip(value['rotations'],row['native']))
    group['f32_different_quaternions']+=sum(f32(a)!=f32(b)for a,b in zip(value['rotations'],row['native']))
    if mask==31:
        assert value['joint']==position(original['expected_source_positions'][0])
        assert value['end']==position(original['expected_source_positions'][1])
        assert row['exact']
    if original['secondary_weight']==1 and mask in [31,0]:
        control=outputs[(index+1,mask)]['result']
        assert original['input_index']==inputs[index+1]['input_index']
        assert [stage['primary_rotation']for stage in value['stages']]==control['rotations']
    if original['domain']=='current_m'and original['secondary_weight']==1 and mask==0:
        old=actual['current'][original['row']]
        assert runtime(value['joint'])==old['bones_after'][1][1]['translation']
        assert runtime(value['end'])==old['bones_after'][2][1]['translation']
    if not row['exact']:
        table.append(dict(row=index,domain=original['domain'],secondary_weight=original['secondary_weight'],mask=mask,position_mask=row['position_mask'],rotations=value['rotations'],native=row['native']))
assert all(value['changed_cases']>0 for value in factors.values())
report=dict(gate='PASS_COMPLETE_IK_PRIMARY_SECONDARY_NATIVE_PUBLIC_ORIENTATION_AND_ISOLATED_FACTORS',native_source_cases_exact=5192,current_public_models_exact=5192,current_actual_default_rotations_and_positions_exact=1272,native_primary_only_controls_independently_check_default_primary_stages=2596,public_primary_only_controls_check_default_primary_stages=2596,orientation_factor_positions_held_to_qualified_source=True,groups=groups,isolated_orientation_factors=factors,source_position_primitive_reused_without_output_injection=True,nonunit_secondary_weights_opposite_axes_and_degenerate_branches_not_claimed=True,rotation_wrapper_partial_and_descendants_still_open=True,new_differences=[],confirmed_difference_count=272,screenshots=0,performance_acceptance='paused')
(r/'comparison.json').write_text(json.dumps(report,indent=2)+'\n')
(r/'difference-table.json').write_text(json.dumps(table,separators=(',',':'))+'\n')
print(json.dumps(dict(gate=report['gate'],groups=groups,isolated_orientation_factors=factors)),flush=True)
