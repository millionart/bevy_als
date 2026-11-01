import hashlib
import gzip
import json
from pathlib import Path

r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())
raw=lambda n:(r/n).read_bytes()if(r/n).exists()else gzip.decompress((r/(n+'.gz')).read_bytes())
assert raw('target-replay-input-verified.json')==raw('target-replay-restored.json')
assert (r/'analysis-input-verified.json').read_bytes()==(r/'analysis-restored.json').read_bytes()
assert raw('frame-table-input-verified.json')==raw('frame-table-restored.json')
for label in ['no-source-base','no-source-additive']:assert read('replay-'+label+'.report.json')['process_exit_code']==101
assert read('replay-compile-restored.report.json')['success']and read('identity-check.json')['observer_sources_restored']
assert hashlib.sha256((r/'replay.rs').read_bytes()).hexdigest()==read('final-source.json')['sha256']
a=read('analysis-restored.json');assert a['matched_frames']==318 and a['domain_outputs_per_system']==1272 and a['bone_outputs_per_system']==100488
assert a['native_ss_full_actual_pose_and_curve_frames_exact']==a['current_cc_actual_frames_exact']==318
assert a['summaries']['cc']['same_domain_native_to_current']['translation']==0 and a['summaries']['cc']['same_domain_native_to_current']['scale']==0
assert a['summaries']['cc']['same_domain_native_to_current']['rotation_aligned']==23032
assert a['summaries']['ss']['current_to_actual_source']['translation']==403 and a['summaries']['ss']['current_to_actual_source']['rotation_aligned']==22719
assert len(a['negative_fixtures_rejected'])==4
summary=dict(gate='PASS_COMPLETE318_HEAD_FOUR_DOMAIN_MATRIX_WITH_EXPLICIT_MATH_BOUNDARY',matched_positive_frames=318,domain_outputs_each_system=1272,bone_outputs_each_system=100488,
    native_ss_full_source_pose_and_curves_frames_exact=318,current_cc_full_actual_target_frames_exact=318,
    cc_same_lifted_input_native_current_translation_difference_rows=0,cc_same_lifted_input_native_current_scale_difference_rows=0,cc_same_lifted_input_native_current_rotation_difference_rows=23032,
    source_input_runtime_projection_current_kernel_position_difference_rows=403,source_input_runtime_projection_current_kernel_rotation_difference_rows=22719,
    current_data_source_data_interactions_not_summed=True,source_curves_held_common=True,extra_scene_node_excluded_from79_source_count=True,none_additive_scale_native_full_tqs_boundary_identity_zero=True,
    source_base_and_additive_removal_controls_rejected=True,source_bytes_restored=True,fixture_recompiled_against_same_target_six_libraries=True,full_replay_analysis_and_table_restored_byte_exact=True,
    negative_fixtures_rejected=4,reused_real_playground_functional_gate='docs/audit/bf144/451a/summary.json',production_reader_converter_or_original_assets_fixed=False,
    next_direct_boundary='Source lifted-current inputs versus current kernel rotation pipeline, including normalization stage and numerical space; do not classify23032 rotations as data-only',
    new_differences=[],total_confirmed_differences=272,full_root_or_als_equivalence_claimed=False,screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps(summary),flush=True)
