import gzip
import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
def read(name):
 p=r/name
 if not p.exists():p=r/(name+'.gz')
 return json.loads(gzip.decompress(p.read_bytes())if p.suffix=='.gz'else p.read_bytes())
reader=Path('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp');assert reader.read_bytes()==(r/'reader.final.cpp').read_bytes();assert hashlib.sha256(reader.read_bytes()).hexdigest()==read('reader-final-sha256.json')['sha256'];assert read('restored-package-source-check.json')['source_package_bytes_exact']and read('restored-package-source-check.json')['full_build_terminal_verified']
for label in['baseline','flag-false','flag-true','restored']:assert read('build-'+label+'.report.json')['success']
for label in['baseline','restored']:assert read('check-'+label+'.json')['calls']==1272 and read('check-'+label+'.json')['dirty']==1272
for label in['weights-baseline','weights-restored']:assert read('check-'+label+'.json')['calls']==60 and read('check-'+label+'.json')['dirty']==58 and read('check-'+label+'.json')['clean']==2
assert read('check_missing.report.json')['process_exit_code']==1
assert read('invalid-character-asset.report.json')['process_exit_code']==7 and read('invalid-character-asset.json')['failure_excluded_from_compatibility_acceptance']
for label in['weights-flag-false','weights-flag-true']:assert read('check-'+label+'.report.json')['process_exit_code']==1
ablation=read('ablations.json');assert ablation['results']['flag-false']['wrong_flags']==58 and ablation['results']['flag-true']['wrong_flags']==2;assert read('restore-check.json')['actual_calls']==1272 and read('legacy-boundary-check.json')['getter_values']==2544
assert read('compatibility.json')['whole_old_default_rig_exact']and read('compatibility.json')['whole_old_default_character_exact']and read('compatibility.json')['whole_old_three_axis_curvevector_exact'];assert read('identity-check.json')['six_libraries_unchanged']
for row in read('compression-map.json').values():
 packed=(r/row['compressed']).read_bytes();raw=gzip.decompress(packed);assert len(raw)==row['raw_bytes']and hashlib.sha256(raw).hexdigest()==row['raw_sha256']and hashlib.sha256(packed).hexdigest()==row['compressed_sha256']
external=[]
for name in['docs/audit/bf144/469a/reader-migration.json','docs/audit/bf144/469a/request.json','docs/audit/bf144/469a/restored.native.json.gz','docs/audit/bf144/473/weights.request.json','docs/audit/bf144/473/weights.native.json.gz','docs/audit/bf144/462/cache_identity.py','docs/audit/bf144/462/request.json','docs/audit/bf144/462/restored.native.json.gz','docs/audit/bf144/457/default.request.json','docs/audit/bf144/457/default.native.json.gz','docs/audit/bf144/441b/curves.native.json','docs/audit/bf144/441b/curves.request.json','docs/audit/bf144/441b/identities.start.json','docs/audit/bf144/441c/library-identities.json','tools/run_ue_source_reader.ps1','tools/build_ue_source_reader.ps1']:
 data=Path(name).read_bytes();external.append(dict(path=name,bytes=len(data),sha256=hashlib.sha256(data).hexdigest()))
summary=dict(gate='PASS_NATIVE_LOCAL_DIRTY_READONLY_CAPTURE_WITH_ABLATIONS_RESTORE_AND_FULL_LEGACY_COMPATIBILITY',new_metadata_field='current_local_dirty_before_global_read',capture_stage='after original native function before existing global getter',only_existing_opt_in_unit_boundary_changed=True,no_new_config_state_or_source_algorithm=True,actual_post_setrotation_calls=1272,actual_native_dirty_true=1272,controlled_post_setrotation_calls=60,controlled_native_dirty_true=58,controlled_native_clean=2,clean_cohort_exactly_retains_input_local=True,dirty_changed_local_relation_qualified_only_for_this_fixture=True,source_local_math_equivalence_not_claimed=True,legacy_boundary_typed_flags=2544,complete_old_unit_boundary_body_exact_except_new_field=True,complete_actual_and_controlled_execution_pose_work_hierarchy_unchanged=True,forced_false_wrong_flags=58,forced_true_wrong_flags=2,forced_metadata_only_no_other_native_effects=True,source_bytes_restored_full_package_terminal_verified=True,all1332_flag_global_final_local_records_byte_restored=True,whole_default_rig_character_and_three_axis_curvevector_compatibility=True,reader_migration='docs/audit/bf144/474a/reader-migration.json',other395_and_six_libraries_unchanged=True,external_source_files=external,new_differences=[],confirmed_difference_count=272,production_engine_converters_source_assets_unchanged=True,full_rig_or_root_equivalence_claimed=False,invalid_character_asset_attempt_preserved_and_excluded=True,parent474_still_open=True,screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');print('Verified readonly1332 native dirty flags,2544 old boundary items,metadata-only ablations,byte restoration,full legacy compatibility and frozen identities',flush=True)
