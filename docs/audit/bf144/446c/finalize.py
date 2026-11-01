import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())
assert read('compare-ablated.report.json')['process_exit_code']!=0
assert read('replay-ablated.report.json')['process_exit_code']==101
assert read('compare-restored.report.json')['success']and read('replay-restored.report.json')['success']
for name,digest in read('final-sources.json').items():assert hashlib.sha256((r/name).read_bytes()).hexdigest()==digest
assert (r/'idle-replay-baseline.json').read_bytes()==(r/'idle-replay.json').read_bytes()
inputs=read('input-and-state-check.json')
assert inputs['inputs']['external_input_rows_exact']==318 and inputs['inputs']['actual_controller_rows_exact']==318
assert len(inputs['negative_input_stage_axis_clock_cases_rejected'])==4
clocks=read('player-table.json')
assert clocks['source_declarations']==85
assert read('identity-check.json')['current_base_files_unchanged']==396
result=dict(gate='PASS_COMPLETE318_EXTERNAL_INPUT_AND_PRODUCER_BOUNDARY_AUDIT',external_input_rows_exact=318,actual_controller_rows_exact=318,controller_hold_rows=inputs['inputs']['controller_setter_hold_rows'],
    gameplay_state_tag_difference_rows=inputs['differences']['tags']['rows'],pending_difference_rows=inputs['differences']['pending']['rows'],known_pending_difference='BF144-D149',
    source_actual_player_ticks=clocks['source_tick_rows'],target_actual_player_ticks=clocks['target_tick_rows'],eligibility_difference_rows=clocks['eligibility_differences'],clock_difference_rows=clocks['clock_differences'],
    current_and_native_idle_rows_exact=318,idle_known_difference='BF144-D201',idle_clock_difference_rows=read('idle-replay.json')['clock_differences'],
    first_curve_history_eligibility_boundary_open=True,cold_vertical_motor_coefficient_boundary_open=True,
    negative_input_phase_axis_time_and_controller_cases_rejected=4,controller_hold_and_clock_factor_ablations_rejected=True,source_bytes_restored=True,same_target_rebuilt=True,idle_replay_restored_byte_exact=True,
    full_internal_input_history_matched=False,all_differences_attributed=False,full_root_or_als_equivalence_claimed=False,
    new_differences=[],total_confirmed_differences=271,screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result),flush=True)
