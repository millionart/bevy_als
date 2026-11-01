import copy
import gzip
import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent;current=json.loads(Path('docs/audit/bf144/459/inputs.json').read_text());source=json.loads(gzip.decompress(Path('docs/audit/bf144/457/restored-final.native.json.gz').read_bytes()))['character_animation_probe']['cases'][0];names=[b['name']for b in source['frames'][0]['pose_boundaries']['rig_output']['pose']['bones']];mesh=source['frames'][0]['pose_boundaries']['rig_output']['required_bones_asset']
geometry=json.loads((r/'geometry-authored-boxes.json').read_text());assert geometry['exact']==geometry['total']==636
fmt=lambda x:format(x,'.17g')
def vector(v):return '(X='+fmt(v[0])+',Y='+fmt(v[1])+',Z='+fmt(v[2])+')'
def point(v):return [v[0]*100,v[2]*100,v[1]*100]
def rotation(v):return [-v[0],-v[2],-v[1],v[3]]
def quat(v):return '('+','.join(k+'='+fmt(x)for k,x in zip(['X','Y','Z','W'],v,strict=True))+')'
def rig_input(values):
 b=[values[i]['Bool']for i in range(3)];a=[values[i]['F32']for i in range(3,7)];a[2]=-a[2]
 parts=[n+'='+('True'if x else'False')for n,x in zip(['bUseHandIkBones','bUseFootIkBones','bFootTransformsValid'],b,strict=True)]
 parts +=[n+'='+fmt(x)for n,x in zip(['VelocityBlendForwardAmount','VelocityBlendBackwardAmount','SpineYawAngle','PelvisOffsetAmount'],a,strict=True)]
 for side,i in [('Left',7),('Right',9)]:parts+=['Foot'+side+'Location='+vector(point(values[i]['Vector'])),'Foot'+side+'Rotation='+quat(rotation(values[i+1]['Quaternion']))]
 return '('+','.join(parts)+')'
cases=[];reference_count=0
for side in [0,1]:
 frames=[]
 for f in current['frames']:
  query=f['query'];pose={b['name']:b for b in f['pose']['bones']};leaf=[]
  for n in names:
   b=pose[n];s=b['scale'];leaf.append(dict(translation_cm=point(b['translation']),rotation=rotation(b['rotation']),scale=[s[0],s[2],s[1]]))
  refs=f['expected_traces'];ray=refs[side]if refs else next(x['expected_traces'][side]for x in current['frames']if x['expected_traces'])
  reference_count+=bool(refs);values=f['expected_registers'][current['definition']['externals']['RigInput']['register']]['List'];world=f['to_world'];s=world['scale']
  frames.append(dict(name=f'actual-query-{query:03}',delta_seconds=f['delta_seconds'],variables=dict(RigInput=rig_input(values)),pose=dict(curves=f['curves'],bone_transforms=leaf),mesh_to_world=dict(translation_cm=point(world['translation']),rotation=rotation(world['rotation']),scale=[s[0],s[2],s[1]]),reference_ray_start_cm=point(ray['start']),reference_ray_end_cm=point(ray['end'])))
 cases.append(dict(name='actual319-reference-'+str(side),box_half_extents_cm=[2000,1000,25],box_location_cm=[-1000,-2000,-25],frames=frames))
assert reference_count==636
request=dict(node_guid='5514524f-4eea-c42c-19cd-349ea6f99e36',mesh_asset=mesh,bone_names=names,cases=cases)
(r/'request.json').write_text(json.dumps(request,separators=(',',':'))+'\n')
(r/'request-contract.json').write_text(json.dumps(dict(real_reference_rays=636,additional_initial_reference_rays=2,each_case_one_continuous_rig_instance=True,cases=2,frames_each=319,source_box_is_geometric_union_of_two_authored_floor_boxes=True,source_query_numeric_equivalence_not_claimed=True,current_pose_world_and_input_transformed_by_actual_names_and_basis=True),indent=2)+'\n')
print('Prepared two continuous319 source Rig cases with636 actual reference rays and exact current input producers',flush=True)
