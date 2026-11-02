import json
import struct
import sys
from pathlib import Path

r=Path(__file__).resolve().parent;v=json.loads((r/'replay-baseline.json').read_bytes());f32=lambda x:struct.unpack('f',struct.pack('f',x))[0];counts={};first={};first_f32={};records={}
for row in v['outputs']:
 mode=row['mode'];mask=row['data_mask'];source_math=row['source_math'];name=f'{mode}:{mask}:{int(source_math)}';actual=row['result'];expected=row['expected_native'];different=actual!=expected;fd=[f32(x)for x in actual]!=[f32(x)for x in expected];c=counts.setdefault(name,dict(double_differences=0,f32_differences=0));c['double_differences']+=different;c['f32_differences']+=fd;records[(mode,row['query'],row['pc'],mask,source_math)]=row
 if fd and name not in first_f32:first_f32[name]=dict(query=row['query'],pc=row['pc'],actual=actual,expected=expected)
 if different and name not in first:first[name]=dict(query=row['query'],pc=row['pc'],actual=actual,expected=expected)
assert len(v['outputs'])==40704 and v['current_actual_exact']==v['source_endpoint_exact']==1272
for mode in[0,1]:
 assert counts[f'{mode}:15:1']==dict(double_differences=0,f32_differences=0)
 assert counts[f'{mode}:0:0']['f32_differences']==(139 if mode==0 else 128)
 assert counts[f'{mode}:0:1']['f32_differences']==(139 if mode==0 else 128)
 assert counts[f'{mode}:1:0']['f32_differences']==1
partitions={}
for name in counts:
 mode,mask,math=map(int,name.split(':'));recovered=regressed=still=0
 for(mode_id,query,pc,data_mask,source_math),row in records.items():
  if mode_id!=mode or data_mask!=mask or source_math!=bool(math):continue
  base=records[(mode,query,pc,0,False)];expected=row['expected_native'];a=[f32(x)for x in row['result']]!=[f32(x)for x in expected];b=[f32(x)for x in base['result']]!=[f32(x)for x in expected];recovered+=b and not a;regressed+=not b and a;still+=a and b
 partitions[name]=dict(recovered=recovered,regressed=regressed,still_different=still)
if len(sys.argv)>1 and sys.argv[1]in['current-contract','current-f32-contract']:assert counts['0:0:0']['f32_differences']==0,first_f32['0:0:0']
(r/'comparison.json').write_text(json.dumps(dict(gate='PASS_ACTUAL1272_ROTATION_DATA_AND_ALGORITHM_COMPOSITION',actual_current_models_exact=1272,source_full_endpoints_exact=1272,matrix_values=40704,data_groups=dict(pose=1,initial=2,target_and_limits=4,normal_history=8),counts=counts,first=first,first_f32=first_f32,f32_partitions=partitions,pose_substitution_not_claimed_root_cause=True,earlier_actual_pose_producer_still_open=True,no_whole_rig_or_root_equivalence_claimed=True,new_differences=[]),indent=2)+'\n');print(json.dumps({key:counts[key]for key in['0:0:0','0:0:1','0:1:0','0:15:1','1:0:0','1:0:1','1:1:0','1:15:1']}),flush=True)
