import json
from pathlib import Path

r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())
assert read('phase-check.json')['positive_rows']==318
assert read('input-and-state-check.json')['inputs']['external_input_rows_exact']==318
assert len(read('input-and-state-check.json')['negative_input_stage_axis_clock_cases_rejected'])==4
assert read('identity-check.json')['current_base_files_unchanged']==396
table=read('player-table.json')
assert table['source_declarations']==85
firsts=[v['clock_differences'][0]for v in table['table'].values()if v['clock_differences']and v['source']['asset'].endswith('BS_Als_Look.BS_Als_Look')]
assert len(firsts)==1 and firsts[0]['source_frame']==78
result=dict(gate='PASS_FRAME_ALIGNED_COMPLETE_RESIDUAL_INVENTORY_WITH_OPEN_HEAD_INPUT',positive_external_inputs_exact=318,actual_controller_inputs_exact=318,
    source_and_target_new_frame_qualification_verified=True,same_frame_source_and_shifted_target_rejected=True,input_axis_time_controller_negative_cases_rejected=4,
    source_player_declarations=85,source_tick_rows=table['source_tick_rows'],target_tick_rows=table['target_tick_rows'],eligibility_difference_rows=table['eligibility_differences'],clock_difference_rows=table['clock_differences'],
    earliest_unattributed_look_clock=firsts[0],target_actual_head_input_state_observed=False,head_source_assets_and_settings_reextracted=True,
    known_first_idle_cause='BF144-D201',known_first_position_cause='BF144-D31',all_other_differences_attributed=False,full_root_or_als_equivalence_claimed=False,
    new_differences=[],total_confirmed_differences=271,screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result),flush=True)
