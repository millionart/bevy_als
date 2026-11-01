import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())
assert (r/'replay-baseline.json').read_bytes()==(r/'replay-restored.json').read_bytes()
assert (r/'check-baseline.json').read_bytes()==(r/'check-restored.json').read_bytes()
assert read('check-factor-ablated.report.json')['process_exit_code']==1
failure=Path('tools/tmp/bf144/447d/check-factor-ablated.stderr.log').read_text().replace(str(Path.cwd()),'.')
assert "row['late_view_cast_control']==sample['yaw_difference']"in failure
(r/'ablation-failure.json').write_text(json.dumps(dict(expected_exit_code=1,assertion="row['late_view_cast_control']==sample['yaw_difference']",stderr=failure),indent=2)+'\n')
for name,digest in read('final-sources.json').items():assert hashlib.sha256((r/name).read_bytes()).hexdigest()==digest
assert read('replay-compile-restored.report.json')['success']and read('identity-check.json')['fixture_source_restored']
check=read('check-restored.json')
summary=dict(gate='PASS_VIEW_CAST_FACTOR_AND_FIRST_HEAD_LOOK_CONSUMER_AS_RECORDED_D237',actual_target_producer_rows_exact=319,native_actual_source_formula_rows_exact=318,
    same_raw_view_and_representable_body_native_rows_exact=319,view_cast_factor_rows_changed=319,negative_fixtures_rejected=3,
    first_look_actual_consumer_proof='docs/audit/bf144/447c/replay-restored-verified.json',first_look=check['first_look'],recorded_difference='BF144-D237',
    source_bytes_restored=True,fixture_recompiled_against_same_target_six_libraries=True,restored_replay_and_check_byte_exact=True,view_factor_ablation_rejected=True,
    reused_functional_gate='docs/audit/bf144/447b/summary.json',production_or_reader_modified=False,
    evidence_inputs={p:hashlib.sha256(Path(p).read_bytes()).hexdigest()for p in ['docs/audit/bf144/447b/runtime-restored/parameters.json','docs/audit/bf144/446f/restored.native.json.gz','docs/audit/bf144/447c/replay-restored-verified.json']},
    new_differences=[],total_confirmed_differences=271,actual_body_producer_equivalence_claimed=False,full_root_or_als_equivalence_claimed=False,screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps(summary),flush=True)
