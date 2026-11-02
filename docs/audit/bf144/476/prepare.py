import copy
import gzip
import json
import sys
from pathlib import Path

sys.path.insert(0,'tools/animgraph_authoring')
from ue_control_rig_contract import compile_contract
from ue_control_rig_target import compile_target
r=Path(__file__).resolve().parent

def read(path):
 p=Path(path)
 if not p.exists():p=Path(str(p)+'.gz')
 return json.loads(gzip.decompress(p.read_bytes())if p.suffix=='.gz'else p.read_bytes())
rig=read(r/'rig.native.json');contract=compile_contract(rig);ir=contract['program'];definition=read('docs/audit/bf144/459/inputs.json')['definition'];assert compile_target(contract)['program']['instructions']==definition['program']['instructions']
def fields(row):return{x['name']:x for x in row.get('fields',[])}
def decoded(row):
 if'fields'in row:return{k:decoded(x)for k,x in fields(row).items()}
 if'numeric_value'in row:return row['numeric_value']
 text=row['exported_text']
 if text in['True','False']:return text=='True'
 if row.get('cpp_type')in['int32','uint16','uint8']:return int(text)
 return text

def memory(ref,frame):
 assert ref['memory']in['Literal','Work']and not ref['path'];value=(rig['rig_vm']['literal']['properties']if ref['memory']=='Literal'else frame['work_after']['properties'])[ref['register']]
 return value['elements'][ref['slice']]if'slice'in ref else value

def writes(op):
 if op['opcode']=='Execute':
  graph,index=op['model'];pins={p['name']:p for p in contract['graphs'][graph]['nodes'][index]['pins']if p['parent']is None};return[ref for arg,ref in zip(ir['functions'][op['function']]['arguments'],op['operands'],strict=True)if pins[arg['name']]['direction']in['Output','IO','Hidden']]
 if op['opcode']in['Copy','Zero']:return[op['output']]
 return[]
def aliases(a,b):
 if a['memory']!=b['memory']or a['register']!=b['register']:return False
 if'slice'in a and'slice'in b and a['slice']!=b['slice']:return False
 x=a['path'];y=b['path'];return x[:min(len(x),len(y))]==y[:min(len(x),len(y))]
rows=[];pins=0;changed=0
for dataset,legacy_path in[('actual','docs/audit/bf144/469a/restored.native.json.gz'),('weights','docs/audit/bf144/473/weights.native.json.gz')]:
 before=read(r/(dataset+'-before.native.json'))['rig_node_history_probe']['cases'];after=read(r/(dataset+'-after.native.json'))['rig_node_history_probe']['cases'];legacy=read(legacy_path)['rig_node_history_probe']['cases']
 for ci,(bc,ac,lc)in enumerate(zip(before,after,legacy,strict=True)):
  for qi,(bf,af,lf)in enumerate(zip(bc['frames'],ac['frames'],lc['frames'],strict=True)):
   if qi==0:continue
   visited=next(x['instructions']for x in bf['executions']if x['entry']=='Forwards Solve');by_before={x['instruction']:x for x in bf['unit_boundaries']};by_after={x['instruction']:x for x in af['unit_boundaries']};by_rotation={x['instruction']:x for x in lf['unit_boundaries']};hierarchy={x['name']:x for x in bf['hierarchy_after']['bones']}
   for pre,pc,rotation in[(86,91,98),(116,121,128)]:
    a=by_before[pre];b=by_after[pc];assert a['function']=='FAlsRigUnit_CalculatePoleVector::Execute'and b['function']=='FRigUnit_TwoBoneIKSimplePerItem::Execute';assert all(ir['instructions'][j]['opcode']!='Execute'or ir['functions'][ir['instructions'][j]['function']]['name']in['FRigVMFunction_MathVectorScale::Execute','FRigVMFunction_MathVectorAdd::Execute','FAlsRigVMFunction_DamperExactVector::Execute']for j in visited[visited.index(pre)+1:visited.index(pc)])
    parameters=ir['functions'][ir['instructions'][pc]['function']]['arguments'];refs=ir['instructions'][pc]['operands'];later=visited[visited.index(pc)+1:];assert not[(i,j)for j in later for w in writes(ir['instructions'][j])for i,ref in enumerate(refs)if aliases(ref,w)],(ci,qi,pc)
    args=[decoded(memory(ref,bf))for ref in refs];assert len(args)==22 and args[14]==args[15]==0 and args[16]is True;assert args[9]==dict(Type='Bone',Name='None');assert args[17]['bEnabled']is False
    names=[args[i]['Name']for i in range(3)];assert names==[x['name']for x in a['items']]==[x['name']for x in b['items']];assert all(x['argument']==name for x,name in zip(a['items'],['ItemA','ItemB','ItemC'],strict=True));assert all(x['argument']==name for x,name in zip(b['items'],['ItemA','ItemB','EffectorItem'],strict=True))
    pre_bones=[x['current_global_transform']for x in a['items']];post_bones=[x['current_global_transform']for x in b['items']];initial=[x['initial_global_transform']for x in a['items']];assert initial==[x['initial_global_transform']for x in b['items']]==[hierarchy[name]['initial_global_transform']for name in names]
    assert post_bones[:2]==[hierarchy[name]['current_global_transform']for name in names[:2]];consumer={x['argument']:x for x in by_rotation[rotation]['items']};assert post_bones[1]==consumer['CalfItem']['current_global_transform']and post_bones[2]==consumer['FootItem']['current_global_transform'];different=pre_bones!=post_bones;changed+=different;pins+=len(args)
    elements=bf['hierarchy_after']['elements']
    for i,name in zip(range(18,21),names,strict=True):assert args[i]['Key']==dict(Type='Bone',Name=name)and elements[args[i]['Index']]['name']==name
    assert args[21]['Index']==65535
    rows.append(dict(dataset=dataset,case=ci,case_name=bc['name'],query=qi,pc=pc,pre_pc=pre,consumer_pc=rotation,names=names,arguments_after_native_call=args,ordinary_arguments_no_later_alias=True,hidden_cache_after_call_not_fabricated_before_state=True,before=pre_bones,after=post_bones,initial=initial,changed_by_ik=different,weight=args[13],automatic_lengths_requested=True))
assert len(rows)==1332 and pins==29304
current=read(r/'current-calls.json');assert current['legs']==1272
(r/'inputs.json').write_text(json.dumps(dict(definition=definition,source=rows,current=[x for x in current['rows']if x['pc']in[91,121]]),separators=(',',':'))+'\n');(r/'source-boundary-check.json').write_text(json.dumps(dict(gate='PASS_EXACT_NATIVE_IK_PRE_POST22_PIN_LIFETIME_AND_DOWNSTREAM_CONSUMER',calls=len(rows),pins=pins,source_changed_calls=changed,source_hidden_after_state_only_not_falsely_before=True,pre_pole_readonly_to_ik_no_pose_writer=True,post_calf_and_foot_exact_downstream_consumer=True,automatic_lengths_requested_all=True,pole_space_invalid_all=True),indent=2)+'\n');print('Qualified1332 pre/post IK calls,29304 native pins and exact downstream calf/foot consumer boundaries',flush=True)
