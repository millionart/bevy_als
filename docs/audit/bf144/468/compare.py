import json
import math
import struct
import sys
from pathlib import Path

r=Path(__file__).resolve().parent;data=json.loads((r/'inputs.json').read_bytes());modes=json.loads((r/'replay-baseline.json').read_bytes())['modes'];f32=lambda x:struct.unpack('f',struct.pack('f',x))[0]
def projected(v):return[v[0]*.01,v[2]*.01,v[1]*.01]
def clamp(point,origin,maximum,source_dot=False,reciprocal=False):
 leg=[a-b for a,b in zip(point,origin,strict=True)];squared=(leg[0]*leg[0]+leg[2]*leg[2])+leg[1]*leg[1]if source_dot else(leg[0]*leg[0]+leg[1]*leg[1])+leg[2]*leg[2]
 if squared>maximum*maximum:
  scale=maximum*(1.0/math.sqrt(squared))if reciprocal else maximum/math.sqrt(squared);leg=[x*scale for x in leg]
 return[a+b for a,b in zip(origin,leg,strict=True)]
counts={name:dict(double_differences=0,f32_differences=0)for name in['actual_original','actual_trace_control','same_cm_public','same_cm_source_reciprocal','common_meter','source_offset','source_maximum','offset_and_maximum','offset_maximum_source_dot','offset_maximum_source_reciprocal','offset_maximum_source_dot_and_reciprocal','source_prepoint_maximum_dot_reciprocal']};table=[];first={};first_f32={};partitions={name:dict(recovered=0,regressed=0,still_different=0)for name in counts}
for raw,*rows in zip(data['rows'],*[mode['outputs']for mode in modes],strict=True):
 query=raw['query'];pc=raw['pc'];assert all((x['query'],x['pc'])==(query,pc)for x in rows);src=raw['source_arguments_after'];native=src[16];expected=projected(native);cm=rows[2];meter=rows[3];origin_cm=raw['source_thigh_translation_cm'];offset_cm=raw['predicted_source_offset_cm'];point_cm=[src[2][0],src[2][1],src[2][2]+offset_cm];max_cm=f32(src[5]*src[7]);cm_original=clamp(point_cm,origin_cm,max_cm);assert cm_original==cm['after'][16]['Vector'];cm_restored=clamp(point_cm,origin_cm,max_cm,reciprocal=True);assert cm_restored==native,(query,pc)
 origin=projected(origin_cm);target=meter['after'][2]['Vector'];offset=meter['after'][15]['F32'];maximum=f32(meter['after'][5]['F32']*meter['after'][7]['F32']);source_offset=f32(offset_cm*.01);source_max=max_cm*.01
 state=meter['after'][14]['List'];expected_state=raw['expected_common_meter_scalar']['state'];assert[f32(state[0]['F32']),f32(state[1]['F32']),state[2]['Bool']]==[f32(expected_state[0]),f32(expected_state[1]),expected_state[2]],(query,pc)
 for mode_row in[cm,meter]:
  if query==1:assert mode_row['before'][11]=={'Bool':False}and mode_row['before'][14]=={'List':[{'F32':0.0},{'F32':0.0},{'Bool':False}]}and mode_row['before'][15]=={'F32':0.0}
 point=[target[0],target[1]+offset,target[2]];source_point=[target[0],target[1]+source_offset,target[2]];original=clamp(point,origin,maximum);assert original==meter['after'][16]['Vector'],(query,pc)
 variants=dict(actual_original=rows[0]['after'][16]['Vector'],actual_trace_control=rows[1]['after'][16]['Vector'],same_cm_public=projected(cm_original),same_cm_source_reciprocal=projected(cm_restored),common_meter=original,source_offset=clamp(source_point,origin,maximum),source_maximum=clamp(point,origin,source_max),offset_and_maximum=clamp(source_point,origin,source_max),offset_maximum_source_dot=clamp(source_point,origin,source_max,True,False),offset_maximum_source_reciprocal=clamp(source_point,origin,source_max,False,True),offset_maximum_source_dot_and_reciprocal=clamp(source_point,origin,source_max,True,True),source_prepoint_maximum_dot_reciprocal=clamp(projected(point_cm),origin,source_max,True,True))
 changed={}
 for name,value in variants.items():
  different=value!=expected;projected_different=[f32(x)for x in value]!=[f32(x)for x in expected];counts[name]['double_differences']+=different;counts[name]['f32_differences']+=projected_different;changed[name]=dict(double=different,f32=projected_different)
  if projected_different and name not in first_f32:first_f32[name]=dict(query=query,pc=pc,actual=value,expected=expected)
  base_bad=[f32(x)for x in original]!=[f32(x)for x in expected];partitions[name]['recovered']+=base_bad and not projected_different;partitions[name]['regressed']+=not base_bad and projected_different;partitions[name]['still_different']+=base_bad and projected_different
  if different and name not in first:first[name]=dict(query=query,pc=pc,actual=value,expected=expected,source_cm=native,common_numeric_cm_actual=cm_original,common_numeric_cm_expected=cm_restored,source_leg_clamp_active=raw['source_leg_clamp_active'])
 table.append(dict(query=query,pc=pc,expected=expected,maximum_meter=maximum,source_maximum_projected=source_max,source_cm_offset=offset_cm,common_meter_offset=offset,values=variants,changed=changed))
assert counts['same_cm_public']['double_differences']==37 and counts['same_cm_source_reciprocal']['double_differences']==0
if len(sys.argv)>1 and sys.argv[1]=='current-contract':assert counts['same_cm_public']['double_differences']==0,first['same_cm_public']
(r/'comparison.json').write_text(json.dumps(dict(gate='PASS_ACTUAL636_FULL_FOOT_LOCATION_NATIVE_REFERENCE_AND_SAME_CM_VECTOR_RECIPROCAL_FACTOR',calls=636,source_clamp_active=121,source_complete_vector_exact=636,current_original_and_trace_control_full_arguments_exact=1272,current_common_cm_scalar_state_exact=636,current_common_meter_state_exact=636,counts=counts,first=first,first_f32=first_f32,f32_partition_against_common_meter=partitions,known_algorithm_difference='D249',full_rig_equivalence_claimed=False,new_differences=[]),indent=2)+'\n');(r/'comparison-table.json').write_text(json.dumps(table,separators=(',',':'))+'\n');print(json.dumps(counts),flush=True)
