import gzip
import json
from pathlib import Path

r=Path(__file__).resolve().parent

def read(name):
 p=r/name
 if not p.exists():p=r/(name+'.gz')
 return json.loads(gzip.decompress(p.read_bytes())if p.suffix=='.gz'else p.read_bytes())

def before_contract(row):
 assert row['function']=='FAlsRigUnit_CalculatePoleVector::Execute','Before-IK sample is not from the readonly source PoleVector unit'
 assert row['instruction']in[86,116],'Before-IK source instruction is invalid'
 assert row['stage']=='after_native_unit_before_next_instruction'
 assert[item['argument']for item in row['items']]==['ItemA','ItemB','ItemC']
 assert all(item['type']=='Bone'and item['valid_transform_item']for item in row['items'])

positive=0;negative=0;first=None
for kind in['actual','weights']:
 before=read(kind+'-before.native.json')['rig_node_history_probe']['cases'];after=read(kind+'-after.native.json')['rig_node_history_probe']['cases']
 for ci,(a,b)in enumerate(zip(before,after,strict=True)):
  for qi,(frame,post)in enumerate(zip(a['frames'],b['frames'],strict=True)):
   for boundary in frame['unit_boundaries']:before_contract(boundary);positive+=1
   for boundary in post['unit_boundaries']:
    if boundary['instruction']not in[91,121]:continue
    try:before_contract(boundary)
    except AssertionError as error:
     negative+=1
     if first is None:first=dict(dataset=kind,case=ci,query=qi,pc=boundary['instruction'],function=boundary['function'],reason=str(error),typed_arguments=[item['argument']for item in boundary['items']])
    else:raise AssertionError('Post-IK source boundary falsely accepted as consumed pre-IK input')
assert positive==negative==1332
result=dict(gate='PASS_NATIVE_PRE_IK_TEMPORAL_IDENTITY_CONTRACT_REJECTS_POST_IK_INPUTS',accepted_exact_pre_boundaries=positive,rejected_post_boundaries=negative,first_rejection=first,final_output_idempotence_not_used_to_prove_input_time=True,solver_mathematics_not_claimed=True)
(r/'stage-contract.json').write_text(json.dumps(result,indent=2)+'\n');print('Verified1332 typed native pre-stage identities and1332 rejected post-stage substitutions',flush=True)
