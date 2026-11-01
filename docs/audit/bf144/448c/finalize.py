import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())
for name,digest in read('baseline-identities.json').items():assert hashlib.sha256((r/name).read_bytes()).hexdigest()==digest
assert hashlib.sha256((r/'compare.py').read_bytes()).hexdigest()==read('final-source.json')['sha256']
assert read('compare-ablated.report.json')['process_exit_code']==1
assert read('compare-restored.report.json')['success']and read('identity-check.json')['comparator_bytes_restored']
inventory=read('inventory.json');assert inventory['bone_pairs']==125610 and len(inventory['negative_fixtures_rejected'])==4
summary=dict(gate='PASS_FIVE_POSE_RESIDUAL_INVENTORY_WITH_ACTUAL_SCENE_TARGET_PREREQUISITE',matched_positive_frames=318,actual_boundary_rows=1590,source_target_bone_pairs=125610,
    source_current_five_boundary_custom_attributes_empty=True,target_custom_attributes_recorded_as_type_fact=True,none_channels_not_filled=True,raw_and_sign_aligned_quaternions_separate=True,
    first_root_translation_difference=inventory['first_differences']['root_result:translation'],counts=inventory['counts'],negative_fixtures_rejected=4,
    coordinate_projection_ablation_rejected=True,comparator_source_bytes_restored=True,same_python_entry_used=True,full_inventory_and_frame_table_restored_byte_exact=True,
    scene_extra_root_candidate_rows=44,actual_scene_target_binding_and_transform_missing=True,
    required_next_readonly_boundary='Actual AnimationTargetId/AnimatedBy, local Transform and gate before/after pose application for the extra SKM_Als scene node in the original Playground',
    new_differences=[],total_confirmed_differences=271,all_root_differences_attributed=False,full_root_or_als_equivalence_claimed=False,screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps({k:v for k,v in summary.items()if k!='counts'}),flush=True)
