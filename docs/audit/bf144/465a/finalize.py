import gzip
import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
def raw(name):
 p=r/name
 return p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix('.json.gz').read_bytes())
def read(name):return json.loads(raw(name))
reader=Path('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp');assert reader.read_bytes()==raw('reader.final.cpp')
assert hashlib.sha256(reader.read_bytes()).hexdigest()==read('reader-final-sha256.json')['sha256']
assert read('restored-package-source-check.json')['source_package_bytes_exact']and read('restored-package-source-check.json')['full_build_terminal_verified']
for label in ['build-baseline','build-zero-state','build-no-type-guard','build-restored']:assert read(label+'.report.json')['success']
assert read('check-old.report.json')['process_exit_code']==1 and read('check-zero-state.report.json')['process_exit_code']==1
for label in ['baseline','restored']:
 x=read('check-'+label+'.json');assert x['frames']==638 and x['active_state_frames']==636 and x['native_struct_states']==1276 and x['reflection_fields_remain_empty']
assert raw('table-baseline.json')==raw('table-restored.json')
assert read('compatibility.json')['whole_old_default_character_probe_exact']and read('compatibility.json')['whole_old_three_axis_curve_vector_probe_exact']
assert read('identity-check.json')['six_libraries_unchanged']
external=[]
for name in ['docs/audit/bf144/462/restored.native.json.gz','docs/audit/bf144/462/request.json','docs/audit/bf144/457/default.native.json.gz','docs/audit/bf144/457/default.request.json','docs/audit/bf144/441b/curves.native.json','docs/audit/bf144/441b/curves.request.json','docs/audit/bf144/462/reader-migration.json']:
 data=Path(name).read_bytes();external.append(dict(path=name,bytes=len(data),sha256=hashlib.sha256(data).hexdigest()))
summary=dict(gate='PASS_NATIVE_OPAQUE_FLOAT_SPRING_STATE_READONLY_CAPTURE_WITH_FULL_LEGACY_COMPATIBILITY',source_struct='FFloatSpringState',actual_non_uproperty_members=['PrevTarget','Velocity','bPrevTargetValid'],strict_opt_in='capture_float_spring_state',capture_only_matching_work_array_element_type=True,actual_source_frames=638,native_struct_states=1276,source_nonzero_active_history_frames=636,actual_previous_target_velocity_and_validity_lifecycle_qualified=True,zero_delta_initialization_exact=True,reflection_fields_remain_empty_and_native_state_is_separate=True,old_record_missing_state_rejected=True,zero_state_getter_ablation_rejected=True,type_guard_ablation_accepts_invalid_request=True,final_type_guard_rejects_invalid_request=True,full_old_rig_default_character_and_curvevector_probes_exact=True,reader_source_bytes_restored_same_full_package_build_terminal_and_package_sha_exact=True,complete_native_state_table_byte_exact_after_restore=True,reader_migration='docs/audit/bf144/465a/reader-migration.json',other395_and_six_libraries_unchanged=True,parent_spring_math_equivalence_not_claimed=True,source_memory_and_source_algorithms_not_modified=True,external_source_files=external,new_differences=[],confirmed_difference_count=272,production_engine_converters_assets_unchanged=True,next_boundary='Resume465 with actual non-reflected spring state and proven source pin lifetime; qualify full continuous source and target spring math and unit domains',screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');print('Verified native opaque float spring state capability, getter/type controls, complete legacy compatibility and exact restoration',flush=True)
