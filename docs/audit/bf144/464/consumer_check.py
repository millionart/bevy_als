import gzip
import json
from pathlib import Path

r=Path(__file__).resolve().parent
def raw(name):
 p=r/name
 return p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix('.json.gz').read_bytes())
modes=json.loads(raw('replay-baseline.json'))['modes'];count=0
for mode in modes:
 for frame in mode['outputs'][1:]:
  calls=frame['calls'];trace=[c for c in calls if c['function']=='FAlsRigUnit_FootOffsetTrace::Execute'];spring=next(c for c in calls if c['function']=='FRigUnit_SpringInterpV2::Execute');locations=[c for c in calls if c['function']=='FAlsRigUnit_ApplyFootOffsetLocation::Execute'];rotations=[c for c in calls if c['function']=='FAlsRigUnit_ApplyFootOffsetRotation::Execute']
  assert len(trace)==len(locations)==len(rotations)==2
  assert spring['arguments_before_call'][0]['F32']==min(c['arguments_after_call'][8]['F32']for c in trace)
  for t,l,q in zip(trace,locations,rotations,strict=True):
   assert l['arguments_before_call'][3]==t['arguments_after_call'][8]
   assert l['arguments_before_call'][4]==spring['arguments_after_call'][8]
   assert q['arguments_before_call'][3]==t['arguments_after_call'][9]
   if mode['source']:assert t['arguments_after_call'][9]=={'Vector':[0,1,0]}
   count+=1
assert count==1272
ablation=json.loads(raw('replay-no-trace-control.json'))['modes'];assert [x['source']for x in ablation]==[False,False];assert ablation[0]['outputs']==ablation[1]['outputs']==modes[0]['outputs']
assert raw('replay-baseline.json')==raw('replay-restored.json')
(r/'consumer-check.json').write_text(json.dumps(dict(gate='PASS_ACTUAL_TRACE_TO_PELVIS_SPRING_FOOT_LOCATION_AND_ROTATION_CONSUMER_LINKS_AND_ABLATION',original_and_controlled_consumer_rows=1272,pelvis_spring_target_exact_minimum_trace_offset=True,foot_location_offset_and_pelvis_spring_result_exact=True,foot_rotation_normal_exact_trace_output=True,source_control_normals_all_up=True,control_removal_returns_entire_second_vm_domain_to_original_byte_values=True,complete_restored_two_domain_output_bytes_exact=True),indent=2)+'\n');print('Verified1272 direct consumer links and complete continuous VM effect disappearance on control removal',flush=True)
