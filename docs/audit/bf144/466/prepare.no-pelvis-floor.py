import gzip
import json
import struct
import sys
from pathlib import Path

sys.path.insert(0,'tools/animgraph_authoring')
from ue_control_rig_contract import compile_contract
from ue_control_rig_target import compile_target,UNIT_ARGUMENTS
r=Path(__file__).resolve().parent
rig=json.loads((r/'rig.native.json').read_bytes()if(r/'rig.native.json').exists()else gzip.decompress((r/'rig.native.json.gz').read_bytes()))
contract=compile_contract(rig);ir=contract['program'];definition=json.loads(Path('docs/audit/bf144/459/inputs.json').read_bytes())['definition'];assert compile_target(contract)['program']['instructions']==definition['program']['instructions']
native=json.loads(gzip.decompress(Path('docs/audit/bf144/465a/restored.native.json.gz').read_bytes()))['rig_node_history_probe']['cases'][0]['frames']
actual=json.loads(gzip.decompress(Path('docs/audit/bf144/464/replay-restored.json.gz').read_bytes()))['modes']
f32=lambda x:struct.unpack('f',struct.pack('f',x))[0]
def writes(op):
 if op['opcode']=='Execute':
  graph,index=op['model'];node=contract['graphs'][graph]['nodes'][index];pins={p['name']:p for p in node['pins']if p['parent']is None}
  return[ref for arg,ref in zip(ir['functions'][op['function']]['arguments'],op['operands'],strict=True)if pins[arg['name']]['direction']in['Output','IO','Hidden']]
 if op['opcode']in['Copy','Zero']:return[op['output']]
 assert op['opcode']in['Jump','JumpToBranch','Exit'],op['opcode']
 return[]
def aliases(a,b):return a['memory']==b['memory']and a['register']==b['register']
def fields(row):return{x['name']:x for x in row['fields']}
def extract(row,kind):
 if kind=='bool':return row['exported_text']=='True'
 if kind=='float':return row['numeric_value']
 fs=fields(row)
 if kind=='FVector':return[fs[k]['numeric_value']for k in['X','Y','Z']]
 if kind=='FRigElementKey':return[fs['Type']['exported_text'],fs['Name']['exported_text'].strip('"')]
 if kind=='FAlsSpringFloatState':return[fs['Velocity']['numeric_value'],fs['PreviousTarget']['numeric_value'],fs['bStateValid']['exported_text']=='True']
 assert kind=='FCachedRigElement',kind
 return dict(key=extract(fs['Key'],'FRigElementKey'),index=int(fs['Index']['exported_text']))
def value(ref,frame,kind):
 assert not ref['path']
 if ref['memory']=='External':
  assert ref['register']==11 and kind=='float';return frame['externals_after']['PelvisOffset']['numeric_value']
 row=(frame['work_after']['properties']if ref['memory']=='Work'else rig['rig_vm']['literal']['properties'])[ref['register']];assert row['index']==ref['register']
 if 'slice'in ref:row=row['elements'][ref['slice']]
 return extract(row,kind)
parents={b['name']:b['parent']for b in rig['rig_hierarchy']['bones']};ancestors={'pelvis'};name=parents['pelvis']
while name!='None':ancestors.add(name);name=parents[name]
location='FAlsRigUnit_ApplyFootOffsetLocation::Execute';schema=[x.split(':')for x in UNIT_ARGUMENTS[location].split()];pcs=[i for i,o in enumerate(ir['instructions'])if o['opcode']=='Execute'and ir['functions'][o['function']]['name']==location];assert pcs==[83,113]
focused_height_bounds=[f32(1+sign*f32(.0001))for sign in[-1,1]]
assert max(min(1,focused_height_bounds[0]),-2)!=max(min(1,focused_height_bounds[1]),-2)
if len(sys.argv)>1 and sys.argv[1]=='bad-lifetime':
 refs=ir['instructions'][83]['operands'];writer=next(i for i,op in enumerate(ir['instructions'])if i<83 and any(aliases(ref,w)for ref in refs for w in writes(op)))
 event=next(x for x in native[1]['executions']if x['entry']=='Forwards Solve');event['instructions'][-1]=writer;print('Injected controlled post-foot alias writer '+str(writer),flush=True)
