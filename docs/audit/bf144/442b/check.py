import json
from pathlib import Path
r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())
a=read('actual-consumers.json');b=read('no-normalization.json');c=read('no-angle-stage.json');assert len(a)==317
assert(r/'matrix.rs').read_bytes()==(r/'matrix.final.rs').read_bytes()
assert(r/'restored.json').read_bytes()==(r/'actual-consumers.json').read_bytes()
assert not read('no-minimax.report.json')['success']and read('no-minimax.report.json')['process_exit_code']==101
x=next(x for x in a if x['name']=='45:1');y=next(x for x in b if x['name']=='45:1')
assert x['source']['world_input']==x['source']['cmc_input']
assert x['standard_trig_raw_with_source_clamp']!=x['native_trig_raw_f32_with_source_clamp']
assert x['same_raw_normalized_cmc']!=x['native_cmc_projected']and x['same_raw_extra_norm_double_projected']!=x['native_cmc_projected']
assert y['same_raw_normalized_cmc']==y['native_cmc_projected']and y['same_raw_extra_norm_public_plan']==y['native_input_public_plan']
for name in ['no-normalization-build','no-normalization','no-angle-stage-build','no-angle-stage','restored-build','restored','actual-consumers-build','actual-consumers']:
    assert read(name+'.report.json')['success'],name
assert all(z['actual_consumers']is None or z['actual_consumers']['source_angular_input']==z['actual_consumers']['current']for z in c)
actual=[z for z in a[77:]if z['actual_consumers']['source_angular_input']!=z['actual_consumers']['current']];assert len(actual)==1 and actual[0]['actual_consumers']['ordinal']==22
for z in a[77:]:
    assert z['source_producer_all_stages_native_exact']
    assert z['actual_consumers']['source_angular_input']==z['actual_consumers']['reduced_angle_standard_math']
report={'gate':'PASS_NATIVE317_INPUT_PRODUCERS_THREE_BOUNDARIES_AND_BYTE_RESTORATION','native_source_angle_clamp_trig_world_and_cmc_stages_exact':317,'shared_operator_cases':77,'actual_input_camera_cases':240,'same_angle_trig_raw_difference_cases':sum(z['standard_trig_raw_with_source_clamp']!=z['native_trig_raw_f32_with_source_clamp']for z in a[:77]),'same_native_vector_extra_norm_minimum':x,'actual_physical_protocol_direction_difference_frames':[21,22],'actual_public_plan_angular_difference_frames':[22],'actual_plan_other_frame_cancels':[21],'three_ablations_have_expected_failed_or_lost_signals':True,'source_bytes_restored_same_target_recompiled_complete317_bytes_exact':True,'remaining_after_removing_extra_norm_source_diagonal_clamp_input_width':'known D266; identical wide clamp changes only projected input','live_body_feedback_or_full_root_recovery_claimed':False,'parent442_cold_and_no_camera_fallback_lifecycles':'unverified','new_differences':['D269/P3','D270/P3','D271/P3'],'confirmed_active_differences':268,'screenshots':0,'performance_acceptance':'paused'}
(r/'summary.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({k:v for k,v in report.items()if k!='same_native_vector_extra_norm_minimum'}),flush=True)
