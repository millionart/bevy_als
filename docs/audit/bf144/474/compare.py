import gzip
import json
import struct
import sys
from pathlib import Path

r=Path(__file__).resolve().parent;v=json.loads((r/'replay-baseline.json').read_bytes()if(r/'replay-baseline.json').exists()else gzip.decompress((r/'replay-baseline.json.gz').read_bytes()));f=lambda x:struct.unpack('f',struct.pack('f',x))[0]
def runtime_native(n):
 t=n['translation'];q=n['rotation'];s=n['scale'];return[t['x']*.01,t['z']*.01,t['y']*.01,-q['x'],-q['z'],-q['y'],q['w'],s['x'],s['z'],s['y']]
def runtime_model(n):
 t=n['translation_runtime'];q=n['rotation'];s=n['scale'];return t+[-q[0],-q[2],-q[1],q[3]]+[s[0],s[2],s[1]]
def flat(n):return n['translation']+n['rotation']+n['scale']
def counts(rows):
 return{key:dict(double_differences=sum(a[start:end]!=b[start:end]for a,b in rows),f32_differences=sum([f(x)for x in a[start:end]]!=[f(x)for x in b[start:end]]for a,b in rows))for key,start,end in[('all',0,10),('translation',0,3),('rotation',3,7),('scale',7,10)]}
common={str(mask):counts([(runtime_model(x['result']),runtime_native(x['native']))for x in v['outputs']if x['mask']==mask])for mask in range(16)};matrix={};first={}
for mode in[0,1]:
 for data in range(4):
  for math in range(16):
   key=f'{mode}:{data}:{math}';rows=[x for x in v['matrix']if x['mode']==mode and x['data_mask']==data and x['math_mask']==math];matrix[key]=counts([(flat(x['result']),flat(x['native']))for x in rows])
   bad=[x for x in rows if[f(y)for y in flat(x['result'])]!=[f(y)for y in flat(x['native'])]]
   if bad:first[key]=bad[0]
 assert matrix[f'{mode}:3:15']['all']==dict(double_differences=0,f32_differences=0)
assert v['source_exact']==696 and v['current_actual_exact']==v['source_endpoint_exact']==1272 and len(v['matrix'])==81408 and len(v['outputs'])==11136
assert common['15']['all']==dict(double_differences=0,f32_differences=0)
(r/'comparison.json').write_text(json.dumps(dict(gate='PASS_FOOT_LOCAL_NATIVE_CACHE_COMMON_AND_ACTUAL_FACTORS',source_exact=696,current_actual_exact=1272,matrix_records=81408,common_records=11136,data_groups=dict(foot_global=1,parent_global=2),math_factors=dict(source_basis_product=1,source_neon_normalize=2,source_simd_vector=4,source_centimeter_phase=8),common=common,matrix=matrix,first_f32=first,new_differences=[]),indent=2)+'\n');print(json.dumps(dict(common={key:common[key]for key in['0','7','11','13','14','15']},actual={key:matrix[key]for key in['0:0:0','1:0:0','0:3:0','1:3:0','0:0:15','1:0:15']})),flush=True)
if len(sys.argv)>1 and sys.argv[1]=='current-f32-contract':assert matrix['0:0:0']['all']['f32_differences']==0,first['0:0:0']
