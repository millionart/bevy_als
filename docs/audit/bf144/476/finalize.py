import gzip
import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
def raw(name):
 p=r/name
 if not p.exists():p=r/(name+'.gz')
 return gzip.decompress(p.read_bytes())if p.suffix=='.gz'else p.read_bytes()
def read(name):return json.loads(raw(name))
foundation=read('foundation-summary.json');assert foundation['gate']=='PASS_NATIVE_IK_TEMPORAL_ABI_DATA_FOUNDATION_WITH_COMPLETE_HISTORY_TRANSPARENCY'
for name,row in read('foundation-manifest.json')['files'].items():assert hashlib.sha256((r/name).read_bytes()).hexdigest()==row['sha256'],name
assert raw('replay.rs')==raw('replay.rs.final')and hashlib.sha256(raw('replay.rs')).hexdigest()==read('final-source.json')['sha256'];assert raw('replay-unit-baseline.json')==raw('replay-unit-simplified-seed.json')==raw('replay-unit-restored.json')
for label in['baseline','simplified-seed','restored']:assert read('replay-compile-'+label+'.report.json')['success']and read('replay-'+label+'.report.json')['success']
assert read('replay-compile-no-pole-pose-input.report.json')['success']and read('replay-no-pole-pose-input.report.json')['process_exit_code']==101;assert read('parameter-negative.json')['different_calls']==1272 and read('replay-unit-no-pole-pose-input.json')['exact']==0
assert read('replay-unit-restored.json')['exact']==1272;assert read('identity-parent-final.report.json')['success']and read('identity-check.json')['six_libraries_unchanged']
summary=dict(gate='PASS_ACTUAL_IK_ABI_NATIVE_PRE_POST_AND_COMPLETE_PUBLIC_SINGLE_CALL_REPLAY',foundation_gate=foundation['gate'],foundation_commit='6326eb51',source_native_leg_calls=1332,source_native_pin_values=29304,source_pre_post_pose_changed_calls=1330,source_weight_zero_unchanged_calls=2,source_stage_contract_positive=1332,source_stage_contract_post_substitution_rejected=1332,source_hidden_after_not_falsely_before=True,source_effector_upstream_position_rotation_crossed=1272,complete_old_native_and_current_histories_preserved=True,current_shared_calls_observed=2548,current_active_legs_exact_from_inputs=1272,current_complete_args_and_three_bone_tqs_exact=True,initial_definitions_unchanged=True,forced_transform_seed_removed_all_report_bytes_equal=True,wrong_post_pose_idempotence_pass_not_accepted_as_input_time_proof=True,pole_echo_only_failure_not_pose_necessity=True,pole_pose_negative_argument_echo_accepted_then_all1272_pose_results_rejected=True,source_bytes_restored_same_target_six_libraries_recompiled_all_output_bytes_exact=True,original_scope_preserved_after_timeout_split=True,source_solver_length_position_rotation_partial_math_still_open=True,actual_source_target_input_and_output_differences_not_all_attributed=True,source_current396_reader474a_and_six_libraries_unchanged=True,external_source_files=foundation['external_source_files'],new_differences=[],confirmed_difference_count=272,production_reader_engine_converters_source_assets_unchanged=True,full_rig_or_root_equivalence_claimed=False,next_boundary='Source current automatic lengths and IK position/rotation/partial wrapper on qualified pre-pose and parameters',screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');print('Verified native1332 ABI/time facts and1272 complete public IK calls with meaningful pole pose counterexamples and exact source-byte recovery',flush=True)
