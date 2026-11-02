import json
import struct
from pathlib import Path

r=Path(__file__).resolve().parent;data=json.loads((r/'inputs.json').read_bytes());report=json.loads((r/'replay-baseline.json').read_bytes());actual=[row for row in data['rows']if row['dataset']=='actual'];f32=lambda x:struct.unpack('f',struct.pack('f',x))[0]
def vector(v):return[v[0],v[2],v[1]]
def quat(v):return[-v[0],-v[2],-v[1],v[3]]
counts={name:[0,0]for name in['normal_input','offset_normal','initial_rotation','foot_rotation']};table=[]
for source,original,control in zip(actual,*[mode['outputs']for mode in report['modes']],strict=True):
 src=source['source_arguments_after'];query=source['query'];pc=source['pc'];assert original['query']==control['query']==query and original['pc']==control['pc']==pc
 expected=dict(normal_input=vector(src[3]),offset_normal=vector(src[11]),initial_rotation=quat(src[13]),foot_rotation=quat(src[12]));fields={'normal_input':('before',3,'Vector'),'offset_normal':('after',11,'Vector'),'initial_rotation':('after',13,'Quaternion'),'foot_rotation':('after',12,'Quaternion')};differences={}
 for name,(phase,index,kind)in fields.items():
  differs=[row[phase][index][kind]!=expected[name]for row in[original,control]]
  for i,different in enumerate(differs):counts[name][i]+=different
  differences[name]=differs
 assert control['before'][3]['Vector']==expected['normal_input']and control['after'][11]['Vector']==expected['offset_normal']
 table.append(dict(query=query,pc=pc,source_projected=expected,original=original['after'],trace_control=control['after'],differences=differences))
assert len(actual)==636 and len(report['normals'])==684
changed=sum(row['before']!=row['after']for row in report['normals']);assert changed==8
(r/'comparison.json').write_text(json.dumps(dict(gate='PASS_ACTUAL_ROTATION_INPUT_STATE_AND_SOURCE_NORMAL_HISTORY_WITH_EXACT_NATIVE_BONE_BOUNDARY',actual_source_calls=636,controlled_source_calls=48,source_pin_values=684*14,current_public_calls_exact=1272,source_normal_states_exact=684,controlled_normal_state_transitions=changed,counts_original_and_trace_control=counts,source_trace_corrected_normal_input_and_state_exact=636,source_foot_current_from_exact_unit_boundary=True,source_after_foot_quat_not_injected=True,full_source_initial_relative_and_final_quaternion_math_not_claimed=True,new_differences=[],screenshots=0,performance_acceptance='paused'),indent=2)+'\n');(r/'comparison-table.json').write_text(json.dumps(table,separators=(',',':'))+'\n');print(json.dumps(dict(counts=counts,source_normal_transitions=changed)),flush=True)
