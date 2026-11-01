import collections
import gzip
import json
from pathlib import Path

r=Path(__file__).resolve().parent
a=json.loads(gzip.decompress(Path('docs/audit/bf144/460/common-rig.native.json.gz').read_bytes()))['rig_node_history_probe'];b=json.loads((r/'baseline.native.json').read_text()if (r/'baseline.native.json').exists()else gzip.decompress((r/'baseline.native.json.gz').read_bytes()))['rig_node_history_probe'];counts=collections.Counter();first=[]
def diff(x,y,path):
 if type(x)!=type(y):
  counts[path]+=1
  if len(first)<12:first.append(dict(path=path,old=x,new=y))
 elif isinstance(x,dict):
  assert x.keys()==y.keys(),path
  for k in x:diff(x[k],y[k],path+'/'+str(k))
 elif isinstance(x,list):
  assert len(x)==len(y),path
  for i,(v,w)in enumerate(zip(x,y,strict=True)):diff(v,w,path+'/'+str(i))
 elif x!=y:
  counts[path]+=1
  if len(first)<12:first.append(dict(path=path,old=x,new=y))
for side,(x,y)in enumerate(zip(a['cases'],b['cases'],strict=True)):
 for i,(v,w)in enumerate(zip(x['frames'],y['frames'],strict=True)):
  for key in ['work_after','executions']:diff(v[key],w[key],key)
(r/'source-state-differences.json').write_text(json.dumps(dict(counts=dict(counts),first=first),indent=2)+'\n');print(json.dumps(dict(path_count=len(counts),first=first)),flush=True)
