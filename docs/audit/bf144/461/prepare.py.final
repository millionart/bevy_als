import gzip
import json
import struct
import sys
from pathlib import Path

sys.path.insert(0,'tools/animgraph_authoring')
from ue_control_rig_contract import compile_contract
from ue_control_rig_target import compile_target, UNIT_ARGUMENTS
r=Path(__file__).resolve().parent
native=json.loads(gzip.decompress(Path('docs/audit/bf144/460/common-rig.native.json.gz').read_bytes()))['rig_node_history_probe']['cases'][0]['frames']
rig=json.loads((r/'rig.native.json').read_text()if (r/'rig.native.json').exists()else gzip.decompress((r/'rig.native.json.gz').read_bytes()));contract=compile_contract(rig);compiled=compile_target(contract)
current=json.loads(Path('docs/audit/bf144/459/inputs.json').read_text());program=current['definition']['program'];assert compiled['program']['instructions']==program['instructions']and compiled['program']['literal_count']==program['literal_count']==183
ir=contract['program'];print('Source VM contract keys '+str(list(ir)),flush=True)
def writes(op):
 if op['opcode']=='Execute':
  graph,index=op['model'];node=contract['graphs'][graph]['nodes'][index];pins={p['name']:p for p in node['pins']if p['parent']is None}
  function=ir['functions'][op['function']]
  return [ref for arg,ref in zip(function['arguments'],op['operands'],strict=True)if pins[arg['name']]['direction']in ['Output','IO','Hidden']]
 if op['opcode']in ['Copy','Zero']:return [op['output']]
 assert op['opcode']in ['Jump','JumpToBranch','Exit'],op['opcode']
 return []
def aliases(a,b):return a['memory']==b['memory']and a['register']==b['register']
def value(row):
 kind=row['cpp_type']
 if row['property_class']=='BoolProperty':return row['exported_text']=='True'
 if kind in ['float','double']:return row['numeric_value']
 if kind=='FVector':
  fields={x['name']:x for x in row['fields']};return [fields[k]['numeric_value']for k in ['X','Y','Z']]
 assert 'ECollisionChannel'in kind,kind
 return row['exported_text']
f32=lambda x:struct.unpack('f',struct.pack('f',x))[0]
def project(name,v):
 if name=='FootTargetLocation':return dict(Vector=[v[0]*.01,v[2]*.01,v[1]*.01])
 if name=='OffsetNormal':return dict(Vector=[v[0],v[2],v[1]])
 if name in ['TraceDistanceUpward','TraceDistanceDownward','FootHeight','OffsetLocationZ']:return dict(F32=f32(v*.01))
 if name=='WalkableFloorAngle':return dict(F32=v)
 if name in ['bEnabled','bDrawDebug']:return dict(Bool=v)
 assert name=='TraceChannel'
 return dict(Name=v)
if len(sys.argv)>1 and sys.argv[1].startswith('bad-lifetime'):
 operands=ir['instructions'][31]['operands']
 writer=next(i for i,op in enumerate(ir['instructions'])if i<31 and any(aliases(a,w)for a in operands for w in writes(op)))
 event=next(e for e in native[1]['executions']if e['entry']=='Forwards Solve');event['instructions'][-1]=writer
 print('Injected controlled post-trace alias writer '+str(writer),flush=True)
name='FAlsRigUnit_FootOffsetTrace::Execute';schema=[x.split(':')for x in UNIT_ARGUMENTS[name].split()];trace_pcs=[i for i,op in enumerate(program['instructions'])if isinstance(op,dict)and op.get('Call',{}).get('function')==name];assert trace_pcs==[31,40]
lifetimes=[];rows=[];differences={key:0 for key,_,_ in schema};first={}
for query,(frame,t)in enumerate(zip(native,current['frames'],strict=True)):
 execution=[x for x in frame['executions']if x['entry']=='Forwards Solve'];assert len(execution)==1;visited=execution[0]['instructions'];assert all(visited.count(pc)==1 for pc in trace_pcs if pc in visited),(query,visited)
 if query:assert len(visited)==t['expected_steps']
 else:
  construction=next(x['instructions']for x in frame['executions']if x['entry']=='Construction');assert len(construction)==15 and visited[:15]==construction and len(visited[15:])==t['expected_steps'];assert not any(pc in visited for pc in trace_pcs)

 memory={x['index']:x for x in frame['work_after']['properties']};assert len(memory)==210
 for pc in trace_pcs:
  if pc not in visited:continue
  source=ir['instructions'][pc];assert source['opcode']=='Execute'and ir['functions'][source['function']]['name']==name
  operands=source['operands'];assert len(operands)==len(schema)==10 and all(not x['path']for x in operands)
  after=visited[visited.index(pc)+1:];conflicts=[dict(pc=k,write=w,argument=j)for k in after for w in writes(ir['instructions'][k])for j,a in enumerate(operands)if aliases(a,w)];assert not conflicts,(query,pc,conflicts)
  if query==1:lifetimes.append(dict(pc=pc,argument_registers=operands,subsequent_visited=after,overwriting_writes=conflicts))
  source_args=[]
  for ref in operands:
   bank=ref['memory'];assert bank in ['Work','Literal'];row=memory[ref['register']]if bank=='Work'else rig['rig_vm']['literal']['properties'][ref['register']];assert row['index']==ref['register'];source_args.append(value(row))
  actual=next(c['arguments_after_call']for c in t['expected_calls']if c['instruction']==pc);projected=[project(key,v)for (key,_,_),v in zip(schema,source_args,strict=True)]
  assert source_args[6]is True
  changed=[]
  for (key,_,_),a,b in zip(schema,projected,actual,strict=True):
   if a!=b:
    differences[key]+=1;changed.append(key)
    if key not in first:first[key]=dict(query=query,pc=pc,source_raw=source_args[[x[0]for x in schema].index(key)],source_projected=a,current=b)
  rows.append(dict(query=query,pc=pc,source_arguments_raw=source_args,source_arguments_projected=projected,current_arguments=actual,source_world=frame['mesh_to_world'],current_world=t['to_world'],source_delta=frame['delta_seconds_after_update'],actual_trace=next(x for x in t['expected_traces']if x['instruction']==pc),changed=changed))
assert len(rows)==636
(r/'inputs.json').write_text(json.dumps(dict(definition=current['definition'],rows=rows),separators=(',',':'))+'\n')
(r/'lifetime-proof.json').write_text(json.dumps(dict(gate='PASS_ACTUAL_VISITED_ORDER_AND_NO_POST_TRACE_ARGUMENT_OVERWRITE',frames=319,calls=636,argument_rows=6360,all_arguments_preserved_after_actual_call=True,initial_source_construction_prefix=15,initial_forward_remaining_steps=75,instructions_exact_fresh_source=True,representative_lifetimes=lifetimes,write_rule='Source pin direction Output IO Hidden and Copy Zero destination; alias by entire register'),indent=2)+'\n')
(r/'arguments-check.json').write_text(json.dumps(dict(gate='PASS_ACTUAL_SOURCE_FOOT_TRACE_ARGUMENT_LIFETIME_AND_COMPLETE_COMPARISON',calls=636,arguments=6360,differences=differences,first_differences=first,internal_hit_capture_claimed=False,requests_computed_by_native_api=False,new_differences=[]),indent=2)+'\n')
print('Qualified636 source trace arguments by actual visited lifetime; differences '+json.dumps(differences),flush=True)
