import gzip
import hashlib
import json
import struct
from pathlib import Path

r=Path(__file__).resolve().parent
def raw(name):
    p=r/name
    if not p.exists():
        p=r/(name+'.gz')
    return gzip.decompress(p.read_bytes())if p.suffix=='.gz'else p.read_bytes()
def read(name):
    return json.loads(raw(name))
def project(value,domain):
    unit=0.01 if domain=='source_cm'else 1.0
    return [struct.unpack('f',struct.pack('f',value[i]*unit))[0]for i in [0,2,1]]

baseline=read('replay-unit-minimal.json')
assert baseline['source_exact']==[987,1230,1938,2596]
assert baseline['actual_current_public_exact']==1272
assert raw('replay-unit-baseline.json')==raw('replay-unit-simplified.json')==raw('replay-unit-restored.json')
original=read('replay-unit-baseline.json')
for row in original['outputs']:
    assert not row.pop('fully_extended')
assert original==baseline
assert raw('replay-unit-minimal.json')==raw('replay-unit-final-restored.json')
assert raw('replay.rs')==raw('replay.rs.final')
assert hashlib.sha256(raw('replay.rs')).hexdigest()==read('final-source.json')['sha256']
for label in ['baseline','simplified','restored','minimal','final-restored']:
    assert read('replay-compile-'+label+'.report.json')['success']
    assert read('replay-'+label+'.report.json')['success']
for label,expected in [('no-source-reciprocal',1230),('no-source-dot-order',1938),('no-source-reciprocal-minimal',1230)]:
    assert read('replay-compile-'+label+'.report.json')['success']
    assert read('replay-'+label+'.report.json')['process_exit_code']==101
    negative=read('replay-unit-'+label+'.json')
    assert negative['actual_current_public_exact']==1272
    assert negative['source_exact'][3]==expected
groups={}
table=[]
for row in baseline['outputs']:
    domain=row['domain']
    mask=row['mask']
    value=groups.setdefault(domain,{}).setdefault(str(mask),dict(cases=0,complete_exact=0,joint_double_differences=0,end_double_differences=0,joint_f32_differences=0,end_f32_differences=0,fully_extended=0))
    value['cases']+=1
    value['complete_exact']+=row['exact']
    differences={}
    for i,key in enumerate(['joint','end']):
        different=row[key]!=row['expected'][i]
        float_different=project(row[key],domain)!=project(row['expected'][i],domain)
        value[key+'_double_differences']+=different
        value[key+'_f32_differences']+=float_different
        differences[key]=dict(double=different,f32=float_different)
    if not row['exact']:
        table.append(dict(row=row['row'],domain=domain,mask=mask,differences=differences,joint=row['joint'],end=row['end'],expected=row['expected']))
assert groups['source_cm']['3']['complete_exact']==1324
assert groups['current_m']['3']['complete_exact']==1272
report=dict(gate='PASS_ACTUAL_IK_POSITION_NATIVE_PUBLIC_SOURCE_DOT_AND_RECIPROCAL_FACTORS',groups=groups,source_complete_native_cases=1324,current_same_numeric_native_cases=1272,current_public_actual_complete_positions=1272,source_and_current_endpoints_exact_with_source_arithmetic=True,common_float_lengths_and_inputs_preserved=True,source_position_model_current_mask_exact_to_public_in_both_domains=True,regular_cohort_no_goal_or_pole_degeneracy=True,stretch_disabled_all_actual_inputs=True,unused_plane_normal_normalization_not_added=True,geometry_adapter_and_duplicate_identity_removed_full_report_bytes_unchanged=True,unvisited_branches_and_constant_state_deleted_numeric_records_unchanged=True,both_arithmetic_factors_physically_removed_and_rejected=True,source_bytes_restored_same_target_recompiled_all_minimal_report_bytes_exact=True,existing_difference='D249',new_differences=[],confirmed_difference_count=272,rotation_partial_and_descendant_equivalence_claimed=False,screenshots=0,performance_acceptance='paused')
(r/'comparison.json').write_text(json.dumps(report,indent=2)+'\n')
(r/'difference-table.json').write_text(json.dumps(table,separators=(',',':'))+'\n')
print(json.dumps(dict(gate=report['gate'],groups=groups)),flush=True)
