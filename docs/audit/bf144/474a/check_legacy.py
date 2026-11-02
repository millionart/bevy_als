import gzip
import json
import sys
from pathlib import Path

sys.path.insert(0,'docs/audit/bf144/462')
from cache_identity import cache_identity
r=Path(__file__).resolve().parent
p=r/'legacy-boundary.native.json';actual=json.loads(p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix(p.suffix+'.gz').read_bytes()))['rig_node_history_probe'];old=json.loads(gzip.decompress(Path('docs/audit/bf144/469a/restored.native.json.gz').read_bytes()))['rig_node_history_probe'];count=0
for case in actual['cases']:
 for frame in case['frames']:
  for boundary in frame['unit_boundaries']:
   for item in boundary['items']:assert type(item.pop('current_local_dirty_before_global_read'))is bool;count+=1
for body in[actual,old]:
 for case in body['cases']:
  for frame in case['frames']:frame['work_after']=cache_identity(frame['work_after'])
assert actual==old and count==2544
(r/'legacy-boundary-check.json').write_text(json.dumps(dict(gate='PASS_COMPLETE_LEGACY_UNIT_BOUNDARY_EXCEPT_ONE_ADDITIVE_DIRTY_FIELD',getter_values=2544,all_old_boundary_items_current_initial_execution_pose_state_exact=True),indent=2)+'\n');print('Verified2544 additive typed flags and byte-semantic complete old boundary compatibility',flush=True)
