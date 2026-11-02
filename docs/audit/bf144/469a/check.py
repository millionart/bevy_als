import copy
import gzip
import json
import sys
from pathlib import Path

sys.path.insert(0,'docs/audit/bf144/462')
from cache_identity import cache_identity
sys.path.insert(0,'tools/animgraph_authoring')
from ue_control_rig_contract import compile_contract
r=Path(__file__).resolve().parent;label=sys.argv[1]
def read(path):
 p=Path(path);return json.loads(gzip.decompress(p.read_bytes())if p.suffix=='.gz'else p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix(p.suffix+'.gz').read_bytes()))
actual=read(r/(label+'.native.json'))['rig_node_history_probe'];old=read('docs/audit/bf144/467/restored.native.json.gz')['rig_node_history_probe'];rig=read('docs/audit/bf144/468/rig.native.json.gz');ir=compile_contract(rig)['program'];rows=[];changed=0;full=0
fields=lambda row:{x['name']:x for x in row['fields']}
def quat(row):
 f=fields(row);return[f[k]['numeric_value']for k in'XYZW']
def memory(ref,frame):
 assert ref['memory']in['Work','Literal']and not ref['path'];return(frame['work_after']['properties']if ref['memory']=='Work'else rig['rig_vm']['literal']['properties'])[ref['register']]
for case,(a,b)in enumerate(zip(actual['cases'],old['cases'],strict=True)):
 assert a['unit_boundary_binding_restored']is True;assert len(a['frames'])==len(b['frames'])==319
 stripped_case={k:v for k,v in a.items()if k not in['frames','unit_boundary_binding_restored']};assert stripped_case=={k:v for k,v in b.items()if k!='frames'}
 for query,(x,y)in enumerate(zip(a['frames'],b['frames'],strict=True)):
  if query%64==0:print('Checked native unit boundary case='+str(case)+' query='+str(query),flush=True)
  boundaries=x['unit_boundaries'];assert len(boundaries)==(0 if query==0 else 2)
  hierarchy={bone['name']:bone for bone in x['hierarchy_after']['bones']}
  for row,pc,calf,foot in zip(boundaries,[98,128],['calf_l','calf_r'],['foot_l','foot_r'],strict=False):
   assert row['instruction']==pc and row['function']=='FAlsRigUnit_ApplyFootOffsetRotation::Execute';assert row['stage']=='after_native_unit_before_next_instruction';assert row['delta_seconds']==x['delta_seconds_after_update'];items={item['argument']:item for item in row['items']};assert set(items)=={'CalfItem','FootItem'}
   for argument,name in[('CalfItem',calf),('FootItem',foot)]:
    item=items[argument];assert item['type']=='Bone'and item['name']==name and item['valid_transform_item']is True;assert item['initial_global_transform']==hierarchy[name]['initial_global_transform']
   assert items['CalfItem']['current_global_transform']==hierarchy[calf]['current_global_transform']
   foot_at_call=items['FootItem']['current_global_transform'];after=hierarchy[foot]['current_global_transform'];different=foot_at_call['rotation']!=after['rotation'];changed+=different
   ik=91 if pc==98 else 121;operands=ir['instructions'][ik]['operands'];weight=memory(operands[13],x)['numeric_value']
   if weight==1:
    effector=fields(memory(operands[3],x));expected=quat(effector['Rotation']);q=foot_at_call['rotation'];actual_quat=[q[k]for k in'xyzw'];assert min(max(abs(v-w)for v,w in zip(actual_quat,expected,strict=True)),max(abs(v+w)for v,w in zip(actual_quat,expected,strict=True)))<=.0001,(case,query,pc,actual_quat,expected);full+=1
   rows.append(dict(case=case,query=query,pc=pc,delta_seconds=row['delta_seconds'],items=row['items'],foot_end_global=after,foot_rotation_changed_later=different,ik_weight=weight))
  stripped=copy.deepcopy(x);stripped.pop('unit_boundaries');stripped['work_after']=cache_identity(stripped['work_after']);expected=copy.deepcopy(y);expected['work_after']=cache_identity(expected['work_after']);assert stripped==expected,(case,query)
assert len(rows)==1272 and full>0
result=dict(gate='PASS_EXACT_NATIVE_UNIT_BOUNDARY_ITEMS_AND_COMPLETE_OLD_PROBE',calls=1272,frames=638,foot_rotation_changed_between_consumer_and_frame_end=changed,full_weight_ik_effector_rotation_source_bounds=full,current_calf_and_initial_items_exact_frame_end=True,complete_old_probe_and_execution_and_pose_exact=True,descriptor_and_thread_local_binding_restored=True,stage='after_native_unit_before_next_instruction',source_unit_readonly_hierarchy_makes_consumed_pose_exact=True,new_differences=[],screenshots=0,performance_acceptance='paused');(r/('check-'+label+'.json')).write_text(json.dumps(result,indent=2)+'\n');(r/('table-'+label+'.json')).write_text(json.dumps(rows,separators=(',',':'))+'\n');print(json.dumps(result),flush=True)
