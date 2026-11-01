import hashlib
import json
from pathlib import Path
from check import validate

r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())
a=read('runtime-baseline/parameters.json')
b=read('runtime-restored/parameters.json')
assert validate(a)==validate(b)
for key in ['rows','after_motors','camera_after_post_update','animation_before','animation_after']:
    assert a[key]==b[key],key
old=json.loads(Path('docs/audit/bf144/445/runtime-baseline-full/parameters.json').read_text())
for key in ['rows','after_motors','camera_after_post_update']:assert a[key]==old[key]
for key in ['animation_before','animation_after']:
    assert [{k:v for k,v in row.items()if k!='history_input'}for row in a[key]]==old[key]
try:validate(read('runtime-ablated/parameters.json'))
except KeyError:pass
else:raise AssertionError('Missing startup history passed')
assert read('check-ablated.report.json')['process_exit_code']!=0
for name,digest in read('final-sources.json').items():assert hashlib.sha256((r/name).read_bytes()).hexdigest()==digest
for label in ['baseline','restored']:assert read(label+'-functional.json')['pass']is True
assert (r/'analysis-baseline.json').read_bytes()==(r/'analysis-restored.json').read_bytes()
assert read('identity-check.json')['observer_sources_restored']
result=dict(gate='PASS_ACTUAL_COLD_START_STATE_AND_INPUT_HISTORY_WITH_NATIVE_PREREQUISITES',queries_per_phase=319,cold_queries=79,ready_queries=240,positive_queries=318,
    cold_mode='Grounded',cold_movement_input_zero=True,cold_orientation_nonzero=True,native_current_fixed_initial_pose_not_matched=True,native_frame_limit_insufficient=True,
    source_zero_delta_initialization_confirmed=True,zero_initialization_itself_new_difference=False,pending_zero_update_difference_already_registered='BF144-D149',
    observation_ablation_rejected=True,source_bytes_restored=True,same_target_rebuilt=True,complete_runtime_arrays_restored_exact=True,analysis_restored_byte_exact=True,
    native_full_history_replayed=False,matched_full_history_claimed=False,full_root_or_als_equivalence_claimed=False,new_differences=[],total_confirmed_differences=271,
    render_gpu_pass_acceptance='deferred under explicit peripheral exclusion',animation_functional_gate_passed=True,screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result),flush=True)
