import copy
import gzip
import json
import sys
from pathlib import Path

sys.path.insert(0,'docs/audit/bf144/462')
from cache_identity import cache_identity
r=Path(__file__).resolve().parent;label=sys.argv[1];kind='weights'if label.startswith('weights-')else'actual'
def read(path):
 p=Path(path)
 if not p.exists():p=Path(str(p)+'.gz')
 return json.loads(gzip.decompress(p.read_bytes())if p.suffix=='.gz'else p.read_bytes())
actual=read(r/(label+'.native.json'))['rig_node_history_probe'];old=read('docs/audit/bf144/473/weights.native.json.gz'if kind=='weights'else'docs/audit/bf144/469a/restored.native.json.gz')['rig_node_history_probe'];table=[]
for case_index,(a,b)in enumerate(zip(actual['cases'],old['cases'],strict=True)):
 assert a['unit_boundary_binding_restored']and a['adapter_enabled']and a['adapter_ready'];assert{key:value for key,value in a.items()if key!='frames'}=={key:value for key,value in b.items()if key!='frames'}
 for query,(frame,prior)in enumerate(zip(a['frames'],b['frames'],strict=True)):
  if query%64==0:print('Checked local dirty state case='+str(case_index)+' query='+str(query),flush=True)
  expected=copy.deepcopy(prior);observed=copy.deepcopy(frame)
  for body in[expected,observed]:body.pop('unit_boundaries');body['work_after']=cache_identity(body['work_after'])
  assert observed==expected,(case_index,query)
  boundaries=frame['unit_boundaries'];assert len(boundaries)==(0 if query==0 else 2)
  for boundary,pc,foot in zip(boundaries,[100,130],['foot_l','foot_r'],strict=False):
   assert boundary['instruction']==pc and boundary['function']=='FRigUnit_SetRotation::Execute'and boundary['stage']=='after_native_unit_before_next_instruction';assert len(boundary['items'])==1;item=boundary['items'][0];assert item['argument']=='Item'and item['type']=='Bone'and item['name']==foot and item['valid_transform_item'];dirty=item['current_local_dirty_before_global_read'];assert type(dirty)is bool
   hierarchy={bone['name']:bone for bone in frame['hierarchy_after']['bones']};assert item['current_global_transform']==hierarchy[foot]['current_global_transform']and item['initial_global_transform']==hierarchy[foot]['initial_global_transform'];before=next(bone['local_transform']for bone in frame['before']['bones']if bone['name']==foot);native=next(bone['local_transform']for bone in frame['after']['bones']if bone['name']==foot);assert dirty==(before!=native),'Focused source cohort changed/held local branch does not match the native dirty flag';table.append(dict(case=case_index,query=query,pc=pc,foot=foot,local_dirty_before_global_read=dirty,source_current_global=item['current_global_transform'],source_final_local=native))
expected_count=60 if kind=='weights'else 1272;expected_clean=2 if kind=='weights'else 0;assert len(table)==expected_count and sum(not row['local_dirty_before_global_read']for row in table)==expected_clean
(r/('check-'+label+'.json')).write_text(json.dumps(dict(gate='PASS_EXACT_NATIVE_POST_SETROTATION_LOCAL_DIRTY_AND_FINAL_LOCAL_BRANCH',calls=len(table),dirty=sum(row['local_dirty_before_global_read']for row in table),clean=expected_clean,complete_old_execution_pose_work_and_hierarchy_unchanged=True,descriptor_binding_restored=True),indent=2)+'\n');(r/('table-'+label+'.json')).write_text(json.dumps(table,separators=(',',':'))+'\n');print('Verified post-writer native dirty flag and source local branch '+str(len(table)),flush=True)
