import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())
baseline=(r/'replay-verified-baseline.json').read_bytes()
assert baseline==(r/'replay-restored-verified.json').read_bytes()
data=json.loads(baseline)
assert data['actual_target_rows_exact']==319
assert data['native_common_input_rows']==318 and data['common_input_difference_rows']==0
control=data['first_control']
assert control['source_view_only']==control['source']==control['source_view_and_history']
assert control['source_history_only']==control['target'] and control['source']!=control['target']
for name,digest in read('final-sources.json').items():assert hashlib.sha256((r/name).read_bytes()).hexdigest()==digest
assert read('replay-compile-restored-verified.report.json')['success']
assert read('replay-ablated-verified.report.json')['process_exit_code']==101
failure=Path('tools/tmp/bf144/447c/replay-ablated-verified.stderr.log').read_text()
assert 'First Look difference is removed by the native view yaw alone'in failure
(r/'ablation-failure.json').write_text(json.dumps(dict(expected_exit_code=101,assertion='First Look difference is removed by the native view yaw alone',stderr=failure),indent=2)+'\n')
assert read('identity-check.json')['fixture_source_restored']
inputs=['docs/audit/bf144/446f/restored.native.json.gz','docs/audit/bf144/447b/runtime-restored/parameters.json','docs/audit/bf144/447/head.native.json','docs/audit/bf144/447/head-settings.native.json']
summary=dict(gate='PASS_COMMON_INPUT_HEAD_KERNEL_AND_FIRST_LOOK_VIEW_FACTOR',actual_target_continuous_rows_exact=319,native_actual_previous_state_rows_exact=318,
    source_first_view_yaw=45.0,target_first_view_yaw=44.999996185302734,first_look_difference_removed_by_view_only=True,history_only_does_not_remove_first_difference=True,
    view_factor_ablation_rejected=True,fixture_source_bytes_restored=True,fixture_recompiled_against_same_target_six_libraries=True,restored_replay_byte_exact=True,
    reused_real_playground_functional_gate='docs/audit/bf144/447b/summary.json',new_runtime_required=False,
    evidence_inputs={p:hashlib.sha256(Path(p).read_bytes()).hexdigest()for p in inputs},new_differences=[],total_confirmed_differences=271,
    upstream_view_yaw_producer_cause_open=True,full_root_or_als_equivalence_claimed=False,screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps(summary),flush=True)
