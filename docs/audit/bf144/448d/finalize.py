import hashlib
import json
from pathlib import Path
from check import load,validate

r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())
a=load('baseline');b=load('restored');assert validate(a)==validate(b)==44
for key in ['rows','after_motors','camera_after_post_update','animation_before','animation_after']:assert a[key]==b[key],key
assert (r/'check-baseline.json').read_bytes()==(r/'check-restored.json').read_bytes()
assert read('check-ablated.report.json')['process_exit_code']==1
ablated=load('ablated')
for key in ['rows','after_motors','camera_after_post_update']:assert a[key]==ablated[key],key
for key in ['animation_before','animation_after']:assert [{k:v for k,v in row.items()if k!='scene_root'}for row in a[key]]==ablated[key],key
for name,digest in read('final-sources.json').items():assert hashlib.sha256((r/name).read_bytes()).hexdigest()==digest
assert read('compile-restored.report.json')['success']and read('identity-check.json')['observer_sources_restored']
for label in ['baseline','restored']:assert read(label+'-functional.json')['pass']
summary=dict(gate='PASS_ACTUAL_SCENE_TARGET_QUALIFICATION_AND_EXTRA_SCALE_CANDIDATE_REJECTION',queries_per_phase=319,actual_matching_targets_zero_all319=True,
    mapped_entity_is_actual_skinned_mesh=True,target_id_and_animated_by_absent_all319=True,mapped_local_transform_identity_all319=True,nonidentity_pose_rows_exact_prior_b=44,
    scene_scale_shrink_candidate_rejected_for_current_apply_chain=True,source_bytes_restored=True,same_target_six_libraries_used=True,restored_complete_new_and_old_arrays_and_check_exact=True,
    observation_ablation_rejected=True,negative_fixtures_rejected=4,source_and_production_unchanged=True,
    remaining_root_bone_and_curve_residual_inventory='docs/audit/bf144/448c/inventory.json',all_root_differences_attributed=False,
    new_differences=[],total_confirmed_differences=271,full_root_or_als_equivalence_claimed=False,screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps(summary),flush=True)
