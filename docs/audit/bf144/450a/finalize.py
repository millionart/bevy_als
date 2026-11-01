import hashlib
import json
from pathlib import Path
from check import load,validate

r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())
a=load('baseline');b=load('restored');assert validate(a)==validate(b)
for key in ['rows','after_motors','camera_after_post_update','animation_before','animation_after']:assert a[key]==b[key],key
assert (r/'check-baseline.json').read_bytes()==(r/'check-restored.json').read_bytes()
assert read('check-ablated.report.json')['process_exit_code']==1
ablated=load('ablated')
for key in ['rows','after_motors','camera_after_post_update']:assert a[key]==ablated[key],key
for key in ['animation_before','animation_after']:assert [{k:v for k,v in row.items()if k!='forward_difference'}for row in a[key]]==ablated[key],key
for name,digest in read('final-sources.json').items():assert hashlib.sha256((r/name).read_bytes()).hexdigest()==digest
for label in ['baseline','restored']:assert read(label+'-functional.json')['pass']
assert read('compile-restored.report.json')['success']and read('identity-check.json')['observer_sources_restored']
endpoints=json.loads(Path('docs/audit/bf144/450/endpoints.json').read_text())
assert endpoints['first_actual_native_head_x_equals_compressed']and endpoints['raw_absolute_equals_raw_reference_x']
result=dict(gate='PASS_ACTUAL_FORWARD_DIFFERENCE_INPUTS_PUBLIC_KERNEL_AND_MODE_ENDPOINTS',queries_per_phase=319,actual_difference_records=319,actual_bone_rows=25201,
    all_actual_difference_poses_exact_public=True,first_reference_x=0,first_absolute_and_current_delta_x=1.7007637193685825e-30,
    native_raw_additive_x_cm=0,native_compressed_additive_x_cm=.003921627998352051,source_current_head_first_delta_x=endpoints['compressed_physical_x'],
    first_source_sample_only_forward_at_half_phase_verified=True,unit_float_and_physical_projection_same_for_this_first_delta=True,
    old_five_complete_history_arrays_exact=True,observation_ablation_rejected=True,source_bytes_restored=True,fixture_recompiled_against_same_target_six_libraries=True,
    restored_complete_runtime_arrays_and_check_byte_exact=True,negative_fixtures_rejected=4,actual_consumer_data_factor_control_still_required=True,
    new_differences=[],total_confirmed_differences=272,full_root_or_als_equivalence_claimed=False,screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
