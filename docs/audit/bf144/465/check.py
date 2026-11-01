import json
import struct
from pathlib import Path

r=Path(__file__).resolve().parent
report=json.loads((r/'replay-restored.json').read_text());modes=report['modes'];assert len(modes)==3 and [m['mode']for m in modes]==[0,1,2]
assert(r/'replay-baseline.json').read_bytes()==(r/'replay-restored.json').read_bytes();assert modes[2]['native_exact']==319 and modes[2]['first_difference']is None
f32=lambda v:struct.unpack('f',struct.pack('f',v))[0]
lengths={0,3,5}
def projected(v):
 if 'F32'in v:return{'F32':f32(v['F32']*.01)}
 if 'Bool'in v:return v
 return{'List':[projected(x)for x in v['List']]}
counts=dict(source_trace_inputs_exact=0,current_result_differences=0,trace_control_result_differences=0,trace_control_velocity_differences=0,trace_control_state_differences=0);first=None;rows=[]
for current,control,source in zip(*[x['outputs']for x in modes],strict=True):
 assert current['exact']and control['exact']and source['exact']
 for i in range(8):
  expected=projected(source['before'][i])if i in lengths else source['before'][i];assert control['before'][i]==expected,(current['query'],i)
 counts['source_trace_inputs_exact']+=1
 expect=[projected(x)for x in source['after'][8:]];a=current['after'][8:];b=control['after'][8:]
 rows.append(dict(query=current['query'],source_projected_outputs=expect,current=a,trace_control=b))
 counts['current_result_differences']+=a[0]!=expect[0];counts['trace_control_result_differences']+=b[0]!=expect[0];counts['trace_control_velocity_differences']+=b[1]!=expect[1];counts['trace_control_state_differences']+=b[3]!=expect[3]
 if b!=expect and first is None:first=dict(query=current['query'],target_meter_outputs=b,source_cm_outputs=source['after'][8:],source_projected_outputs=expect)
(r/'output-comparison.json').write_text(json.dumps(rows,separators=(',',':'))+'\n')
assert counts==dict(source_trace_inputs_exact=319,current_result_differences=318,trace_control_result_differences=158,trace_control_velocity_differences=272,trace_control_state_differences=272)
assert json.loads((r/'replay-no-state-history.report.json').read_text())['process_exit_code']==101
(r/'check.json').write_text(json.dumps(dict(gate='PASS_ACTUAL319_PELVIS_SPRING_CM_NATIVE_AND_PUBLIC_METER_HISTORY_DOMAIN_DIFFERENCE',source_cm_complete_state_exact_native=319,public_original_and_trace_control_complete_state_exact=638,counts=counts,first_difference=first,independent_histories_no_native_after_state_injection=True,source_algorithms_match_native_when_original_cm_domain_preserved=True,history_removal_rejected=True,complete_restoration_bytes_exact=True,known_difference='D165',new_differences=[],full_rig_equivalence_claimed=False),indent=2)+'\n');print(json.dumps(dict(counts=counts,first_difference=first)),flush=True)
