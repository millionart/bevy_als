import copy
import gzip
import json
import sys
from pathlib import Path

r=Path(__file__).resolve().parent
def load(label):
 p=r/('runtime-'+label)/'parameters.json';return json.loads(p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix('.json.gz').read_bytes()))
def bones(p):return {b['name']:b for b in p['bones']}
def validate_record(v,rig):
 assert v['node_guid']=='a88d5696-4292-b2ae-ca2e-ac8d13e7dd2f'and v['state_key']=='Default'
 assert v['actual_state']['initialized']and v['actual_state']['weights']==[1,0]and v['actual_state']['last_active_index']==0
 assert v['actual_state']['storage']in ['upcoming_Default','last_state_fallback']and not v['inner_node_updated']
 assert v['base']==v['copy_control']==rig['poses']['rig_output']['pose']and v['copy_control_applied_to_runtime']is False
 assert v['output']==v['public']==v['graph_root']==rig['poses']['ragdolling_output']['pose']==rig['poses']['root_result']['pose']
 a=bones(v['base']);b=bones(v['output']);assert a.keys()==b.keys()and len(a)==80
 for n in a:
  for k in ['translation','scale','weights']:assert a[n][k]==b[n][k]
 for k in ['timestamp','skeleton_asset','root_motion']:assert v['base'][k]==v['output'][k]
def validate(data):
 assert len(data['animation_before'])==len(data['animation_after'])==319
 assert all(x['ragdoll_blend']==[]for x in data['animation_before'])
 count=positive=0;first=None
 for i,row in enumerate(data['animation_after']):
  assert len(row['ragdoll_blend'])==1;v=row['ragdoll_blend'][0];validate_record(v,row['rig_snapshot'])
  a=bones(v['base']);b=bones(v['output'])
  for n in sorted(a):
   if n!='SKM_Als'and a[n]['rotation']!=b[n]['rotation']:
    count+=1;positive+=i>0
    if first is None:first=dict(query=i,bone=n,current=b[n]['rotation'],source_copy_control=a[n]['rotation'])
 old=json.loads(gzip.decompress(Path('docs/audit/bf144/456/runtime-restored/parameters.json.gz').read_bytes()))
 for k in ['rows','after_motors','camera_after_post_update']:assert data[k]==old[k],k
 for k in ['animation_before','animation_after']:assert [{n:v for n,v in x.items()if n!='ragdoll_blend'}for x in data[k]]==old[k],k
 assert (count,positive)==(3728,3714)
 return dict(all319_changed_rotation_rows=count,positive318_changed_rotation_rows=positive,first=first)
if __name__=='__main__':
 label=sys.argv[1];data=load(label);counts=validate(data);negative=[];original=data['animation_after'][0]
 for kind in ['wrong-weights','wrong-active-child','wrong-public','missing-base','copy-control-changed']:
  v=copy.deepcopy(original['ragdoll_blend'][0])
  if kind=='wrong-weights':v['actual_state']['weights']=[.5,.5]
  elif kind=='wrong-active-child':v['actual_state']['last_active_index']=1
  elif kind=='wrong-public':v['public']['bones'][0]['scale'][0]+=1
  elif kind=='missing-base':v['base']['bones'].pop()
  else:v['copy_control']['bones'][0]['translation'][0]+=1
  try:validate_record(v,original['rig_snapshot'])
  except (AssertionError,KeyError):negative.append(kind)
  else:raise AssertionError('Invalid actual single-input Ragdolling observation passed: '+kind)
 result=dict(gate='PASS_ACTUAL319_RAGDOLLING_FULL_WEIGHT_STATE_PUBLIC_KERNEL_AND_COPY_CONTROL',actual_queries=319,actual_weights_all=[1,0],actual_child_all0=True,inner_branch_inactive_all=True,public_full_pose_exact_all=True,base_equals_actual_rig_output_all=True,root_equals_actual_outer_output_all=True,counts=counts,old_five_arrays_exact=True,negative_fixtures_rejected=negative,copy_control_is_only_a_value_not_runtime_mutation=True,new_differences=[],screenshots=0,performance_acceptance='paused')
 (r/('check-'+label+'.json')).write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
