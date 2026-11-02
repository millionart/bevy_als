import gzip
import collections
import json
import struct
import sys
from pathlib import Path

r=Path(__file__).resolve().parent;v=json.loads((r/'setter-matrix.json').read_bytes()if(r/'setter-matrix.json').exists()else gzip.decompress((r/'setter-matrix.json.gz').read_bytes()));assert v['source_exact']==696 and v['actual_public_exact']==v['actual_source_endpoint_exact']==1272;assert len(v['matrix'])==10176 and len(v['factors'])==2784
f=lambda x:struct.unpack('f',struct.pack('f',x))[0]
def flat(t):return t['translation']+t['rotation']+t['scale']
def count(rows,a,b):return dict(double_differences=sum(x[a]!=x[b]for x in rows),f32_differences=sum([f(z)for z in x[a]]!=[f(z)for z in x[b]]for x in rows))
factors={str(mask):count([x for x in v['factors']if x['mask']==mask],'result','native')for mask in range(4)};matrix={};first={}
for mode in[0,1]:
 for mask in range(4):
  for math in[False,True]:
   name=f'{mode}:{mask}:{int(math)}';rows=[dict(x,result=flat(x['result']),native=flat(x['native']))for x in v['matrix']if x['mode']==mode and x['data_mask']==mask and x['source_math']==math];matrix[name]=count(rows,'result','native')
   fd=[x for x in rows if[f(z)for z in x['result']]!=[f(z)for z in x['native']]]
   if fd:first[name]=fd[0]
 assert matrix[f'{mode}:1:0']==matrix[f'{mode}:1:1']==matrix[f'{mode}:3:0']==matrix[f'{mode}:3:1']==dict(double_differences=0,f32_differences=0)
 assert matrix[f'{mode}:0:0']==matrix[f'{mode}:0:1']==matrix[f'{mode}:2:0']==matrix[f'{mode}:2:1']
assert [factors[str(m)]['double_differences']for m in range(4)]==[5,2,3,0]and all(row['f32_differences']==0 for row in factors.values());skips=collections.Counter((x['dataset'],x['source_skip'])for x in v['rows']);assert skips[('actual',True)]==636 and skips[('weights',True)]==52 and skips[('weights',False)]==8
summary=dict(gate='PASS_SETROTATION_SOURCE_GLOBAL_WRITER_COMMON_AND_ACTUAL_PUBLIC_FACTORS',source_calls=696,actual_public_calls_exact=1272,matrix_records=10176,source_skip_actual=636,source_skip_control=52,source_write_control=8,source_partial_weights=[0,0.25,0.5,0.75,1],factor_masks=dict(source_slerp=1,source_normalize=2),factors=factors,matrix=matrix,first_f32=first,source_pose_substitution_not_root_cause=True,current_source_math_alone_no_global_endpoint_restoration=True,new_differences=[])
(r/'comparison.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps(dict(factors=factors,actual={k:matrix[k]for k in['0:0:0','1:0:0','0:1:0','1:1:0']})),flush=True)
if len(sys.argv)>1 and sys.argv[1]=='current-f32-contract':assert matrix['0:0:0']['f32_differences']==0,first['0:0:0']
