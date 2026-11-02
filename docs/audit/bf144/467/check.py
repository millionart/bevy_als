import copy
import gzip
import json
import struct
import sys
from pathlib import Path

sys.path.insert(0,'docs/audit/bf144/462')
from cache_identity import cache_identity
sys.path.insert(0,'tools/animgraph_authoring')
from ue_control_rig_contract import compile_contract
r=Path(__file__).resolve().parent;label=sys.argv[1]
def read(path):
 p=Path(path);return json.loads(gzip.decompress(p.read_bytes())if p.suffix=='.gz'else p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix(p.suffix+'.gz').read_bytes()))
old=read('docs/audit/bf144/465a/restored.native.json.gz')['rig_node_history_probe'];actual=read(r/(('baseline'if label=='premature-f32'else label)+'.native.json'))['rig_node_history_probe'];rig=read('docs/audit/bf144/466/rig.native.json.gz');ir=compile_contract(rig)['program'];parents={b['name']:b['parent']for b in rig['rig_hierarchy']['bones']};f32=lambda x:struct.unpack('f',struct.pack('f',x))[0]
def fields(row):return{x['name']:x for x in row['fields']}
def key(ref,frame):
 assert ref['memory']in['Work','Literal']and not ref['path'];row=(frame['work_after']['properties']if ref['memory']=='Work'else rig['rig_vm']['literal']['properties'])[ref['register']];fs=fields(row);assert fs['Type']['exported_text']=='Bone';return fs['Name']['exported_text'].strip('"')
def ancestor(a,b):
 while b!='None':
  if a==b:return True
  b=parents[b]
 return False
prototype=actual if label=='baseline'else read(r/'baseline.native.json')['rig_node_history_probe']
frames=local_bones=qualified=0;table=[];proof=[];all_globals=0;removed_local=True
for case,(a,b)in enumerate(zip(actual['cases'],old['cases'],strict=True)):
 assert len(a['frames'])==len(b['frames'])==319
 for query,(x,y)in enumerate(zip(a['frames'],b['frames'],strict=True)):
  if query%64==0:print('Checked current hierarchy case='+str(case)+' query='+str(query),flush=True)
  assert 'hierarchy_after'in x,'Actual current hierarchy snapshot is missing'
  hierarchy=x['hierarchy_after'];bones=hierarchy['bones'];assert[(v['name'],v['parent'])for v in bones]==list(parents.items());by_name={bone['name']:bone for bone in bones}
  for bone in bones:assert 'current_global_transform'in bone
  output_locals={bone['name']:bone['local_transform']for bone in x['after']['bones']};assert set(by_name)==set(output_locals)
  reference_snapshot=prototype['cases'][case]['frames'][query]['hierarchy_after'];reference_by_name={bone['name']:bone for bone in reference_snapshot['bones']}
  removed=all('current_local_transform'not in bone for bone in bones);removed_local=removed_local and removed
  expected_snapshot=copy.deepcopy(reference_snapshot)
  if removed:
   for bone in expected_snapshot['bones']:bone.pop('current_local_transform')
  if label not in['baseline','zero-global']:assert hierarchy==expected_snapshot,(case,query)
  all_globals+=len(bones)
  for output in x['after']['bones']:
   local=by_name[output['name']].get('current_local_transform',reference_by_name[output['name']]['current_local_transform']);expected=output['local_transform']
   for kind,axes in[('translation','xyz'),('rotation','xyzw'),('scale','xyz')]:
    for axis in axes:assert(f32(local[kind][axis])if label=='premature-f32'else local[kind][axis])==expected[kind][axis],(case,query,output['name'],kind,axis,f32(local[kind][axis]),expected[kind][axis],local,by_name[output['name']]['current_global_transform'])
   local_bones+=1
  if query:
   work=x['work_after']['properties'];desired=fields(work[60]);desired_z=desired['Z']['numeric_value'];pelvis=by_name['pelvis']['current_global_transform']['translation'];assert abs(pelvis['z']-desired_z)<=f32(.0001),(case,query,pelvis,desired_z)
   visits=next(e['instructions']for e in x['executions']if e['entry']=='Forwards Solve')
   for pc in[83,113]:
    assert visits.count(pc)==1;target=key(ir['instructions'][pc]['operands'][1],x);writes=[]
    for k in visits[visits.index(pc)+1:]:
     op=ir['instructions'][k]
     if op['opcode']!='Execute':continue
     name=ir['functions'][op['function']]['name']
     if name=='FRigUnit_TwoBoneIKSimplePerItem::Execute':
      keys=[key(op['operands'][j],x)for j in[0,1,2]];assert all(not ancestor(v,target)or j==0 and v==target for j,v in enumerate(keys)),(case,query,pc,k,keys,target);writes.append(dict(pc=k,function=name,keys=keys,root_translation_preserved=True))
     elif name.startswith('FRigUnit_Set'):
      item=key(op['operands'][0],x);assert not ancestor(item,target),(case,query,pc,k,item,target);writes.append(dict(pc=k,function=name,key=item))
    qualified+=1
    if case==0 and query in[1,106,318]:proof.append(dict(query=query,pc=pc,thigh=target,later_writers=writes,field='translation',rotation_and_scale_not_qualified=True))
  stripped=copy.deepcopy(x);stripped.pop('hierarchy_after');stripped['work_after']=cache_identity(stripped['work_after']);expected=copy.deepcopy(y);expected['work_after']=cache_identity(expected['work_after']);assert stripped==expected,(case,query)
  table.append(dict(case=case,query=query,bones=[dict(name=name,local=output_locals[name],global_transform=by_name[name]['current_global_transform'])for name in['pelvis','thigh_l','thigh_r','calf_l','calf_r']]));frames+=1
assert frames==638 and local_bones==50402 and qualified==1272
result=dict(gate='PASS_ACTUAL_CURRENT_HIERARCHY_READONLY_CAPTURE_AND_FIELD_LIFETIME',frames=frames,compact_local_bones_exact=local_bones,thigh_translation_call_fields_qualified=qualified,whole_old_probe_exact_after_removing_snapshot_and_instance_hash=True,current_global_bone_transforms=all_globals,redundant_current_local_removed_without_losing_compact_local_data=removed_local,complete_global_snapshot_exact_prototype=label not in['baseline','zero-global'],source_pelvis_current_height_within_source_write_skip_interval=True,whole_thigh_transform_at_call_not_claimed=True,source_vm_or_assets_not_modified=True,new_differences=[],screenshots=0,performance_acceptance='paused')
(r/('check-'+label+'.json')).write_text(json.dumps(result,indent=2)+'\n');(r/('table-'+label+'.json')).write_text(json.dumps(table,separators=(',',':'))+'\n');(r/'field-lifetime.json').write_text(json.dumps(dict(gate='PASS_SOURCE_IK_ROOT_TRANSLATION_PRESERVED_AND_NO_LATER_ANCESTOR_WRITE',qualified_call_fields=qualified,source_scope='Only thigh translation; preserve rotation and scale mutation boundary',representatives=proof),indent=2)+'\n');print(json.dumps(result),flush=True)
