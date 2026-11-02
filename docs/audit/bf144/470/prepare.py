import gzip
import json
import sys
from pathlib import Path

sys.path.insert(0,'tools/animgraph_authoring')
from ue_control_rig_contract import compile_contract
from ue_control_rig_target import compile_target
r=Path(__file__).resolve().parent;data=json.loads(Path('docs/audit/bf144/469/inputs.json').read_bytes());rig=json.loads(gzip.decompress((r/'rig.native.json.gz').read_bytes()));assert compile_target(compile_contract(rig))['program']['instructions']==data['definition']['program']['instructions'];bones={b['name']:b for b in data['definition']['bones']};q=lambda v:[v[k]for k in'xyzw'];rows=[];streams={}
for row in data['rows']:
 src=row['source_arguments_after'];calf=src[0][1];foot=src[1][1];a=q(row['source_calf_initial_global']['rotation']);b=q(row['source_foot_initial_global']['rotation']);expected=src[13];key=(row['dataset'],row['pc']);value=dict(calf=a,foot=b,relative=expected)
 if key in streams:assert streams[key]==value,key
 else:streams[key]=value
 assert src[8]is True
 current=[]
 for mode in row['target_domains']:current.append(mode['after'][13])
 rows.append(dict(dataset=row['dataset'],query=row['query'],pc=row['pc'],source_calf_initial=a,source_foot_initial=b,expected_native_relative=expected,current_calf_initial=bones[calf]['initial_global']['rotation'],current_foot_initial=bones[foot]['initial_global']['rotation'],observed_current_relative=current))
assert len(rows)==684 and len(streams)==10
initializations=0
for source_file in ['docs/audit/bf144/469a/restored.native.json.gz','docs/audit/bf144/469a/controls-restored.native.json.gz']:
 cases=json.loads(gzip.decompress(Path(source_file).read_bytes()))['rig_node_history_probe']['cases']
 for case in(cases[:1]if 'controls-'not in source_file else cases):
  first=case['frames'][0];work=first['work_after']['properties'];ir=compile_contract(rig)['program'];visits=next(x['instructions']for x in first['executions']if x['entry']=='Forwards Solve')
  for pc in[98,128]:
   assert pc not in visits
   init_ref=ir['instructions'][pc]['operands'][8];relative_ref=ir['instructions'][pc]['operands'][13];assert work[init_ref['register']]['elements'][0]['exported_text']=='False';fields={x['name']:x for x in work[relative_ref['register']]['elements'][0]['fields']};assert[fields[k]['numeric_value']for k in'XYZW']==[0,0,0,1];initializations+=1
assert initializations==10
(r/'inputs.json').write_text(json.dumps(dict(rows=rows),separators=(',',':'))+'\n');(r/'lifecycle.json').write_text(json.dumps(dict(gate='PASS_INITIAL_RELATIVE_INPUTS_AND_NATIVE_CACHE_CONSTANCY',calls=684,streams=10,source_initial_inputs_and_hidden_relative_constant_per_stream=True,source_initialized_after_all_calls=True,source_before_first_call_defaults_verified=10,source_input_from_exact_boundary=True,cache_math_not_filled_from_native=True,source_implementation_initializes_once=True,fixture_redundant_cache_holder_not_added=True),indent=2)+'\n');print('Prepared684 exact initial relative source values,10 constant lifetime streams and current data/operator matrix',flush=True)
