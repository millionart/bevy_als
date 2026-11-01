import gzip
import json
import struct
import sys
from pathlib import Path
from cache_identity import cache_identity

r=Path(__file__).resolve().parent
label=sys.argv[1]
def read(path):
 p=Path(path)
 return json.loads(p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix(p.suffix+'.gz').read_bytes()))
original=read('docs/audit/bf144/460/common-rig.native.json')['rig_node_history_probe']
actual=original if label=='old'else read(r/(label+'.native.json'))['rig_node_history_probe']
request=read(r/'request.json');rows=read('docs/audit/bf144/461/inputs.json')['rows'];values={(x['query'],x['pc']):x['source_arguments_raw']for x in rows}
f32=lambda x:struct.unpack('f',struct.pack('f',x))[0]
count=pose_exact=offset_exact=normal_exact=0;table=[]
assert len(actual['cases'])==len(original['cases'])==len(request['cases'])==2
for side,(wanted,case,prior)in enumerate(zip(request['cases'],actual['cases'],original['cases'],strict=True)):
 assert len(case['frames'])==len(prior['frames'])==319
 for query,(f,old,req)in enumerate(zip(case['frames'],prior['frames'],wanted['frames'],strict=True)):
  if query%64==0:print('Checked native VM ray case '+str(side)+' frame '+str(query),flush=True)
  q=f['reference_query_after_forward'];assert all(k in q for k in ['world_start_cm','world_end_cm','vm_impact_point_cm','vm_impact_normal']),'Native VM-space query endpoints or results are missing'
  assert q['blocking_hit']and f['before']==old['before']and f['after']==old['after'];pose_exact+=1
  assert f['mesh_to_world']==old['mesh_to_world']and f['delta_seconds_after_update']==old['delta_seconds_after_update']and f['externals_after']==old['externals_after']
  assert cache_identity(f['work_after'])==cache_identity(old['work_after'])and f['executions']==old['executions']
  if query==0:continue
  pc=[31,40][side];v=values[(query,pc)];assert req['reference_ray_start_cm']==[v[0][0],v[0][1],v[2]]and req['reference_ray_end_cm']==[v[0][0],v[0][1],-v[3]]
  p=q['vm_impact_point_cm'];n=q['vm_impact_normal'];normal=[n[k]for k in 'xyz'];assert normal==v[9]==[0,0,1];normal_exact+=1
  assert f32(p['z'])==v[8];offset_exact+=1;count+=1
  table.append(dict(query=query,pc=pc,world_start_cm=q['world_start_cm'],world_end_cm=q['world_end_cm'],world_hit_cm=q['impact_point_cm'],world_normal=q['impact_normal'],vm_hit_cm=p,vm_normal=n,source_offset_cm=v[8],source_output_normal=v[9]))
assert count==offset_exact==normal_exact==636 and pose_exact==638
result=dict(gate='PASS_NATIVE_VM_SPACE_REFERENCE_QUERY_AND_ACTUAL_SOURCE_UNIT_OUTPUT_QUALIFICATION',real_source_rays=636,initial_extra_queries=2,complete_source_pose_work_externals_and_visits_exact_old=638,cache_instance_pointer_hash_compared_by_source_semantics=True,invalid_cache_minus_one_and_all_keys_indices_values_preserved=True,source_offset_cm_f32_exact=636,source_normal_exact=636,source_internal_query_hook_claimed=False,reference_query_after_forward=True,same_static_world_and_actual_execution_context=True,new_differences=[],screenshots=0,performance_acceptance='paused')
(r/('check-'+label+'.json')).write_text(json.dumps(result,indent=2)+'\n');(r/('table-'+label+'.json')).write_text(json.dumps(table,separators=(',',':'))+'\n');print(json.dumps(result),flush=True)
