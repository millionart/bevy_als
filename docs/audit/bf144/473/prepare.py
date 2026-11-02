import gzip
import json
import sys
from pathlib import Path

sys.path.insert(0,'tools/animgraph_authoring')
from ue_control_rig_contract import compile_contract
from ue_control_rig_target import compile_target
def read(path):
 p=Path(path)
 if not p.exists():p=Path(str(p)+'.gz')
 return json.loads(gzip.decompress(p.read_bytes())if p.suffix=='.gz'else p.read_bytes())
r=Path(__file__).resolve().parent;rig=read(r/'rig.native.json');c=compile_contract(rig);ir=c['program'];definition=json.loads(Path('docs/audit/bf144/459/inputs.json').read_bytes())['definition'];assert compile_target(c)['program']['instructions']==definition['program']['instructions'];rows=[]
def fields(row):return{x['name']:x for x in row['fields']}
def value(ref,frame):
 assert ref['memory']in['Work','Literal']and not ref['path'];row=(frame['work_after']['properties']if ref['memory']=='Work'else rig['rig_vm']['literal']['properties'])[ref['register']]
 if'slice'in ref:row=row['elements'][ref['slice']]
 return row
def writes(op):
 if op['opcode']=='Execute':
  graph,index=op['model'];pins={p['name']:p for p in c['graphs'][graph]['nodes'][index]['pins']if p['parent']is None};return[ref for arg,ref in zip(ir['functions'][op['function']]['arguments'],op['operands'],strict=True)if pins[arg['name']]['direction']in['Output','IO','Hidden']]
 if op['opcode']in['Copy','Zero']:return[op['output']]
 return[]
def no_alias(refs,visits):
 return not any(ref['memory']==write['memory']and ref['register']==write['register']for pc in visits for write in writes(ir['instructions'][pc])for ref in refs)
def key(ref,frame):
 f=fields(value(ref,frame));assert f['Type']['exported_text']=='Bone';return f['Name']['exported_text']
lifetimes=[]
sources=[('actual',Path('docs/audit/bf144/469a/restored.native.json.gz')),('weights',r/'weights.native.json')]
for kind,path in sources:
 data=read(path)['rig_node_history_probe']['cases']
 for case in(data[:1]if kind=='actual'else data):
  for query,frame in enumerate(case['frames']):
   hierarchy={bone['name']:bone for bone in frame['hierarchy_after']['bones']}
   for boundary in frame['unit_boundaries']:
    pc=boundary['instruction'];setter=100 if pc==98 else 130;refs=ir['instructions'][setter]['operands'];foot=next(x for x in boundary['items']if x['argument']=='FootItem');row=value(refs[3],frame);q=fields(row);rotation=[q[k]['numeric_value']for k in'XYZW'];weight=value(refs[4],frame)['numeric_value'];assert value(refs[1],frame)['exported_text']=='GlobalSpace'and value(refs[2],frame)['exported_text']=='False';before=foot['current_global_transform'];after=hierarchy[foot['name']]['current_global_transform'];assert before['translation']==after['translation']and before['scale']==after['scale']
    visits=next(x['instructions']for x in frame['executions']if x['entry']=='Forwards Solve');assert visits.count(setter)==1 and visits.count(pc)==1;after_visits=visits[visits.index(setter)+1:];assert no_alias(refs,after_visits);assert refs[3]==ir['instructions'][pc]['operands'][12]and refs[0]==ir['instructions'][pc]['operands'][1];assert not any(ir['instructions'][j]['opcode']=='Execute'for j in visits[visits.index(pc)+1:visits.index(setter)]);ancestors=set();name=foot['name']
    while name!='None':ancestors.add(name);name=hierarchy[name]['parent']
    later_writers=[]
    for j in after_visits:
     op=ir['instructions'][j]
     if op['opcode']!='Execute':continue
     fn=ir['functions'][op['function']]['name'];indices=[0]if fn=='FRigUnit_SetRotation::Execute'else[0,1,2]if fn=='FRigUnit_TwoBoneIKSimplePerItem::Execute'else[]
     for i in indices:
      assert no_alias([op['operands'][i]],visits[visits.index(j)+1:]);name=key(op['operands'][i],frame);assert name not in ancestors;later_writers.append(dict(pc=j,function=fn,bone=name))
    lifetimes.append(dict(dataset=kind,case=case['name'],query=query,pc=setter,unit_before=pc,intermediate_pose_writes=0,setter_pin_late_alias_writes=0,later_writers=later_writers,foot_and_ancestors=sorted(ancestors)))
    rows.append(dict(dataset=kind,case=case['name'],query=query,pc=setter,rotation_pc=pc,foot=foot['name'],before=before,value=rotation,weight=weight,expected_after=after,requested_case_weight=float(case['name'].rsplit('_',1)[1])if kind=='weights'else None))
assert sum(row['dataset']=='actual'for row in rows)==636 and sum(row['dataset']=='weights'for row in rows)==60
(r/'inputs.json').write_text(json.dumps(dict(definition=definition,rows=rows),separators=(',',':'))+'\n');print('Prepared636 actual and60 source zero/partial/full SetRotation writes with exact pre/post globals',flush=True)

current=read(r/'replay-baseline.json');old=read('docs/audit/bf144/464/replay-restored.json.gz');assert len(current['modes'])==2
actual=[];native={ (row['query'],row['pc']):row for row in rows if row['dataset']=='actual'}
for mode in[0,1]:
 for frame,prior in zip(current['modes'][mode]['outputs'],old['modes'][mode]['outputs'],strict=True):
  for call in frame['calls']:
   if call['function']=='FRigUnit_SetRotation::Execute':
    args=call['arguments_before_call'];assert args[1]=={'Name':'GlobalSpace'}and args[2]=={'Bool':False}and args[4]=={'F32':1.0};assert len(call['bones_before_call'])==1
    actual.append(dict(mode=mode,query=frame['query'],pc=call['instruction'],foot=args[0]['List'][1]['Name'],before=call['bones_before_call'][0][1],value=args[3]['Quaternion'],weight=args[4]['F32'],observed=call['bone_after_call'],source=native[(frame['query'],call['instruction'])]))
  stripped=json.loads(json.dumps(frame))
  for call in stripped['calls']:del call['bone_after_call']
  assert stripped==prior
assert len(actual)==1272
(r/'actual.json').write_text(json.dumps(dict(definition=definition,rows=actual),separators=(',',':'))+'\n');(r/'lifetime.json').write_text(json.dumps(dict(calls=len(lifetimes),rows=lifetimes,all_actual_vm_registers_calls_local_and_trace_histories_unchanged=True),separators=(',',':'))+'\n');print('Qualified696 source lifetimes,no late aliases/no later foot-ancestor writers,and1272 current cached global writes with complete legacy VM histories',flush=True)

assert all(row['weight']==(row['requested_case_weight']if row['query']==6 else 1)for row in rows if row['dataset']=='weights')
