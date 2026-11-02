import gzip
import json
import math
import struct
import sys
from pathlib import Path

sys.path.insert(0,'tools/animgraph_authoring')
from ue_control_rig_contract import compile_contract
from ue_control_rig_target import compile_target
r=Path(__file__).resolve().parent;rig=json.loads(gzip.decompress((r/'rig.native.json.gz').read_bytes()));program=compile_target(compile_contract(rig))['program'];base=json.loads(Path('docs/audit/bf144/466/inputs.json').read_bytes());assert program['instructions']==base['definition']['program']['instructions']
proof=json.loads(Path('docs/audit/bf144/467/check-restored.json').read_bytes());assert proof['thigh_translation_call_fields_qualified']==1272 and proof['complete_global_snapshot_exact_prototype']
tables=json.loads(Path('docs/audit/bf144/467/table-restored.json').read_bytes());native={(x['case'],x['query']):{b['name']:b for b in x['bones']}for x in tables};scalar=json.loads(Path('docs/audit/bf144/466/replay-baseline.json').read_bytes())['domains'];cm={(x['query'],x['pc']):x for x in scalar[2]['outputs']};meter={(x['query'],x['pc']):x for x in scalar[3]['outputs']};f32=lambda x:struct.unpack('f',struct.pack('f',x))[0]
rows=[];active=0;different=0;first=None
for row in base['rows']:
 query=row['query'];pc=row['pc'];source=row['source_arguments_after'];key=(query,pc);name=source[1][1];a=native[(0,query)];b=native[(1,query)];assert a==b;thigh=a[name]['global_transform']['translation'];pelvis=a['pelvis']['global_transform']['translation'];offset=cm[key]['after']['offset'];assert offset==source[15]
 target=source[2];foot=[target[0],target[1],target[2]+offset];origin=[thigh[k]for k in'xyz'];leg=[x-y for x,y in zip(foot,origin,strict=True)];maximum=f32(source[5]*source[7]);squared=(leg[0]*leg[0]+leg[1]*leg[1])+leg[2]*leg[2];is_active=squared>maximum*maximum;active+=is_active
 clamped=[v*(maximum*(1.0/math.sqrt(squared)))for v in leg]if is_active else leg;output=[x+y for x,y in zip(origin,clamped,strict=True)];exact=output==source[16];different+=not exact
 if not exact and first is None:first=dict(query=query,pc=pc,actual=source[16],predicted=output,clamped=is_active)
 rows.append(dict(**row,source_thigh_translation_cm=origin,source_pelvis_translation_cm=[pelvis[k]for k in'xyz'],predicted_source_offset_cm=offset,expected_common_meter_scalar=meter[key]['after'],source_leg_clamp_active=is_active,source_vector_prediction_cm=output))
assert len(rows)==636
(r/'inputs.json').write_text(json.dumps(dict(definition=base['definition'],rows=rows),separators=(',',':'))+'\n');(r/'source-inventory.json').write_text(json.dumps(dict(gate='ACTUAL636_FOOT_VECTOR_INITIAL_SOURCE_MODEL',calls=636,source_clamp_active=active,source_complete_vector_differences=different,first_difference=first,offsets_independently_computed_in_466=True,thigh_translation_qualified_in_467=True,other_thigh_transform_fields_not_used=True),indent=2)+'\n');print(json.dumps(dict(calls=636,clamp_active=active,source_vector_differences=different,first=first)),flush=True)
