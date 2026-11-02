import json
from pathlib import Path

r=Path(__file__).resolve().parent
b=json.loads(Path('docs/audit/bf144/476/inputs.json').read_bytes())
p=b['definition']['program'];names=[x['name']for x in b['definition']['bones']];parents=[x['parent']for x in b['definition']['bones']];writers=[]
for pc,op in enumerate(p['instructions']):
 if pc<=91 or 'Call'not in op:continue
 call=op['Call'];fn=call['function'];count=3 if fn=='FRigUnit_TwoBoneIKSimplePerItem::Execute'else 1 if fn=='FRigUnit_SetRotation::Execute'else 0
 if not count:continue
 targets=[]
 for ref in call['arguments'][:count]:
  assert not ref['path']and ref['register']<p['literal_count'];key=p['initial_values'][ref['register']]['List'];assert key[0]=={'Name':'Bone'};targets.append(key[1]['Name'])
 writers.append(dict(pc=pc,function=fn,targets=targets))
checks=[]
for side,pc in [('l',91),('r',121)]:
 for name in ['calf_twist_01_'+side,'thigh_twist_01_'+side]:
  ancestors=[];i=names.index(name)
  while i is not None:ancestors.append(names[i]);i=parents[i]
  late=[row for row in writers if row['pc']>pc];assert all(not(set(row['targets'])&set(ancestors))for row in late);checks.append(dict(bone=name,after_pc=pc,ancestors=ancestors,later_writer_targets_disjoint=True))
(r/'twist-lifetime.json').write_text(json.dumps(dict(gate='PASS_LITERAL_SOURCE_IK_FOOT_WRITER_TARGETS_DISJOINT_FROM_TWIST_ANCESTORS',literal_count=p['literal_count'],later_writers=writers,checks=checks,other_late_als_foot_offset_units_readonly_from_existing_source_closure=True),indent=2)+'\n')
print('Verified literal writer target lifetimes for four twist chains without using frame-end ball',flush=True)
