import copy
import gzip
import json
from pathlib import Path

r=Path(__file__).resolve().parent;v=json.loads((r/'replay-baseline.json').read_bytes()if(r/'replay-baseline.json').exists()else gzip.decompress((r/'replay-baseline.json.gz').read_bytes()));old=json.loads(gzip.decompress(Path('docs/audit/bf144/473/replay-baseline.json.gz').read_bytes()));rows=[]
for mode in[0,1]:
 for f,p in zip(v['modes'][mode]['outputs'],old['modes'][mode]['outputs'],strict=True):
  clean=copy.deepcopy(f)
  for call in clean['calls']:call.pop('ik_after_call')
  assert clean==p
  for call in f['calls']:
   if call['function']=='FRigUnit_TwoBoneIKSimplePerItem::Execute':
    assert len(call['arguments_before_call'])==22 and len(call['bones_before_call'])>=3 and len(call['ik_after_call'])==3
    rows.append(dict(mode=mode,query=f['query'],pc=call['instruction'],arguments_before=call['arguments_before_call'],arguments_after=call['arguments_after_call'],bones_before=call['bones_before_call'],bones_after=call['ik_after_call']))
assert len(rows)==2548 and sum(x['pc']in[91,121]for x in rows)==1272
(r/'current-calls.json').write_text(json.dumps(dict(gate='PASS_ALL_CURRENT_VM_IK_BOUNDARIES_WITH_COMPLETE_OLD_HISTORIES',legs=1272,all_calls=2548,rows=rows),separators=(',',':'))+'\n');print('Verified2548 cloned post-IK observations and full original/trace-control VM register/pose/call/trace histories,1272 active legs',flush=True)
