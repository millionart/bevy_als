import gzip
import json
from pathlib import Path

r=Path(__file__).resolve().parent
request=json.loads((r/'request.json').read_text());path=r/'common-input.native.json';native=json.loads(path.read_bytes()if path.exists()else gzip.decompress(path.with_suffix('.json.gz').read_bytes()))['blend_list_probe'];definition=json.loads((r/'runtime-baseline/rig-definition.json').read_text());names=json.loads((r/'source-bone-order.json').read_text())['names'];assert len(names)==79 and set(names)=={b['name']for b in definition['bones']}
assert len(native['samples'])==len(request['samples'])==319
for a,b in zip(request['samples'],native['samples'],strict=True):
 assert a['name']==b['name']and len(b['frames'])==1
 frame=b['frames'][0];assert frame['active']==0 and frame['weights']==[1,0]and frame['pose']==frame['input_poses'][0]
 for expected,actual in zip(a['poses'],frame['input_poses'],strict=True):
  assert [b['name']for b in actual['bones']]==names
  assert actual['curves']==expected['curves']
  for x,y in zip(expected['bone_transforms'],actual['bones'],strict=True):
   t=y['local_transform']
   assert [t['translation'][k]for k in ['x','y','z']]==x['translation_cm']
   assert [t['rotation'][k]for k in ['x','y','z','w']]==x['rotation']
   assert [t['scale'][k]for k in ['x','y','z']]==x['scale']
report=json.loads((r/'common-input.native.json.reader.report.json').read_text());assert report['success']and report['process_exit_code']==0
result=dict(gate='PASS_NATIVE319_ACTUAL_INPUT_SINGLE_FULL_WEIGHT_POSE_PASSTHROUGH',samples=319,bone_rows=25201,all_input_values_exact=True,all_output_full_pose_and_curves_exact_child0=True,inactive_input_explicitly_equal_not_actual_flail_or_snapshot=True,delta_zero_initial_state_evaluate_fixture_not_actual_update_history=True,new_differences=[],full_root_or_als_equivalence_claimed=False)
(r/'native-check.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
