import json
import struct
import sys
from pathlib import Path

r=Path(__file__).resolve().parent;data=json.loads((r/'inputs.json').read_bytes());result=json.loads((r/'replay-baseline.json').read_bytes());f32=lambda x:struct.unpack('f',struct.pack('f',x))[0];counts={};first={};current_consumers=0
for original,row in zip(data['rows'],result['outputs'],strict=True):
 assert original['dataset']==row['dataset']and original['query']==row['query']and original['pc']==row['pc'];current_consumers+=len(original['observed_current_relative'])
 for v in row['variants']:
  name=('source'if v['source_data']else'current')+'_data_'+('source'if v['source_order']else'current')+'_product';record=counts.setdefault(name,dict(double_differences=0,f32_differences=0));different=v['actual']!=v['expected'];record['double_differences']+=different;record['f32_differences']+=[f32(x)for x in v['actual']]!=[f32(x)for x in v['expected']]
  if different and name not in first:first[name]=dict(dataset=row['dataset'],query=row['query'],pc=row['pc'],actual=v['actual'],expected=v['expected'])
assert len(result['outputs'])==684 and current_consumers==1272
assert counts['source_data_source_product']==dict(double_differences=0,f32_differences=0)
for key,value in counts.items():
 if key!='source_data_source_product':assert value==dict(double_differences=684,f32_differences=0)
if len(sys.argv)>1 and sys.argv[1]=='current-contract':assert counts['source_data_current_product']['double_differences']==0,first['source_data_current_product']
(r/'comparison.json').write_text(json.dumps(dict(gate='PASS_ACTUAL684_INITIAL_RELATIVE_SOURCE_QUATERNION_AND_DATA_PRODUCT_MATRIX',source_relative_values_exact=684,actual_current_cache_consumers_exact=1272,counts=counts,first=first,no_source_after_cache_value_injected=True,fixture_cache_holder_omitted_after_constant_input_lifetime_proof=True,known_differences=['D163','D242'],new_differences=[],full_foot_rotation_or_rig_equivalence_claimed=False),indent=2)+'\n');print(json.dumps(counts),flush=True)
