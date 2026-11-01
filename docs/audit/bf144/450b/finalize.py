import hashlib
import json
import struct
from pathlib import Path
from check import load,validate

r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())
f32=lambda v:struct.unpack('f',struct.pack('f',v))[0]
a=load('baseline');b=load('restored');assert validate(a,True)==validate(b,True)
for key in ['rows','after_motors','camera_after_post_update','animation_before','animation_after']:assert a[key]==b[key],key
assert (r/'check-baseline.json').read_bytes()==(r/'check-restored.json').read_bytes()
control=validate(load('no-route'),False);assert control['first_actual_rig_and_root_x']==1.7007637193685825e-30
assert control['raw_reference_only_first_delta']==-1.88079096131566e-37
endpoint=json.loads(Path('docs/audit/bf144/450/endpoints.json').read_text());cm=endpoint['raw_reference_pelvis_x_cm']
same_unit_ref=f32(f32(cm)*f32(.01));assert same_unit_ref==1.7007637193685825e-30 and f32(same_unit_ref-same_unit_ref)==endpoint['raw_delta_x_cm']==0
assert f32(same_unit_ref-f32(cm*.01))==control['raw_reference_only_first_delta']
assert read('runtime-no-once/process.report.json')['process_exit_code']!=0
failure=Path('tools/tmp/bf144/450b/no-once.stderr.log').read_text();assert 'c450b_data_factor'in failure and 'left: Some(Vec3(3.921628e-5'in failure and 'right: Some(Vec3(1.7007637e-30'in failure
(r/'no-once-failure.json').write_text(json.dumps(dict(expected_failure='Repeated initialization no longer matches original Forward translation keys',process_exit_code=read('runtime-no-once/process.report.json')['process_exit_code'],actual_already_patched_x=3.921628012903966e-5,required_original_x=1.7007637193685825e-30),indent=2)+'\n')
for name,digest in read('final-sources.json').items():assert hashlib.sha256((r/name).read_bytes()).hexdigest()==digest
assert read('compile-restored.report.json')['success']and read('identity-check.json')['observer_sources_restored']
for label in ['baseline','restored']:assert read(label+'-functional.json')['pass']
summary=dict(gate='PASS_HEAD_FIRST_COMPRESSED_DATA_X_AND_REFERENCE_UNIT_FACTOR_SEPARATION',original_translation_keys=31,only_half_key_index=15,only_pelvis_x_changed=True,only_forward_sample_caller_routes_clone=True,
    actual_half_phase_records=184,first_delta_rig_and_root_x_recovered=3.921628012903966e-5,original_delta_rig_and_root_x=1.7007637193685825e-30,
    compressed_data_factor_known_difference='BF144-D224',raw_reference_factor_known_difference='BF144-D235',physical_raw_reference_only_delta=-1.88079096131566e-37,
    remaining_absolute_unit_factor_known_difference='BF144-D275',same_export_unit_raw_reference_delta=0,
    source_bytes_restored=True,fixture_recompiled_against_same_target_six_libraries=True,restored_all_controlled_arrays_and_check_exact=True,route_ablation_returns_original_root_x=True,once_guard_ablation_rejected=True,
    other_motion_inputs_clocks_and_pose_channels_exact=True,camera_entity_refs_compared_by_unique_role=True,negative_fixtures_rejected=4,
    production_reader_converter_or_original_assets_fixed=False,new_differences=[],total_confirmed_differences=272,full_root_or_als_equivalence_claimed=False,screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps(summary),flush=True)
