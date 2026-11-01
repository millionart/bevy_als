import json
import gzip
import struct
import sys
from pathlib import Path

r=Path(__file__).resolve().parent;request=json.loads((r/'request.json').read_text());document=json.loads((r/'common-rig.native.json').read_text()if (r/'common-rig.native.json').exists()else gzip.decompress((r/'common-rig.native.json.gz').read_bytes()));native=document['rig_node_history_probe'];assert len(native['cases'])==2
if len(sys.argv)>1 and sys.argv[1].startswith('bad-'):
 frame=native['cases'][0]['frames'][0]
 case=sys.argv[1]
 if case=='bad-delta':frame['delta_seconds_after_update']=0.125
 elif case=='bad-world':frame['mesh_to_world']['translation']['x']+=1
 elif case=='bad-pose':frame['before']['bones'][0]['local_transform']['translation']['x']+=1
 elif case=='bad-input':
  field=next(x for x in frame['externals_after']['RigInput']['fields']if x['name']=='SpineYawAngle');field['numeric_value']+=1
 else:raise ValueError(case)
f32=lambda x:struct.unpack('f',struct.pack('f',x))[0]
world_exact=inputs_exact=ref_count=0;ids=[];source_point_differences=0;first=None
current=json.loads(Path('docs/audit/bf144/459/inputs.json').read_text())['frames']
def components(t,keys):return [t[k]for k in keys]
for side,(wanted,case)in enumerate(zip(request['cases'],native['cases'],strict=True)):
 assert case['game_world']and case['bound_component']and case['adapter_enabled']and case['adapter_ready']and case['external_controls_stage']=='after_update_before_evaluate'
 assert len(case['frames'])==len(wanted['frames'])==319 and len({f['rig_id']for f in case['frames']})==1;ids.append(case['frames'][0]['rig_id']);construction=case['frames'][0]['construction_events']
 for i,(a,b)in enumerate(zip(wanted['frames'],case['frames'],strict=True)):
  assert a['name']==b['name']and b['delta_seconds_after_update']==f32(a['delta_seconds'])and b['leaf_update_weight']==1
  assert b['forward_events']==i+1 and b['construction_events']==construction and b['attribute_count']==0
  assert [v['name']for v in b['before']['bones']]==request['bone_names']
  for expected,bone in zip(a['pose']['bone_transforms'],b['before']['bones'],strict=True):
   t=bone['local_transform'];assert components(t['translation'],'xyz')==expected['translation_cm'];assert components(t['rotation'],'xyzw')==expected['rotation'];assert components(t['scale'],'xyz')==expected['scale']
  assert b['before']['curves']==a['pose']['curves'];inputs_exact+=1
  fields={x['name']:x for x in b['externals_after']['RigInput']['fields']}
  values=current[i]['expected_registers'][394]['List']
  for j,name in enumerate(['bUseHandIkBones','bUseFootIkBones','bFootTransformsValid']):assert (fields[name]['exported_text']=='True')==values[j]['Bool']
  for j,name in enumerate(['VelocityBlendForwardAmount','VelocityBlendBackwardAmount','SpineYawAngle','PelvisOffsetAmount'],3):assert fields[name]['numeric_value']==(-values[j]['F32']if name=='SpineYawAngle'else values[j]['F32'])
  for side_name,j in [('Left',7),('Right',9)]:
   v=values[j]['Vector'];q=values[j+1]['Quaternion'];point=[v[0]*100,v[2]*100,v[1]*100];rotation=[-q[0],-q[2],-q[1],q[3]]
   assert [x['numeric_value']for x in fields['Foot'+side_name+'Location']['fields']]==point
   assert [x['numeric_value']for x in fields['Foot'+side_name+'Rotation']['fields']]==rotation
  expected=a['mesh_to_world'];world=b['mesh_to_world'];assert components(world['translation'],'xyz')==expected['translation_cm']and components(world['rotation'],'xyzw')==expected['rotation']and components(world['scale'],'xyz')==expected['scale']
  executions=[e for e in b['executions']if e['entry']=='Forwards Solve'];assert len(executions)==1 and executions[0]['succeeded']and executions[0]['instructions']
  assert executions[0]['rig_to_world']==world;world_exact+=1
  query=b['reference_query_after_forward'];assert query['blocking_hit']
  if i:
   actual=current[i]['expected_traces'][side]['hit'];point=query['impact_point_cm'];projected=[f32(point['x']*.01),f32(point['z']*.01),f32(point['y']*.01)];normal=query['impact_normal'];assert [normal['x'],normal['z'],normal['y']]==actual[1]
   ref_count+=1;different=projected!=actual[0];source_point_differences+=different
   if different and first is None:first=dict(query=i,side=side,source_projected=projected,actual_current=actual[0])
assert inputs_exact==world_exact==638 and ref_count==636
result=dict(gate='PASS_CONTINUOUS_SOURCE_RIG_POSE_CLOCK_WORLD_INPUT_QUALIFICATION_WITH_EXPLICIT_QUERY_BOUNDARY',cases=2,frames_each=319,before_pose_and_curve_frames_exact=inputs_exact,requested_world_and_actual_execution_world_frames_exact=world_exact,actual_rig_input_fields_exact=638*11,source_instance_ids=ids,source_reference_rays_actual=ref_count,source_reference_point_projection_differences=source_point_differences,first_reference_difference=first,source_query_not_silently_replaced_by_target_hits=True,source_rig_or_root_equivalence_claimed=False,new_differences=[],screenshots=0,performance_acceptance='paused')
(r/'native-check.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
