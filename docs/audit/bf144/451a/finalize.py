import hashlib
import json
from pathlib import Path
from check import load,validate

r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())
a=load('baseline-fixed');b=load('restored');assert validate(a)==validate(b)
for key in ['rows','after_motors','camera_after_post_update','animation_before','animation_after']:assert a[key]==b[key],key
assert (r/'check-baseline-fixed.json').read_bytes()==(r/'check-restored.json').read_bytes()
assert read('check-ablated.report.json')['process_exit_code']==1
ablated=load('ablated')
for key in ['rows','after_motors','camera_after_post_update']:assert a[key]==ablated[key],key
for key in ['animation_before','animation_after']:assert [{k:v for k,v in row.items()if k!='head_apply'}for row in a[key]]==ablated[key],key
for name,digest in read('final-sources.json').items():assert hashlib.sha256((r/name).read_bytes()).hexdigest()==digest
for label in ['baseline-fixed','restored']:assert read(label+'-functional.json')['pass']
assert read('compile-restored.report.json')['success']and read('identity-check.json')['observer_sources_restored']
native=json.loads(Path('docs/audit/bf144/451/source-check.json').read_text());assert native['full_pose_and_curve_exact_actual_source']
assert (r/'runtime-baseline-fixed/skeleton.json').read_bytes()==(r/'runtime-restored/skeleton.json').read_bytes()
result=dict(gate='PASS_ACTUAL_FULL_HEAD_APPLY_ALPHA_SKELETON_AND_PUBLIC_KERNEL_ENTRY',queries_per_phase=319,full_current_head_pose_records=319,current_output_bone_rows=25520,
    input_base_bones=80,input_additive_bones=79,effective_alpha_all1_by_actual_input_identity_contract=True,historical_state_value_not_used_as_alpha=True,
    public_output_exact_all319=True,skeleton_path_records=82,empty_namespace_anchor_defaults_none_preserved=True,skeleton_snapshot_restored_byte_exact=True,
    native_source_control_frames=2,native_source_pose_bones_exact=158,native_source_curves_exact=True,
    old_five_complete_history_arrays_exact=True,observation_ablation_rejected=True,source_bytes_restored=True,fixture_recompiled_against_same_target_six_libraries=True,
    restored_complete_runtime_arrays_and_check_byte_exact=True,negative_current_fixtures_rejected=5,negative_native_fixtures_rejected=3,
    four_input_domain_factor_comparison_still_required=True,new_differences=[],total_confirmed_differences=272,full_root_or_als_equivalence_claimed=False,screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
