import gzip
import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
def raw(name):
 p=r/name
 if not p.exists():p=r/(name+'.gz')
 return gzip.decompress(p.read_bytes())if p.suffix=='.gz'else p.read_bytes()
def read(name):return json.loads(raw(name))
assert raw('replay.rs')==raw('replay.rs.final')and hashlib.sha256(raw('replay.rs')).hexdigest()==read('final-source.json')['sha256'];assert raw('replay-baseline.json')==raw('replay-restored.json')
for label in['baseline','restored']:assert read('replay-compile-'+label+'.report.json')['success']and read('replay-'+label+'.report.json')['success']
assert read('replay-compile-no-public-unit-control.report.json')['success']and read('replay-no-public-unit-control.report.json')['process_exit_code']==101;assert read('analyze-no-unit-control.report.json')['process_exit_code']==1 and 'Independent public centimeter method exact'in raw('ablation-no-public-unit-control.failure.txt').decode()
analysis=read('analysis.json');classification=read('classification.json');assert analysis['common_current']['double_differences']==694 and analysis['common_current']['f32_differences']==692 and analysis['common_source']['f32_differences']==691;assert classification['existing_difference']=='BF144-D232'and not classification['new_difference_number_required'];assert classification['legacy_early_unit_sc_f32_changes']==36
for mode,changed in[('0',635),('1',636)]:assert analysis['actual'][mode]['effects']==dict(recovered=0,regressed=0,changed=changed,current_meter_native_different=636,current_centimeter_native_different=636)
assert read('identity-check.json')['six_libraries_unchanged']
for row in read('compression-map.json').values():
 packed=(r/row['compressed']).read_bytes();decoded=gzip.decompress(packed);assert len(decoded)==row['raw_bytes']and hashlib.sha256(decoded).hexdigest()==row['raw_sha256']and hashlib.sha256(packed).hexdigest()==row['compressed_sha256']
external=[]
for name in['docs/audit/bf144/474/inputs.json','docs/audit/bf144/474/replay-restored.json.gz','docs/audit/bf144/474/replay.rs','docs/audit/bf144/474/unit-phase-boundary.json','docs/audit/bf144/471/replay.rs','docs/audit/bf144/453/replay.rs','docs/audit/bf144/453/analysis-baseline.json','docs/audit/bf144/453/summary.json','docs/audit/bf144/474a/reader-migration.json','docs/audit/bf144/441b/identities.start.json','docs/audit/bf144/441c/library-identities.json','tools/animgraph_authoring/ue_control_rig_contract.py','tools/animgraph_authoring/ue_control_rig_target.py','tools/run_ue_source_reader.ps1']:
 data=Path(name).read_bytes();external.append(dict(path=name,bytes=len(data),sha256=hashlib.sha256(data).hexdigest()))
(r/'summary.json').write_text(json.dumps(dict(gate='PASS_DOUBLE_UNIT_PHASE_INDEPENDENT_PUBLIC_CONTROLS_AND_EXISTING_D232_CLASSIFICATION',source_common_cases=696,actual_current_cases=1272,independent_public_cm_m_both_exact_to_qualified_models=True,algorithm_rotation_scale_and_f64_precision_preserved=True,common_current_unit_double_differences=694,common_current_unit_f32_differences=692,common_current_unit_peak_meters=analysis['common_current']['peak_meters'],common_source_unit_double_differences=693,common_source_unit_f32_differences=691,common_source_unit_peak_meters=analysis['common_source']['peak_meters'],actual_unit_only_changed_f32_vectors=[635,636],actual_unit_only_full_vector_recovered=[0,0],actual_unit_only_native_full_vector_difference=[636,636],source_clean_cache_and_native_cm_endpoints_preserved=True,classification='Existing D232 source numeric space unit-phase consumer',legacy_453_f64_early_unit_control_existing_f32_changes=36,legacy_early_f32_and_current_unit_phase_subfactors_separate=True,no_f32_cast_or_import_coefficient_in_this_control=True,physical_public_cm_removal_rejected=True,analysis_no_unit_control_is_diagnostic_assertion_not_physical_ablation=True,source_bytes_restored_same_target_recompiled_complete_report_byte_exact=True,current396_reader474a_and_six_libraries_unchanged=True,external_source_files=external,new_differences=[],confirmed_difference_count=272,production_reader_engine_converters_source_assets_unchanged=True,full_rig_or_root_equivalence_claimed=False,next_boundary='Earlier actual IK pose producer and full descendant hierarchy writeback',screenshots=0,performance_acceptance='paused'),indent=2)+'\n');print('Verified independent1968 public cm/m double controls,existing D232 unit-phase fact,necessary ablation and complete restored report',flush=True)
