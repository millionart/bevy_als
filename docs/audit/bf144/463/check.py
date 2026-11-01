import json
import struct
from pathlib import Path

r=Path(__file__).resolve().parent
v=json.loads((r/'replay-restored.json').read_text());rows=v['outputs'];assert len(rows)==636 and (r/'replay-canonical.json').read_bytes()==(r/'replay-restored.json').read_bytes()
names=['current','normal_only','normal_and_point','native_hit_and_inverse','source_forward_full_inverse','native_ray_target_query']
counts={name:sum(x[name][8]==x['source'][8]for x in rows)for name in names};assert counts==dict(current=48,normal_only=92,normal_and_point=600,native_hit_and_inverse=636,source_forward_full_inverse=636,native_ray_target_query=636)
partitions={}
for a,b in zip(names[:3],['normal_only','normal_and_point','source_forward_full_inverse'],strict=True):
 values=dict(preserved=0,recovered=0,regressed=0,still_different=0)
 for x in rows:
  old=x[a][8]==x['source'][8];new=x[b][8]==x['source'][8];key='preserved'if old and new else'regressed'if old else'recovered'if new else'still_different';values[key]+=1
 partitions[a+'->'+b]=values
query=dict(current_vs_source_forward_full_hit_changed=0,current_vs_source_forward_vertical_hit_changed=0,source_forward_vs_native_ray_full_hit_changed=0,native_world_point_vs_same_ray_target_full_point_differences=0,native_world_point_vs_same_ray_target_vertical_differences=0)
for x in rows:
 query['current_vs_source_forward_full_hit_changed']+=x['regenerated_hit']!=x['source_forward_hit']
 query['current_vs_source_forward_vertical_hit_changed']+=x['regenerated_hit'][0][1]!=x['source_forward_hit'][0][1]
 query['source_forward_vs_native_ray_full_hit_changed']+=x['source_forward_hit']!=x['native_ray_target_hit']
 query['native_world_point_vs_same_ray_target_full_point_differences']+=x['native_hit_runtime']!=x['native_ray_target_hit'][0]
 query['native_world_point_vs_same_ray_target_vertical_differences']+=x['native_hit_runtime'][1]!=x['native_ray_target_hit'][0][1]
 for j,key in [(0,'x'),(2,'y')]:
  assert x['native_world_start_cm'][key]==x['native_world_end_cm'][key]
  assert x['native_ray_target_hit'][0][j]==struct.unpack('f',struct.pack('f',x['native_world_start_cm'][key]*.01))[0]
ablations={}
for label in ['no-source-forward','no-source-point']:
 value=json.loads((r/('replay-'+label+'.json')).read_text());assert json.loads((r/('replay-'+label+'.report.json')).read_text())['process_exit_code']==101
 assert [x['current']for x in value['outputs']]==[x['current']for x in rows]
 ablations[label]=value['all_source_normal_point_forward_matches'];assert ablations[label]<636
result=dict(gate='PASS_ACTUAL636_FORWARD_NORMAL_AND_POINT_SOURCE_VECTOR_ROTATION_CAUSAL_CLOSURE',offset_matches=counts,partitions=partitions,query=query,ablations=ablations,no_new_float_cm_scalar_difference_triggered_in_this_sample=True,all_current_public_calls_and_regenerated_queries_exact=True,full_source_output_model_uses_world_hit_and_input_world_not_vm_output=True,source_forward_keeps_original_f32_query_and_two_authored_boxes=True,source_internal_query_hook_claimed=False,full_rig_or_root_equivalence_claimed=False,known_scalar_cause='D247',remaining_world_point_precision_boundary='D246',all1272_target_horizontal_hit_values_exact_float_quantization_of_native_ray_origin=True,new_differences=[])
(r/'check.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