rows=[];proof=[];margins=[];upper_active=0
for query,frame in enumerate(native):
 if query%64==0:print('Qualified foot location source frame '+str(query),flush=True)
 visits=next(x['instructions']for x in frame['executions']if x['entry']=='Forwards Solve')
 if query==0:
  assert not any(pc in visits for pc in pcs);continue
 assert all(visits.count(pc)==1 for pc in pcs)
 writer=ir['instructions'][66];assert ir['functions'][writer['function']]['name']=='FRigUnit_SetTranslation::Execute'
 for ref in writer['operands']:
  assert not any(aliases(ref,w)for k in visits[visits.index(66)+1:]for w in writes(ir['instructions'][k])),(query,ref)
 assert value(writer['operands'][0],frame,'FRigElementKey')==['Bone','pelvis']
 assert value(writer['operands'][2],frame,'bool')is False and value(writer['operands'][4],frame,'float')==1
 space=rig['rig_vm']['literal']['properties'][writer['operands'][1]['register']]['exported_text'];assert space=='GlobalSpace',space
 desired=value(writer['operands'][3],frame,'FVector')[2]
 mutators=[]
 for k in visits[visits.index(66)+1:]:
  op=ir['instructions'][k]
  if op['opcode']!='Execute':continue
  function=ir['functions'][op['function']]['name']
  if function.startswith('FRigUnit_Set')or function=='FRigUnit_TwoBoneIKSimplePerItem::Execute':
   indices=[0,1,2]if function=='FRigUnit_TwoBoneIKSimplePerItem::Execute'else[0]
   keys=[value(op['operands'][i],frame,'FRigElementKey')for i in indices];assert all(key[1]not in ancestors for key in keys),(query,k,keys)
   mutators.append(dict(pc=k,function=function,keys=keys))
 for pc in pcs:
  refs=ir['instructions'][pc]['operands'];after=visits[visits.index(pc)+1:];conflicts=[dict(pc=k,argument=j,write=w)for k in after for w in writes(ir['instructions'][k])for j,ref in enumerate(refs)if aliases(ref,w)];assert not conflicts,(query,pc,conflicts)
  args=[value(ref,frame,kind)for ref,(_,kind,_)in zip(refs,schema,strict=True)]
  upper_offsets=[f32(f32(f32(desired+sign*f32(.0001))-args[6])-f32(args[2][2]))for sign in[-1,1]]
  targets=[min(args[3],bound)for bound in upper_offsets];assert targets[0]==targets[1],(query,pc,upper_offsets,targets)
  target=targets[0];margin=upper_offsets[0]-args[3];margins.append(margin);upper_active+=margin<=0
  assert target==args[14][1]if args[14][2]else args[15]==target
  assert args[12]['key']==args[0]and args[12]['index']>=0 and args[13]['key']==args[1]and args[13]['index']>=0
  domains=[]
  for mode in actual:
   call=next(x for x in mode['outputs'][query]['calls']if x['instruction']==pc);domains.append(dict(source_trace=mode['source'],before=call['arguments_before_call'],after=call['arguments_after_call'],bones_before=call['bones_before_call']))
  rows.append(dict(query=query,pc=pc,delta_seconds=frame['delta_seconds_after_update'],source_arguments_after=args,source_target_cm=target,source_pelvis_desired_height_cm=desired,source_height_upper_offset_interval_cm=upper_offsets,source_target_interval_cm=targets,target_domains=domains))
  if query in[1,19,106,318]:proof.append(dict(query=query,pc=pc,refs=refs,after_visited=after,overwriting_writes=conflicts,later_hierarchy_mutators=mutators))
assert len(rows)==636
(r/'inputs.json').write_text(json.dumps(dict(definition=definition,rows=rows),separators=(',',':'))+'\n');(r/'lifetime-proof.json').write_text(json.dumps(dict(gate='PASS_636_FOOT_LOCATION_ARGUMENT_LIFETIME_AND_PROVEN_UNIQUE_SCALAR_TARGET_FROM_HEIGHT_BOUNDS',source_pcs=pcs,argument_values=636*17,pelvis_writer=66,pelvis_ancestors=sorted(ancestors),source_hierarchy_translation_skip_tolerance_cm=f32(.0001),source_upper_clamp_minimum_margin_cm=min(margins),source_upper_clamp_may_be_active_rows=upper_active,source_target_interval_collapsed_all_rows=True,focused_ambiguous_height_case_refused=True,source_exact_pelvis_global_read_claimed=False,source_scalar_target_independent_of_height_with_proven_margin=True,representatives=proof),indent=2)+'\n');print('Qualified636 source foot location arguments and actual unique scalar target from conservative hierarchy bounds',flush=True)
