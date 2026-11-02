import copy
import gzip
import json
import sys
from pathlib import Path

sys.path.insert(0,'docs/audit/bf144/462')
from cache_identity import cache_identity
r=Path(__file__).resolve().parent;label=sys.argv[1];kind='weights'if label.startswith('weights')else'actual'
def read(path):
 p=Path(path)
 if not p.exists():p=Path(str(p)+'.gz')
 return json.loads(gzip.decompress(p.read_bytes())if p.suffix=='.gz'else p.read_bytes())
a=read(r/(label+'.native.json'))['rig_node_history_probe'];b=read('docs/audit/bf144/473/weights.native.json.gz'if kind=='weights'else'docs/audit/bf144/469a/restored.native.json.gz')['rig_node_history_probe'];calls=0
for ci,(x,y)in enumerate(zip(a['cases'],b['cases'],strict=True)):
 assert x['unit_boundary_binding_restored']and x['adapter_enabled']and x['adapter_ready'];assert{k:v for k,v in x.items()if k!='frames'}=={k:v for k,v in y.items()if k!='frames'}
 for qi,(frame,prior)in enumerate(zip(x['frames'],y['frames'],strict=True)):
  if qi%64==0:print('Checked complete IK observation '+label+' case='+str(ci)+' query='+str(qi),flush=True)
  p=copy.deepcopy(frame);q=copy.deepcopy(prior);boundaries=p.pop('unit_boundaries');q.pop('unit_boundaries');p['work_after']=cache_identity(p['work_after']);q['work_after']=cache_identity(q['work_after']);assert p==q,(label,ci,qi)
  sites=[86,116]if label.endswith('before')else[91,121,139,146];visited=[pc for execution in frame['executions']if execution['entry']=='Forwards Solve'for pc in execution['instructions']];pcs=[pc for pc in visited if pc in sites];assert[b['instruction']for b in boundaries]==pcs
  for boundary in boundaries:
   assert boundary['stage']=='after_native_unit_before_next_instruction'and len(boundary['items'])==3
   for item in boundary['items']:assert item['valid_transform_item']and item['type']=='Bone'and type(item['current_local_dirty_before_global_read'])is bool
  calls+=len(boundaries)
expected=(60 if kind=='weights'else 1272)if label.endswith('before')else(130 if kind=='weights'else 2548);assert calls==expected
(r/('check-'+label+'.json')).write_text(json.dumps(dict(gate='PASS_COMPLETE_NATIVE_IK_BOUNDARY_OBSERVATION_TRANSPARENCY',calls=calls,complete_old_native_execution_pose_work_state_and_hierarchy_unchanged=True,descriptor_binding_restored=True),indent=2)+'\n');print('Verified complete unchanged native history and '+str(calls)+' typed boundary calls',flush=True)
