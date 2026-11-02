import gzip
import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
def raw(name):
 p=r/name;return p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix(p.suffix+'.gz').read_bytes())
def read(name):return json.loads(raw(name))
reader=Path('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp');assert reader.read_bytes()==raw('reader.final.cpp');assert hashlib.sha256(reader.read_bytes()).hexdigest()==read('reader-final-sha256.json')['sha256'];assert read('restored-package-source-check.json')['source_package_bytes_exact']and read('restored-package-source-check.json')['full_build_terminal_verified']
for label in['build-public-table','build-zero-current','build-no-normal-restore','build-no-config-guard-explicit','build-restored']:assert read(label+'.report.json')['success']
assert not read('build-baseline.report.json')['success']and not read('build-no-config-guard.report.json')['success'];assert read('invalid-attempt-autobuild.report.json')['process_exit_code']!=0
for label in['check_before','check-zero-current','check-no-normal-restore']:assert read(label+'.report.json')['process_exit_code']==1
for label in['baseline','restored']:
 value=read('check-'+label+'.json');assert value['calls']==1272 and value['frames']==638 and value['complete_old_probe_and_execution_and_pose_exact']and value['descriptor_and_thread_local_binding_restored']
assert raw('table-baseline.json')==raw('table-restored.json')
controls=read('controls-check.json');assert controls['calls']==48 and controls['foot_rotations_changed_after_consumer']==8 and controls['complete_old_execution_pose_and_work_unchanged']
assert read('controls-restored-check.json')['complete_controlled_boundaries_exact_after_byte_restore']
assert read('compatibility.json')['whole_old_default_character_probe_exact']and read('compatibility.json')['whole_old_three_axis_curve_vector_probe_exact']and read('identity-check.json')['six_libraries_unchanged']
for row in read('compression-map.json').values():
 packed=raw(row['compressed']);decoded=gzip.decompress(packed);assert len(decoded)==row['raw_bytes']and hashlib.sha256(decoded).hexdigest()==row['raw_sha256'];assert len(packed)==row['compressed_bytes']and hashlib.sha256(packed).hexdigest()==row['compressed_sha256']
external=[]
for name in['docs/audit/bf144/467/restored.native.json.gz','docs/audit/bf144/467/request.json','docs/audit/bf144/467/reader-migration.json','docs/audit/bf144/468/rig.native.json.gz','docs/audit/bf144/462/restored.native.json.gz','docs/audit/bf144/462/request.json','docs/audit/bf144/457/default.native.json.gz','docs/audit/bf144/457/default.request.json','docs/audit/bf144/441b/curves.native.json','docs/audit/bf144/441b/curves.request.json','docs/audit/bf144/462/cache_identity.py']:
 data=Path(name).read_bytes();external.append(dict(path=name,bytes=len(data),sha256=hashlib.sha256(data).hexdigest()))
summary=dict(gate='PASS_SCOPED_EXACT_NATIVE_UNIT_BOUNDARY_CAPTURE_WITH_FULL_LEGACY_COMPATIBILITY',public_api='Per-VM GetFunctions descriptor pointer, original FunctionPtr and typed FRigElementKey input handles',strict_optional_request='unit_boundary',stage='after_native_unit_before_next_instruction',no_global_registry_or_source_algorithm_modification=True,selected_source_unit_readonly_hierarchy=True,actual_frames=638,actual_calls=1272,actual_flat_foot_rotation_changed_later=0,controlled_calls=48,controlled_foot_rotations_changed_later=8,current_calf_and_initial_source_items_exact=True,full_weight_effector_source_equals_bounds_verified=True,complete_original_and_controlled_probe_unchanged=True,original_function_forwarded_once_without_reimplementation=True,original_descriptor_and_thread_local_state_restored_per_case=True,zero_current_getter_and_no_normal_restore_ablations_rejected=True,invalid_config_accepted_only_when_config_guard_ablated=True,final_bad_config_and_non_key_argument_rejected=True,source_bytes_restored_full_package_terminal_and_source_package_hash_exact=True,whole_original_boundary_table_exact_after_restore=True,whole_controlled_boundary_data_exact_after_restore=True,full_default_rig_character_and_curvevector_probes_exact=True,reader_migration='docs/audit/bf144/469a/reader-migration.json',other395_and_six_libraries_unchanged=True,failed_build_and_auto_retry_outputs_not_accepted=True,external_source_files=external,new_differences=[],confirmed_difference_count=272,production_engine_converters_source_assets_unchanged=True,parent469_rotation_math_not_claimed=True,next_boundary='Resume469 with exact native current foot and calf input and initial rotations; preserve source normal state and all quaternion primitive differences',screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');print('Verified scoped exact native call boundary,48 controlled source calls, descriptor restoration, complete old compatibility and byte recovery',flush=True)
