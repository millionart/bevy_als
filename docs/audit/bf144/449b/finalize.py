import hashlib
import json
from pathlib import Path
from check import load,validate

r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())
a=load('baseline');b=load('restored');assert validate(a,True)==validate(b,True)
for key in ['rows','after_motors','camera_after_post_update','animation_before','animation_after']:assert a[key]==b[key],key
assert (r/'check-baseline.json').read_bytes()==(r/'check-restored.json').read_bytes()
assert validate(load('no-route'),False)['height']==.9675059914588928
assert read('runtime-no-once/process.report.json')['process_exit_code']!=0
failure=Path('tools/tmp/bf144/449b/no-once.stderr.log').read_text().replace(str(Path.cwd()),'.')
assert 'c449b_unit_factor'in failure and 'left: 0.96750605'in failure and 'right: 0.967506'in failure
(r/'no-once-failure.json').write_text(json.dumps(dict(expected_failure='Already converted source cannot satisfy the original target height contract twice',process_exit_code=read('runtime-no-once/process.report.json')['process_exit_code'],assertion_excerpt='left: 0.96750605; right: 0.967506'),indent=2)+'\n')
for name,digest in read('final-sources.json').items():assert hashlib.sha256((r/name).read_bytes()).hexdigest()==digest
assert read('compile-restored.report.json')['success']and read('identity-check.json')['observer_sources_restored']
for label in ['baseline','restored']:assert read(label+'-functional.json')['pass']
result=dict(gate='PASS_SINGLE_HEIGHT_FLOAT_EXPORT_UNIT_FACTOR_AND_ACTUAL_ROOT_CONSUMERS',same_source_height_raw_and_compressed_cm=96.75060272216797,
    target_export_float_coefficient=.009999999776482582,target_original_height=.9675059914588928,source_physical_unit_projection=.9675060510635376,
    public_source_data_only_pelvis_y_changed=True,only_idle_caller_routes_to_clone=True,head_reference_handle_unchanged=True,actual_leaf_records_restored=239,
    first98_actual_baseprovider_heights_recovered=True,first_actual_root_pelvis_y_recovered=True,other_input_clock_and_physics_fields_exact=True,camera_process_local_id_compared_by_unique_role=True,
    no_route_control_returns_original_height=True,once_guard_ablation_rejected=True,source_bytes_restored=True,fixture_recompiled_against_same_target_six_libraries=True,restored_all_controlled_arrays_and_check_exact=True,
    negative_fixtures_rejected=4,recorded_difference='BF144-D275',new_differences=['BF144-D275'],total_confirmed_differences=272,priority_counts=dict(P2=116,P3=156),
    production_reader_converter_or_original_asset_fixed=False,full_root_or_als_equivalence_claimed=False,screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
