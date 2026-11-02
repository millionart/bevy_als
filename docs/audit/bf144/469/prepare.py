import gzip
import json
import sys
from pathlib import Path

sys.path.insert(0,'tools/animgraph_authoring')
from ue_control_rig_contract import compile_contract
from ue_control_rig_target import compile_target,UNIT_ARGUMENTS
r=Path(__file__).resolve().parent;rig=json.loads(gzip.decompress((r/'rig.native.json.gz').read_bytes()));c=compile_contract(rig);ir=c['program'];current=json.loads(gzip.decompress(Path('docs/audit/bf144/464/replay-restored.json.gz').read_bytes()))['modes'];definition=json.loads(Path('docs/audit/bf144/459/inputs.json').read_bytes())['definition'];assert compile_target(c)['program']['instructions']==definition['program']['instructions'];native=json.loads(gzip.decompress(Path('docs/audit/bf144/469a/restored.native.json.gz').read_bytes()))['rig_node_history_probe']['cases'][0]['frames'];controlled=json.loads(gzip.decompress(Path('docs/audit/bf144/469a/controls-restored.native.json.gz').read_bytes()))['rig_node_history_probe']['cases']
name='FAlsRigUnit_ApplyFootOffsetRotation::Execute';schema=[x.split(':')for x in UNIT_ARGUMENTS[name].split()];pcs=[i for i,op in enumerate(ir['instructions'])if op['opcode']=='Execute'and ir['functions'][op['function']]['name']==name];assert pcs==[98,128]
def writes(op):
 if op['opcode']=='Execute':
  graph,index=op['model'];pins={p['name']:p for p in c['graphs'][graph]['nodes'][index]['pins']if p['parent']is None};return[ref for arg,ref in zip(ir['functions'][op['function']]['arguments'],op['operands'],strict=True)if pins[arg['name']]['direction']in['Output','IO','Hidden']]
 if op['opcode']in['Copy','Zero']:return[op['output']]
 return[]
def fields(row):return{x['name']:x for x in row['fields']}
def value(ref,frame,kind):
 assert ref['memory']in['Work','Literal','External']and not ref['path']
 if ref['memory']=='External':row=frame['externals_after'][rig['rig_vm']['external_variables'][ref['register']]['name']]
 else:
  row=(frame['work_after']['properties']if ref['memory']=='Work'else rig['rig_vm']['literal']['properties'])[ref['register']];assert row['index']==ref['register']
 if 'slice'in ref:row=row['elements'][ref['slice']]
 if kind=='bool':return row['exported_text']=='True'
 if kind=='float':return row['numeric_value']
 f=fields(row)
 if kind in['FVector','FQuat']:return[f[k]['numeric_value']for k in('XYZ'if kind=='FVector'else'XYZW')]
 if kind=='FFloatInterval':return[f[k]['numeric_value']for k in['Min','Max']]
 if kind=='FRigElementKey':return[f['Type']['exported_text'],f['Name']['exported_text'].strip('"')]
 assert kind=='FCachedRigElement';return dict(index=int(f['Index']['exported_text']))
parents={b['name']:b['parent']for b in rig['rig_hierarchy']['bones']}
def ancestor(a,b):
 while b!='None':
  if a==b:return True
  b=parents[b]
 return False
boundary_rows=json.loads(Path('docs/audit/bf144/469a/table-restored.json').read_bytes());boundaries={(row['query'],row['pc']):row for row in boundary_rows if row['case']==0};assert len(boundaries)==636
rows=[];proof=[];foot_writes=[]
for dataset,frames in [("actual",native)]+[(case["name"],case["frames"])for case in controlled]:
 for query,frame in enumerate(frames):
  if query%64==0:print('Qualified actual rotation source frame '+str(query),flush=True)
  visits=next(x['instructions']for x in frame['executions']if x['entry']=='Forwards Solve')
  if query==0:assert not any(pc in visits for pc in pcs);continue
  hierarchy={bone['name']:bone for bone in frame['hierarchy_after']['bones']}
  for pc in pcs:
   assert visits.count(pc)==1;refs=ir['instructions'][pc]['operands'];after=visits[visits.index(pc)+1:];aliases=[dict(pc=k,argument=j)for k in after for w in writes(ir['instructions'][k])for j,ref in enumerate(refs)if ref['memory']==w['memory']and ref['register']==w['register']];assert not aliases,(query,pc,aliases)
   args=[value(ref,frame,kind)for ref,(_,kind,_)in zip(refs,schema,strict=True)];calf=args[0][1];foot=args[1][1];mutations=[]
   for k in after:
    op=ir['instructions'][k]
    if op['opcode']!='Execute':continue
    function=ir['functions'][op['function']]['name']
    if function=='FRigUnit_TwoBoneIKSimplePerItem::Execute':
     keys=[value(op['operands'][i],frame,'FRigElementKey')[1]for i in[0,1,2]];assert all(not ancestor(key,calf)for key in keys);mutations.append(dict(pc=k,function=function,keys=keys))
    elif function.startswith('FRigUnit_Set'):
     key=value(op['operands'][0],frame,'FRigElementKey')[1];assert value(op['operands'][2],frame,'bool')is False;assert not ancestor(key,calf);mutations.append(dict(pc=k,function=function,key=key))
     if key==foot:foot_writes.append(dict(query=query,pc=pc,later_writer=k,foot=foot))
   domains=[]
   for mode in(current if dataset=="actual"else[]):
    call=next(x for x in mode['outputs'][query]['calls']if x['instruction']==pc);domains.append(dict(source_trace=mode['source'],before=call['arguments_before_call'],after=call['arguments_after_call'],bones_before=call['bones_before_call']))
   boundary=next(row for row in frame["unit_boundaries"]if row["instruction"]==pc);items={item['argument']:item for item in boundary['items']};assert items['CalfItem']['current_global_transform']==hierarchy[calf]['current_global_transform'];assert items['FootItem']['initial_global_transform']==hierarchy[foot]['initial_global_transform']
   rows.append(dict(dataset=dataset,source_foot_global_at_call=items['FootItem']['current_global_transform'],source_input_stage='after_native_unit_before_next_instruction_readonly_hierarchy',query=query,pc=pc,delta_seconds=frame['delta_seconds_after_update'],source_arguments_after=args,source_calf_global=hierarchy[calf]['current_global_transform'],source_calf_initial_global=hierarchy[calf]['initial_global_transform'],source_foot_initial_global=hierarchy[foot]['initial_global_transform'],source_foot_frame_end_global_not_consumed=hierarchy[foot]['current_global_transform'],target_domains=domains))
   if query in[1,106,318]:proof.append(dict(query=query,pc=pc,arguments=refs,no_later_alias_write=True,later_pose_writers=mutations,calf_global_at_call_qualified=True,foot_global_at_call_qualified=True,foot_source='469a exact native readonly unit boundary'))
assert len(rows)==len(foot_writes)==684
(r/'inputs.json').write_text(json.dumps(dict(definition=definition,rows=rows),separators=(',',':'))+'\n');(r/'lifetime-proof.json').write_text(json.dumps(dict(gate='PASS_ACTUAL636_ROTATION_ABI_AND_EXACT_NATIVE_CONSUMED_GLOBAL_INPUTS',source_calls=684,actual_calls=636,controlled_calls=48,source_pin_values=684*14,calf_current_global_qualified=True,foot_current_global_not_qualified=False,foot_current_global_from_exact_unit_boundary=True,source_foot_after_not_used=True,late_foot_writers=foot_writes,representatives=proof),indent=2)+'\n');print('Qualified684 actual and controlled rotation states and exact native input boundaries',flush=True)
