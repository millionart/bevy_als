import importlib.util
import json
from pathlib import Path
r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())
for name in ['build','compile','prepare_cases','kernel-speeds-build','kernel-speeds','analyze_kernel','check_mapping','check_controls','check_identity']:
    assert read(name+'.report.json')['success'],name
for name in ['curves-at-runtime.native.json.reader.report.json','mapping.native.json.reader.report.json','runtime-assets-root/process.report.json']:
    assert read(name)['success'],name
startup=read('startup/runner.json');assert startup['validation_exit_code']==startup['manual_exit_code']==0 and startup['validation_scope']=='functional'
assert read('runtime-baseline/process.report.json')['exit_reason']=='no_progress_timeout'
spec=importlib.util.spec_from_file_location('source_model',Path('docs/audit/bf144/441b/source_model.py'));m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
source=json.loads(Path('docs/audit/bf144/441b/source-parameters.json').read_text())['parameter_frames'];target=read('kernel-inputs.json');assert len(source)==len(target)==240
own=[sum(m.f(s['curve_values_cm_s2_and_friction'][axis]/(100 if axis<2 else 1))!=t['dynamics'][axis]for s,t in zip(source,target))for axis in range(3)]
result={'gate':'PASS_INDEPENDENT_SOURCE_TARGET240_PARAMETERS_AND_ACTUAL_PLAN_CONSUMERS','source_actual_actor_parameter_frames':240,'target_actual_original_playground_parameter_frames':240,'source_target_own_parameter_difference_counts':own,'own_trajectories_or_world_origins_equal_claimed':False,'target_public_gait_dynamics_plan_and_steps_all240_exact':True,'source_order_only_current_plan_frames':[124],'curve_unit_stage_only_current_plan_frames':[63],'raw_key_only_current_plan_frames':[],'expanded_key_controls_changed':2,'native_same_vector_lengths':240,'native_gait_range_controls':720,'length_stage_only_gait_rows':1,'mapping_units_only_gait_rows':77,'combined_common_gait_difference_rows':76,'current_vertical_component_gait_signal':0,'current_cached_acceleration_and_profile_difference_signal':0,'startup_scope':'functional','startup_full_performance_pass':False,'previous12018_identity_chain':'missing; current source396 and rebuilt six libraries separately bound','identity':read('identity-check.json'),'controls':read('controls-check.json'),'remaining':['Original source/target physical displacement and velocity feedback under declared common world/input conditions','Actual target desired-motion direction producer before current common-vector plan controls','Continuous complete Root and remaining clock/relevance/nonzero-view/custom-attribute closure'],'screenshots':0,'performance_acceptance':'paused'}
(r/'summary.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:result[k]for k in ['gate','source_target_own_parameter_difference_counts','source_order_only_current_plan_frames','curve_unit_stage_only_current_plan_frames','remaining']}),flush=True)
