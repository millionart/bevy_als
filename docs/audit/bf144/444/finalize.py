import hashlib
import json
from pathlib import Path
from check import validate

r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())
a=read('runtime-baseline/parameters.json')
b=read('runtime-restored/parameters.json')
assert validate(a)==validate(b)
keys=['rows','after_motors','camera_after_post_update','animation_before','animation_after']
for key in keys:
    assert a[key]==b[key],key
old=json.loads(Path('docs/audit/bf144/443/runtime-baseline/parameters.json').read_text())
for key in keys[:3]:
    assert a[key]==old[key],key
assert (r/'replay-baseline.json').read_bytes()==(r/'replay-restored.json').read_bytes()
assert read('replay-ablated.report.json')['process_exit_code']==101
assert read('check-ablated.report.json')['process_exit_code']!=0
assert read('replay-restored.report.json')['success']
assert read('runtime-ablated/parameters.json')['animation_before']==[]
assert read('runtime-ablated/parameters.json')['animation_after']==[]
for name,digest in read('final-sources.json').items():
    assert hashlib.sha256((r/name).read_bytes()).hexdigest()==digest
assert read('identity-check.json')['current_base_files_unchanged']==396
for label in ['baseline','restored']:
    assert read(label+'-functional.json')['pass'] is True
matrix=read('replay-restored.json')
assert matrix['ready_rows']==240 and matrix['first_query_absolute_zero']
result=dict(gate='PASS_CURRENT_PHYSICS_TO_ANIMATION_INPUT_AND_CLOCK_ENTRY_WITH_HISTORY_BOUNDARY',
    animation_input_rows_exact=240,motion_and_provider_rows_exact=matrix['motion_and_provider_rows_exact'],acceleration_rows_exact=matrix['acceleration_rows_exact'],
    nonzero_acceleration_rows=matrix['nonzero_acceleration_rows'],startup_rows=matrix['cold_rows'],ready_start_arena_frame=matrix['ready_start_arena_frame'],
    native_source_main_updates=240,native_source_asset_player_slots=85,first_target_query_absolute_zero=True,root_pose_elapsed_used_as_asset_clock=False,
    source_target_same_animation_initial_history_claimed=False,source_state_text_exact_precision_claimed=False,full_asset_clock_or_root_equivalence_claimed=False,
    source_bytes_restored=True,same_target_rebuilt=True,replay_restored_byte_exact=True,full_runtime_arrays_restored_exact=True,
    observation_and_first_query_ablations_rejected=True,new_differences=[],total_confirmed_differences=271,
    render_gpu_pass_acceptance='deferred under explicit peripheral exclusion',animation_functional_gate_passed=True,screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result),flush=True)
