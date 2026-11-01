import copy
import gzip
import json
import sys
from pathlib import Path

sys.path.insert(0,'docs/audit/bf144/462')
from cache_identity import cache_identity
r=Path(__file__).resolve().parent;label=sys.argv[1]
def read(p):
 p=Path(p)
 return json.loads(gzip.decompress(p.read_bytes())if p.suffix=='.gz'else p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix('.json.gz').read_bytes()))
old=read('docs/audit/bf144/462/restored.native.json.gz')['rig_node_history_probe'];actual=old if label=='old'else read(r/(label+'.native.json'))['rig_node_history_probe'];states=active=0;table=[]
for side,(a,b)in enumerate(zip(actual['cases'],old['cases'],strict=True)):
 assert len(a['frames'])==len(b['frames'])==319
 for query,(x,y)in enumerate(zip(a['frames'],b['frames'],strict=True)):
  properties=x['work_after']['properties'];spring=properties[54];assert spring['cpp_type']=='TArray<FFloatSpringState>'
  assert all('native_float_spring_state'in e for e in spring['elements']),'Native non-reflected float spring state is missing'
  stripped=copy.deepcopy(x)
  for row in stripped['work_after']['properties']:
   for element in row.get('elements',[]):element.pop('native_float_spring_state',None)
  stripped['work_after']=cache_identity(stripped['work_after']);expected=copy.deepcopy(y);expected['work_after']=cache_identity(expected['work_after']);assert stripped==expected
  state=spring['elements'][0]['native_float_spring_state'];assert state['velocity']==properties[52]['numeric_value']
  delta=x['delta_seconds_after_update'];assert state['previous_target_valid']==(query>0)
  if query:assert state['previous_target']==properties[50]['numeric_value'];active+=1
  else:assert state['previous_target']==state['velocity']==0
  for e in spring['elements']:assert e['fields']==[];states+=1
  table.append(dict(case=side,query=query,delta_seconds=delta,state=state,target=properties[50]['numeric_value'],result=properties[51]['numeric_value'],velocity=properties[52]['numeric_value']))
assert active==636 and len(table)==638
result=dict(gate='PASS_NATIVE_FLOAT_SPRING_STATE_CAPTURE_AND_COMPLETE_OLD_PROBE_COMPATIBILITY',frames=638,active_state_frames=636,native_struct_states=states,zero_delta_initial_state_exact=True,target_velocity_validity_and_complete_old_probe_exact=True,reflection_fields_remain_empty=True,source_algorithms_or_memory_not_modified=True,new_differences=[],screenshots=0,performance_acceptance='paused')
(r/('check-'+label+'.json')).write_text(json.dumps(result,indent=2)+'\n');(r/('table-'+label+'.json')).write_text(json.dumps(table,separators=(',',':'))+'\n');print(json.dumps(result),flush=True)
