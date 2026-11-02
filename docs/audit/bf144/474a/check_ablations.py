import copy
import gzip
import json
from pathlib import Path
import sys

sys.path.insert(0,'docs/audit/bf144/462')
from cache_identity import cache_identity
r=Path(__file__).resolve().parent

def read(name):
 p=r/name
 if not p.exists():p=r/(name+'.gz')
 return json.loads(gzip.decompress(p.read_bytes())if p.suffix=='.gz'else p.read_bytes())

baseline=read('weights-baseline.native.json')['rig_node_history_probe'];results={}
for label,expected in[('flag-false',58),('flag-true',2)]:
 observed=read('weights-'+label+'.native.json')['rig_node_history_probe'];wrong=[];left=copy.deepcopy(observed);right=copy.deepcopy(baseline)
 for ci,(a,b)in enumerate(zip(left['cases'],right['cases'],strict=True)):
  for qi,(x,y)in enumerate(zip(a['frames'],b['frames'],strict=True)):
   x['work_after']=cache_identity(x['work_after']);y['work_after']=cache_identity(y['work_after'])
   for u,v in zip(x['unit_boundaries'],y['unit_boundaries'],strict=True):
    for item,prior in zip(u['items'],v['items'],strict=True):
     actual=item.pop('current_local_dirty_before_global_read');truth=prior.pop('current_local_dirty_before_global_read')
     if actual!=truth:wrong.append(dict(case=ci,query=qi,pc=u['instruction'],actual=actual,expected=truth))
 assert left==right and len(wrong)==expected;results[label]=dict(wrong_flags=len(wrong),first=wrong[0],all_native_execution_pose_state_and_old_fields_unchanged=True);print('Verified necessary dirty getter '+label+' wrong='+str(len(wrong)),flush=True)
(r/'ablations.json').write_text(json.dumps(dict(gate='PASS_DIRTY_FLAG_GETTER_FALSE_AND_TRUE_NECESSITY_WITH_UNCHANGED_OLD_PROBE',results=results),indent=2)+'\n')
