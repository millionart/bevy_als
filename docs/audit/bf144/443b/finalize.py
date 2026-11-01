import hashlib
import json
from pathlib import Path
from check import validate,self_test

r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())
baseline=read('runtime-baseline-motion-scope/parameters.json');restored=read('runtime-restored/parameters.json')
counts=validate(baseline);assert counts==validate(restored)
for key in ['rows','after_motors','camera_after_post_update','before_floor_sync','integration_coefficients']:assert baseline[key]==restored[key],key
assert (r/'replay-final-baseline.json').read_bytes()==(r/'replay-restored.json').read_bytes()
assert read('replay-ablated.report.json')['process_exit_code']==101
bad=read('runtime-ablated/parameters.json');assert bad['before_floor_sync']==[] and bad['integration_coefficients']==[] and all('floor_probe'not in v for v in bad['rows'])
try:validate(bad)
except(AssertionError,KeyError):pass
else:raise AssertionError('Removed floor observation passed')
for name,digest in read('final-sources.json').items():assert hashlib.sha256((r/name).read_bytes()).hexdigest()==digest
assert read('identity-check.json')['current_base_files_unchanged']==396
for label in ['baseline-motion-scope','restored']:assert read(label+'-functional.json')['pass'] is True
matrix=read('replay-restored.json');assert matrix['mismatches']==[]
p=matrix['no_query_error_protocol'];assert p['source_band_y']==p['center'] and p['current_y']!=p['center']
first=baseline['integration_coefficients'][20];assert first['linear_increment']==[0,0,0] and first['initial_velocity'][1]==0
assert baseline['camera_after_post_update'][20]['position'][1]==baseline['after_motors'][20]['position'][1]!=baseline['rows'][20]['position'][1]
result=dict(gate='PASS_FULL_CURRENT240_FLOOR_TRANSACTION_PHYSICS_FINAL_HEIGHT_WITH_RECORDED_DIFFERENCE',counts=counts,force_and_full_physics_rows_exact=240,post_floor_rows_exact=240,zero_force_actual_transaction_witness=20,source_band_control_without_query_error=p,nonzero_force_rows=matrix['nonzero_force_coefficient_rows'],self_test=self_test(),source_bytes_restored=True,same_target_rebuilt=True,replay_output_restored_byte_exact=True,complete_runtime_records_restored_exact=True,new_difference='BF144-D273',total_confirmed_differences=270,source_world_feedback_restored_claimed=False,render_gpu_pass_acceptance='deferred by explicit peripheral exclusion; public original startup also TAA timeout',production_or_render_config_changed=False,animation_functional_gate_passed=True,screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:v for k,v in result.items()if k!='counts'}),flush=True)
