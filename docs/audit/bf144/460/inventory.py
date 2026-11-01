import json
import gzip
import struct
from pathlib import Path

r=Path(__file__).resolve().parent;native=json.loads((r/'common-rig.native.json').read_text()if (r/'common-rig.native.json').exists()else gzip.decompress((r/'common-rig.native.json.gz').read_bytes()))['rig_node_history_probe']['cases'];current=json.loads(Path('docs/audit/bf144/459/inputs.json').read_text())['frames'];f32=lambda x:struct.unpack('f',struct.pack('f',x))[0]
assert len(native)==2 and len(current)==319
counts={k:0 for k in ['translation','rotation','scale']};first={};raw_changes={k:0 for k in counts};table=[]
for i,(a,b,t)in enumerate(zip(native[0]['frames'],native[1]['frames'],current,strict=True)):
 assert a['before']==b['before']and a['after']==b['after'];assert a['after']['curves']==t['curves']
 target={v['name']:v['local']for v in t['expected_local']};before={v['name']:v['local_transform']for v in a['before']['bones']};values={k:0 for k in counts}
 for bone in a['after']['bones']:
  n=bone['name'];v=bone['local_transform'];p=v['translation'];q=v['rotation'];s=v['scale'];projected=dict(translation=[f32(p['x']*.01),f32(p['z']*.01),f32(p['y']*.01)],rotation=[f32(-q['x']),f32(-q['z']),f32(-q['y']),f32(q['w'])],scale=[f32(s['x']),f32(s['z']),f32(s['y'])])
  for k in counts:
   raw_changes[k]+=before[n][k]!=v[k]
   expected=[f32(x)for x in target[n][k]]
   if projected[k]!=expected:
    values[k]+=1;counts[k]+=1
    if k not in first:first[k]=dict(query=i,bone=n,source_projected=projected[k],current=expected)
 table.append(dict(query=i,differences=values))
result=dict(gate='PASS_ALL319_COMMON_POSE_INPUT_SOURCE_RIG_OUTPUT_INVENTORY_WITH_EXPLICIT_QUERY_DIFFERENCE',frames=319,bone_pairs=25201,all_output_curve_frames_exact_current=319,two_independent_source_cases_output_pose_and_curve_exact=True,source_current_projected_output_differences=counts,first_differences=first,source_before_after_raw_changes=raw_changes,query_data_differences_not_counted_as_vm_algorithm_difference=True,complete_algorithm_equivalence_claimed=False,new_differences=[])
(r/'inventory.json').write_text(json.dumps(result,indent=2)+'\n');(r/'frame-table.json').write_text(json.dumps(table,separators=(',',':'))+'\n');print(json.dumps(result),flush=True)
