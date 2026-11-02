import gzip
import json
import sys
from pathlib import Path

sys.path.insert(0,'tools/animgraph_authoring')
from ue_control_rig_contract import compile_contract
from ue_control_rig_target import compile_target
r=Path(__file__).resolve().parent;base=json.loads(Path('docs/audit/bf144/469/inputs.json').read_bytes());source=json.loads(Path('docs/audit/bf144/471/inputs.json').read_bytes());current=json.loads(Path('docs/audit/bf144/469/replay-restored.json').read_bytes());rig=json.loads(gzip.decompress((r/'rig.native.json.gz').read_bytes()));c=compile_contract(rig);ir=c['program'];assert compile_target(c)['program']['instructions']==base['definition']['program']['instructions'];definition=base['definition'];rows=[];native=json.loads(gzip.decompress(Path('docs/audit/bf144/469a/restored.native.json.gz').read_bytes()))['rig_node_history_probe']['cases'][0]['frames'];projection=lambda q:[-q[0],-q[2],-q[1],q[3]]
def fields(row):return{x['name']:x for x in row['fields']}
def property_value(ref,frame):
 assert ref['memory']in['Work','Literal']and not ref['path'];row=(frame['work_after']['properties']if ref['memory']=='Work'else rig['rig_vm']['literal']['properties'])[ref['register']]
 if'slice'in ref:row=row['elements'][ref['slice']]
 return row
def writes(op):
 if op['opcode']=='Execute':
  graph,index=op['model'];pins={p['name']:p for p in c['graphs'][graph]['nodes'][index]['pins']if p['parent']is None};return[ref for arg,ref in zip(ir['functions'][op['function']]['arguments'],op['operands'],strict=True)if pins[arg['name']]['direction']in['Output','IO','Hidden']]
 if op['opcode']in['Copy','Zero']:return[op['output']]
 return[]
entry=[]
for raw,src in zip(base['rows'],source['rows'],strict=True):
 if raw['dataset']!='actual':continue
 query=raw['query'];pc=raw['pc'];assert src['query']==query and src['pc']==pc;setter=100 if pc==98 else 130;refs=ir['instructions'][setter]['operands'];assert ir['functions'][ir['instructions'][setter]['function']]['name']=='FRigUnit_SetRotation::Execute';assert refs[3]==ir['instructions'][pc]['operands'][12]and refs[0]==ir['instructions'][pc]['operands'][1]
 frame=native[query];visits=next(x['instructions']for x in frame['executions']if x['entry']=='Forwards Solve');assert visits.count(setter)==1;after=visits[visits.index(setter)+1:];aliases=[(k,j)for k in after for write in writes(ir['instructions'][k])for j,ref in enumerate(refs)if ref['memory']==write['memory']and ref['register']==write['register']];assert not aliases,(query,setter,aliases);value=property_value(refs[3],frame);fv=fields(value);source_value=[fv[k]['numeric_value']for k in'XYZW'];assert source_value==src['expected_native'];weight=property_value(refs[4],frame)['numeric_value'];space=property_value(refs[1],frame)['exported_text'];initial=property_value(refs[2],frame)['exported_text'];assert space=='GlobalSpace'and initial=='False';entry.append(dict(query=query,pc=setter,value_from_rotation_pc=pc,source_value=source_value,source_weight=weight,space=space,initial=False,item=raw['source_arguments_after'][1]))
 for mode in[0,1]:
  observed=current['modes'][mode]['outputs'][(query-1)*2+(0 if pc==98 else 1)];assert observed['query']==query and observed['pc']==pc;args=raw['target_domains'][mode]['before'];bones=raw['target_domains'][mode]['bones_before'];keys=[b['name']for b in definition['bones']];by_name={keys[index]:transform for index,transform in bones};calf=src['calf_name'];foot=src['foot_name'];expected=observed['after'][12]['Quaternion'];assert args[7]['F32']==src['half_life'];target=projection(args[2]['Quaternion']);current_limits=[[v['F32']for v in args[5]['List']],[-args[4]['List'][1]['F32'],-args[4]['List'][0]['F32']],[v['F32']for v in args[6]['List']]];normal=observed['after'][11]['Vector']
  rows.append(dict(query=query,pc=pc,mode=mode,source=src,current=dict(calf=projection(by_name[calf]['rotation']),foot=projection(by_name[foot]['rotation']),target=target,normal=[normal[0],normal[2],normal[1]],limits=current_limits,calf_initial=projection(definition['bones'][keys.index(calf)]['initial_global']['rotation']),foot_initial=projection(definition['bones'][keys.index(foot)]['initial_global']['rotation']),expected=projection(expected)),observed_current_runtime=expected))
assert len(rows)==1272 and len(entry)==636
(r/'inputs.json').write_text(json.dumps(dict(definition=definition,rows=rows),separators=(',',':'))+'\n');(r/'setter-entry.json').write_text(json.dumps(dict(gate='PASS_ACTUAL636_SETROTATION_INPUT_VALUE_ITEM_SPACE_BINDING',calls=636,rows=entry,source_setter_math_equivalence_not_claimed=True),separators=(',',':'))+'\n');print('Prepared1272 actual source/current input combinations and636 exact SetRotation consumer entries',flush=True)
