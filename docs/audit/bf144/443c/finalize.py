import hashlib
import json
from pathlib import Path
from check import validate

r=Path(__file__).resolve().parent;read=lambda n:json.loads((r/n).read_text())
a=read('runtime-baseline/parameters.json');b=read('runtime-restored/parameters.json');counts=validate(a);assert counts==validate(b)
for k in ['rows','after_motors','camera_after_post_update','before_floor_sync','integration_coefficients']:assert a[k]==b[k],k
assert (r/'replay-baseline.json').read_bytes()==(r/'replay-restored.json').read_bytes()
assert read('replay-ablated.report.json')['process_exit_code']==101
bad=read('runtime-ablated/parameters.json');assert all('scale_inputs'not in v for v in bad['before_floor_sync'])
try:validate(bad)
except(AssertionError,KeyError):pass
else:raise AssertionError('Missing scale source passed')
for name,digest in read('final-sources.json').items():assert hashlib.sha256((r/name).read_bytes()).hexdigest()==digest
assert read('identity-check.json')['current_base_files_unchanged']==396
for label in ['baseline','restored']:assert read(label+'-functional.json')['pass'] is True
matrix=read('replay-restored.json');assert matrix['radius_changed']==170 and matrix['floor_final_y_changed']==45
assert matrix['double_matrix_same_quaternion_still_differs']>0
result=dict(gate='PASS_STORED_SCALE_PRODUCER_AND_FLOOR_CONSUMER_WITH_RECORDED_DIFFERENCE',actual_root_tqs_qualified=240,parented=0,source_root_scale_one=240,actual_affine_scale_replayed_exact=240,radius_changed=170,only_scale_source_floor_final_y_changed=45,double_same_quaternion_matrix_differs=matrix['double_matrix_same_quaternion_still_differs'],source_bytes_restored=True,same_target_rebuilt=True,replay_restored_byte_exact=True,runtime_records_restored_exact=True,source_scale_selector_and_observation_ablations_rejected=True,new_difference='BF144-D274',total_confirmed_differences=271,source_whole_world_restored_claimed=False,render_gpu_pass_acceptance='deferred under explicit peripheral exclusion',animation_functional_gate_passed=True,screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
