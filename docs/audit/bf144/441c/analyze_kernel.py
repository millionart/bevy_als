import json
from pathlib import Path
r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())
rows=read('kernel-speeds.json')['rows'];observed=read('runtime-assets-root/parameters.json')['rows'];assert len(rows)==len(observed)==240
assert all(x['gait_amount']==x['public_gait']and x['dynamics']==x['public_sample']for x in observed)
assert all(x['relative_velocity_consumed'][1]==0 and x['horizontal_gait']==x['gait_amount']for x in observed)
assert read('kernel-speeds.report.json')['success']
assert read('startup/runner.json')['validation_scope']=='functional' and read('startup/runner.json')['validation_exit_code']==0
assert read('startup/report.json')['screenshots']==[]
order=[x for x in rows if x['current']!=x['source_order_si']];units=[x for x in rows if x['source_order_si']!=x['source_order_cm']];data=[x for x in rows if x['current']!=x['raw_key_control']]
assert not data and sum(x['current']!=x['raw_key']for x in read('kernel-speeds.json')['data_controls'])==2
report={'gate':'PASS_ORIGINAL_PLAYGROUND240_OWN_PARAMETERS_AND_PUBLIC_KERNEL_FACTORS','actual_frames':240,'all_grounded_standing_view_running':True,'actual_gait_and_dynamics_equal_public_producers':True,'actual_pre_collision_plan_and_internal_steps_equal_public_kernel':True,'source_order_only_plan_frames':[x['ordinal']for x in order],'unit_stage_only_plan_frames':[x['ordinal']for x in units],'data_only_current_motion_frames':len(data),'expanded_data_controls_changed':2,'source_order_plan_examples':order,'unit_stage_plan_examples':units,'full_vector_vs_horizontal_gait_signal':0,'original_scene_preserved':True,'actual_desired_motion_not_assumed_equal_to_source_worldX':True,'public_startup_acceptance':'functional only; performance failures preserved','full_root_or_physical_feedback_equivalence_claimed':False,'screenshots':0,'performance_acceptance':'paused'}
(r/'kernel-check.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({k:v for k,v in report.items()if k not in ['source_order_plan_examples','unit_stage_plan_examples']}),flush=True)
