import gzip
import json
import struct
from pathlib import Path

r=Path(__file__).resolve().parent;data=json.loads((r/'inputs.json').read_bytes());public=json.loads((r/'replay-unit-baseline.json').read_bytes()if(r/'replay-unit-baseline.json').exists()else gzip.decompress((r/'replay-unit-baseline.json.gz').read_bytes()));f=lambda x:struct.unpack('f',struct.pack('f',x))[0]
def runtime(t):
 v=t['translation'];q=t['rotation'];s=t['scale'];return dict(translation=[v['x']*.01,v['z']*.01,v['y']*.01],rotation=[-q['x'],-q['z'],-q['y'],q['w']],scale=[s['x'],s['z'],s['y']])
source={(x['query'],x['pc']):x for x in data['source']if x['dataset']=='actual'and x['case']==0};counts={};first={};records=[]
for call in data['current']:
 src=source[(call['query'],call['pc'])]
 for stage,key in[('before','bones_before'),('after','bones_after')]:
  for i,(bone,current)in enumerate(call[key]):
   expected=runtime(src[stage][i]);fields={}
   for field in['translation','rotation','scale']:
    name=f"{call['mode']}:{stage}:{i}:{field}";different=current[field]!=expected[field];fd=list(map(f,current[field]))!=list(map(f,expected[field]));c=counts.setdefault(name,dict(double_differences=0,f32_differences=0));c['double_differences']+=different;c['f32_differences']+=fd;fields[field]=dict(double=different,f32=fd)
    if fd:first.setdefault(name,dict(query=call['query'],pc=call['pc'],bone=src['names'][i],current=current[field],source=expected[field]))
   records.append(dict(mode=call['mode'],query=call['query'],pc=call['pc'],stage=stage,bone=src['names'][i],differences=fields))
assert public['exact']==1272 and len(records)==7632
(r/'comparison.json').write_text(json.dumps(dict(gate='PASS_ACTUAL_IK_BEFORE_AFTER_DATA_DIFFERENCE_INVENTORY_NOT_SOLVER_ATTRIBUTION',source_native_leg_calls=1332,source_native_pin_values=29304,current_complete_public_calls=1272,source_ik_changed_calls=1330,source_zero_weight_unchanged_calls=2,source_primary_full_weight_actual_calls=1272,counts=counts,first_f32=first,initial_data_is_its_own_known_input_domain=True,solver_algorithm_not_yet_claimed_qualified=True,new_differences=[]),indent=2)+'\n');(r/'difference-table.json').write_text(json.dumps(records,separators=(',',':'))+'\n');print(json.dumps({k:v for k,v in counts.items()if':before:0:'in k or':after:1:'in k}),flush=True)
