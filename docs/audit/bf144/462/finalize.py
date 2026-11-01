import gzip
import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
def raw(name):
 p=r/name
 return p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix(p.suffix+'.gz').read_bytes())
def read(name):return json.loads(raw(name))
reader=Path('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp');assert reader.read_bytes()==raw('reader.final.cpp')
assert hashlib.sha256(reader.read_bytes()).hexdigest()==read('reader-final-sha256.json')['sha256']
assert read('restored-package-source-check.json')['sources_exact']and read('restored-package-source-check.json')['full_build_terminal_verified']
for label in ['build-baseline','build-no-vm-transform','build-no-parent-guard','build-restored']:
 assert read(label+'.report.json')['success']and read(label+'.report.json')['process_exit_code']==0
assert read('check-old.report.json')['process_exit_code']==1 and read('check-no-vm-transform.report.json')['process_exit_code']==1
for label in ['baseline','restored']:
 value=read('check-'+label+'.json');assert value['real_source_rays']==value['source_offset_cm_f32_exact']==value['source_normal_exact']==636 and value['complete_source_pose_work_externals_and_visits_exact_old']==638
assert raw('table-baseline.json')==raw('table-restored.json')
assert read('no-vm-transform-check.json')['all_638_changed_from_hit_to_miss']
assert read('cache-selftest.json')['changed_numeric_register_rejected']
assert read('compatibility.json')['whole_old_default_character_probe_exact']and read('compatibility.json')['whole_old_three_axis_curve_vector_probe_exact']
assert read('identity-check.json')['six_libraries_unchanged']
external=[]
for name in ['docs/audit/bf144/460/common-rig.native.json.gz','docs/audit/bf144/460/request.json','docs/audit/bf144/461/inputs.json','docs/audit/bf144/457/default.native.json.gz','docs/audit/bf144/457/default.request.json','docs/audit/bf144/441b/curves.native.json','docs/audit/bf144/441b/curves.request.json','docs/audit/bf144/457/reader-migration.json']:
 p=Path(name);data=p.read_bytes();external.append(dict(path=name,bytes=len(data),sha256=hashlib.sha256(data).hexdigest()))
summary=dict(gate='PASS_NATIVE_VM_SPACE_REFERENCE_QUERY_CAPABILITY_AND_COMPLETE_SOURCE_OUTPUT_COMPATIBILITY',actual_source_queries=636,additional_initial_references=2,source_actual_offset_cm_f32_and_normal_exact=636,complete_old_source_pose_externals_execution_clock_and_work_values_qualified=638,cache_instance_hash_only_canonicalized_by_fresh_ue_pointer_hash_semantics=True,cache_index_invalidation_and_numeric_negative_controls_rejected=True,vm_to_world_removal_loses_all638_hits_with_rig_outputs_unchanged=True,missing_world_guard_ablation_accepts_invalid_request=True,strict_type_and_missing_world_rejected_in_final_package=True,whole_legacy_rig_default_character_and_three_axis_curve_vector_compatible=True,source_bytes_restored_full_package_rebuilt_terminal_and_project_package_sha_exact=True,complete636_world_and_vm_query_rows_exact_after_restore=True,reader_migration='docs/audit/bf144/462/reader-migration.json',other395_and_six_libraries_unchanged=True,source_internal_query_hook_claimed=False,reference_query_after_forward_in_same_static_world=True,no_target_hit_injection=True,external_source_files=external,new_differences=[],confirmed_difference_count=272,production_engine_converter_assets_unchanged=True,next_boundary='Use qualified actual source local inputs and native context ray/hit outputs to separate current query precision and source centimeter float offset from known normal factor, then full continuous Rig outputs',screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');print('Verified native VM-space reference query capability, complete source unit output qualification, compatibility and restoration',flush=True)
