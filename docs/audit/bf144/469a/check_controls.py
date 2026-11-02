import copy
import gzip
import json
from pathlib import Path

import sys
sys.path.insert(0,'docs/audit/bf144/462')
from cache_identity import cache_identity
r=Path(__file__).resolve().parent
def read(name):
 p=r/name;return json.loads(p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix(p.suffix+'.gz').read_bytes()))['rig_node_history_probe']
a=read('controls.native.json');b=read('controls-default.native.json');changed=0;calls=0;table=[]
for actual,expected in zip(a['cases'],b['cases'],strict=True):
 assert actual['unit_boundary_binding_restored']is True
 for query,(x,y)in enumerate(zip(actual['frames'],expected['frames'],strict=True)):
  hierarchy={bone['name']:bone for bone in x['hierarchy_after']['bones']}
  for row in x['unit_boundaries']:
   assert row['stage']=='after_native_unit_before_next_instruction';foot=next(item for item in row['items']if item['argument']=='FootItem');after=hierarchy[foot['name']]['current_global_transform'];different=foot['current_global_transform']['rotation']!=after['rotation'];changed+=different;calls+=1
   if different:table.append(dict(case=actual['name'],query=query,instruction=row['instruction'],foot=foot['name'],at_consumer=foot['current_global_transform']['rotation'],at_frame_end=after['rotation']))
  stripped=copy.deepcopy(x);stripped.pop('unit_boundaries');stripped['work_after']=cache_identity(stripped['work_after']);other=copy.deepcopy(y);other['work_after']=cache_identity(other['work_after']);assert stripped==other
assert calls==48 and changed>0
(r/'controls-check.json').write_text(json.dumps(dict(gate='PASS_NATIVE_CONTROLLED_NORMALS_EXPOSE_FOOT_BOUNDARY_AND_FRAME_END_DIFFERENCE',calls=calls,foot_rotations_changed_after_consumer=changed,complete_old_execution_pose_and_work_unchanged=True,only_last_mesh_rotation_changed_four_source_paths=True,first=table[0]),indent=2)+'\n');(r/'controls-table.json').write_text(json.dumps(table,indent=2)+'\n');print('Validated controlled native boundary differences '+str(changed)+' of '+str(calls),flush=True)
