import copy
import gzip
import hashlib
import json
import struct
import sys
from pathlib import Path

sys.path.insert(0,'tools/animgraph_authoring')
from ue_control_rig_contract import compile_contract
from ue_control_rig_target import compile_target,UNIT_ARGUMENTS
r=Path(__file__).resolve().parent;rig=json.loads(gzip.decompress((r/'rig.native.json.gz').read_bytes()));contract=compile_contract(rig);program=contract['program'];current=json.loads(Path('docs/audit/bf144/459/inputs.json').read_text());assert compile_target(contract)['program']['instructions']==current['definition']['program']['instructions']
native=json.loads(gzip.decompress(Path('docs/audit/bf144/465a/restored.native.json.gz').read_bytes()))['rig_node_history_probe']['cases'][0]['frames'];actual=json.loads(gzip.decompress(Path('docs/audit/bf144/464/replay-restored.json.gz').read_bytes()))['modes']
name='FRigUnit_SpringInterpV2::Execute';pcs=[i for i,x in enumerate(program['instructions'])if x['opcode']=='Execute'and program['functions'][x['function']]['name']==name];assert pcs==[54];pc=pcs[0];schema=[x.split(':')for x in UNIT_ARGUMENTS[name].split()];refs=program['instructions'][pc]['operands'];print('Source spring operands '+json.dumps(refs),flush=True)
def writes(op):
 if op['opcode']=='Execute':
  graph,index=op['model'];node=contract['graphs'][graph]['nodes'][index];pins={p['name']:p for p in node['pins']if p['parent']is None};function=program['functions'][op['function']]
  return [ref for arg,ref in zip(function['arguments'],op['operands'],strict=True)if pins[arg['name']]['direction']in ['Output','IO','Hidden']]
 if op['opcode']in ['Copy','Zero']:return[op['output']]
 return[]
def extract(row,kind):
 if row.get('property_class')=='BoolProperty'or kind=='bool':return row['exported_text']=='True'
 if kind in ['float','double']:return row['numeric_value']
 assert kind=='FFloatSpringState',kind
 state=row['native_float_spring_state'];assert row['fields']==[]
 return[state['previous_target'],state['velocity'],state['previous_target_valid']]
def value(ref,frame,kind):
 bank=ref['memory'];assert bank in ['Work','Literal']and not ref['path'];mem=frame['work_after']['properties']if bank=='Work'else rig['rig_vm']['literal']['properties'];row=mem[ref['register']];assert row['index']==ref['register']
 if 'slice'in ref:row=row['elements'][ref['slice']]
 return extract(row,kind)
frames=[];proof=[]
for query,(source,target)in enumerate(zip(native,current['frames'],strict=True)):
 visits=next(x['instructions']for x in source['executions']if x['entry']=='Forwards Solve');assert visits.count(pc)==1
 after=visits[visits.index(pc)+1:];conflicts=[dict(pc=k,argument=j,write=w)for k in after for w in writes(program['instructions'][k])for j,ref in enumerate(refs)if ref['memory']==w['memory']and ref['register']==w['register']];assert not conflicts,(query,conflicts)
 args=[value(ref,source,kind)for ref,(_,kind,_)in zip(refs,schema,strict=True)];domains=[]
 for mode in actual:
  call=next(x for x in mode['outputs'][query]['calls']if x['function']==name);domains.append(dict(source_trace=mode['source'],before=call['arguments_before_call'],after=call['arguments_after_call']))
 frames.append(dict(query=query,delta_seconds=source['delta_seconds_after_update'],source_arguments_after=args,target_domains=domains))
 if query in [0,1,318]:proof.append(dict(query=query,after_visited=after,overwriting_writes=conflicts))
assert len(frames)==319
(r/'inputs.json').write_text(json.dumps(dict(definition=current['definition'],frames=frames),separators=(',',':'))+'\n');(r/'lifetime-proof.json').write_text(json.dumps(dict(gate='PASS_ACTUAL319_PELVIS_SPRING_ARGUMENTS_NO_LATER_ALIAS_WRITE',pc=pc,arguments=refs,schema=schema,frames=319,values=319*12,representative=proof),indent=2)+'\n');print('Qualified319 source spring frame-end arguments and current/trace-control complete input states',flush=True)
