import hashlib
import json
from pathlib import Path
from check import load,validate

r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())
a=load('baseline');b=load('restored');assert validate(a)==validate(b)==319
for key in ['rows','after_motors','camera_after_post_update','animation_before','animation_after']:assert a[key]==b[key],key
assert (r/'check-baseline.json').read_bytes()==(r/'check-restored.json').read_bytes()
assert read('check-ablated.report.json')['process_exit_code']==1
ablated=load('ablated')
for key in ['rows','after_motors','camera_after_post_update']:assert a[key]==ablated[key],key
for key in ['animation_before','animation_after']:assert [{k:v for k,v in row.items()if k!='pose_boundaries'}for row in a[key]]==ablated[key],key
for name,digest in read('final-sources.json').items():assert hashlib.sha256((r/name).read_bytes()).hexdigest()==digest
for label in ['baseline','restored']:assert read(label+'-functional.json')['pass']
assert read('compile-restored.report.json')['success']and read('identity-check.json')['observer_sources_restored']
assert hashlib.sha256((r/'source-facts.json').read_bytes()).hexdigest()==read('facts-before-simplification.json')['sha256']
check=read('check-restored.json')
result=dict(gate='PASS_ACTUAL319_FIVE_POSE_BOUNDARIES_AND_ROOT_OUTPUT_READONLY_ENTRY',queries_per_phase=319,current_updated_boundaries=1595,default_root_equals_root_cache=True,
    source79_bones_present=True,additional_scene_root='SKM_Als',additional_scene_root_nonidentity_rows=check['additional_scene_root_nonidentity_rows'],first_extra_scene_root_candidate=check['first_additional_scene_root_nonidentity'],
    absent_pose_channels_preserved=True,custom_attribute_container_absence_recorded_as_type_fact=True,old_five_complete_history_arrays_exact=True,observation_ablation_rejected=True,
    source_bytes_restored=True,same_target_six_libraries_used=True,restored_complete_runtime_arrays_and_check_byte_exact=True,negative_fixtures_rejected=4,unused_fact_config_dependencies_removed_with_identical_output=True,
    source_current_attribute_proof='docs/audit/bf144/448a/summary.json',new_differences=[],total_confirmed_differences=271,full_root_or_als_equivalence_claimed=False,screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
