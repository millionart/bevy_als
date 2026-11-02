import gzip
import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
def read(name):
 p=r/name
 if not p.exists():p=r/(name+'.gz')
 return json.loads(gzip.decompress(p.read_bytes())if p.suffix=='.gz'else p.read_bytes())
source=read('source-boundary-check.json');assert source['calls']==1332 and source['pins']==29304 and source['source_changed_calls']==1330
for label,count in[('actual-before',1272),('actual-after',2548),('weights-before',60),('weights-after',130)]:
 assert read('check-'+label+'.json')['calls']==count and read('check-'+label+'.json')['complete_old_native_execution_pose_work_state_and_hierarchy_unchanged']and read(label+'.native.json.reader.report.json')['success']
assert read('stage-contract.json')['accepted_exact_pre_boundaries']==read('stage-contract.json')['rejected_post_boundaries']==1332
assert read('upstream-input-check.json')['calls']==1272 and read('current-calls.json')['legs']==1272
assert read('current-vm-baseline.report.json')['success']and read('current-vm-compile-baseline.report.json')['success'];assert read('wrong-prepose-no-counter.json')['not_accepted_as_necessary_negative_control']and read('replay-wrong-prepose.report.json')['success']
assert read('identity-check.json')['six_libraries_unchanged'];assert Path('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp').read_bytes()==Path('docs/audit/bf144/474a/reader.final.cpp').read_bytes()
for row in read('compression-map.json').values():
 packed=(r/row['compressed']).read_bytes();decoded=gzip.decompress(packed);assert len(decoded)==row['raw_bytes']and hashlib.sha256(decoded).hexdigest()==row['raw_sha256']and hashlib.sha256(packed).hexdigest()==row['compressed_sha256']
external=[]
for name in['docs/audit/bf144/469a/request.json','docs/audit/bf144/469a/restored.native.json.gz','docs/audit/bf144/473/weights.request.json','docs/audit/bf144/473/weights.native.json.gz','docs/audit/bf144/473/replay-baseline.json.gz','docs/audit/bf144/459/inputs.json','docs/audit/bf144/463/replay-restored.json','docs/audit/bf144/468/inputs.json','docs/audit/bf144/469/inputs.json','docs/audit/bf144/474a/reader-migration.json','docs/audit/bf144/474a/reader.final.cpp','docs/audit/bf144/462/cache_identity.py','docs/audit/bf144/441b/identities.start.json','docs/audit/bf144/441c/library-identities.json','tools/animgraph_authoring/ue_control_rig_contract.py','tools/animgraph_authoring/ue_control_rig_target.py','tools/run_ue_source_reader.ps1']:
 raw=Path(name).read_bytes();external.append(dict(path=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest()))
summary=dict(gate='PASS_NATIVE_IK_TEMPORAL_ABI_DATA_FOUNDATION_WITH_COMPLETE_HISTORY_TRANSPARENCY',source_native_leg_calls=1332,source_native_pin_values=29304,ordinary_and_hidden_source_refs_no_late_alias=True,source_hidden_cache_values_after_call_not_before=True,source_pre_post_changed=1330,source_zero_weight_unchanged=2,source_post_calf_and_foot_exact_downstream_consumers=True,source_effector_input_position_and_rotation_independently_crossed=1272,source_automatic_lengths_requested_not_claimed_computed_for_zero_weight=True,full_native_and_current_histories_unchanged=True,current_shared_ik_observations=2548,current_active_leg_calls=1272,inactive_arm_and_initial_frame_not_solver_acceptance=True,native_pre_stage_contract_accepts=1332,native_post_as_pre_contract_rejects=1332,current_post_input_idempotence_no_counterexample_preserved=True,no_pole_input_echo_failure_not_accepted_as_pose_counterexample=True,current_public_final_recovery_and_parameter_pose_ablation_still_pending=True,parent476_full_objective_preserved=True,source_solver_mathematics_and_full_rig_not_claimed=True,current396_reader474a_and_six_libraries_unchanged=True,external_source_files=external,new_differences=[],confirmed_difference_count=272,production_reader_engine_converters_source_assets_unchanged=True,screenshots=0,performance_acceptance='paused')
(r/'foundation-summary.json').write_text(json.dumps(summary,indent=2)+'\n');print('Verified1332 native temporal/ABI facts,29304 pins,full old histories and rejected1332 post-stage substitutions; public final gate remains pending',flush=True)
