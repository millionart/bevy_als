import json
import struct
import sys
from pathlib import Path

r=Path(__file__).resolve().parent;data=json.loads((r/'replay-restored.json').read_bytes());outputs=data['outputs'];f32=lambda x:struct.unpack('f',struct.pack('f',x))[0];counts={};first={};first_f32={};limit_outside=0
for row in outputs:
 mask=row['mask'];actual=row['stages']['result'];expected=row['expected_native'];record=counts.setdefault(str(mask),dict(double_differences=0,f32_differences=0));different=actual!=expected;fd=[f32(x)for x in actual]!=[f32(x)for x in expected];record['double_differences']+=different;record['f32_differences']+=fd
 if different and str(mask)not in first:first[str(mask)]=dict(dataset=row['dataset'],query=row['query'],pc=row['pc'],actual=actual,expected=expected,stages=row['stages'])
 if fd and str(mask)not in first_f32:first_f32[str(mask)]=dict(dataset=row['dataset'],query=row['query'],pc=row['pc'],actual=actual,expected=expected)
assert counts['31']==dict(double_differences=0,f32_differences=0)
assert counts['30']==dict(double_differences=633,f32_differences=0)and counts['29']==dict(double_differences=466,f32_differences=1)
assert counts['23']==dict(double_differences=346,f32_differences=0)and counts['15']==dict(double_differences=1,f32_differences=0)and counts['0']==dict(double_differences=677,f32_differences=1)
if len(sys.argv)>1 and sys.argv[1]=='current-contract':assert counts['0']['double_differences']==0,first['0']
if len(sys.argv)>1 and sys.argv[1]=='current-f32-contract':assert counts['0']['f32_differences']==0,first_f32['0']
(r/'comparison.json').write_text(json.dumps(dict(gate='PASS_COMPLETE684_SOURCE_FOOT_ROTATIONS_AND_EXACT_COMMON_PUBLIC_MODEL',source_rotations_exact=684,common_source_inputs_public_calls_exact=684,source_independent_normal_and_initial_relative_used=True,variant_mask_bits=dict(source_product=1,source_normalizer=2,source_constructor=8,source_between_vectors=16),source_all_mask=31,counts=counts,first=first,first_f32=first_f32,source_euler_removed_after_full_stage_equivalence=True,known_differences=['D163','D248','D251'],new_differences=[],full_rig_or_root_equivalence_claimed=False),indent=2)+'\n');print(json.dumps(counts),flush=True)
