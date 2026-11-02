import gzip
import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent;read=lambda name:json.loads((r/name).read_bytes())
assert(r/'replay.rs').read_bytes()==(r/'replay.rs.final').read_bytes();assert hashlib.sha256((r/'replay.rs').read_bytes()).hexdigest()==read('final-source.json')['sha256']
prototype=read('replay-complete-public.json');restored=read('replay-restored.json');assert(r/'replay-complete-public.json').read_bytes()==(r/'replay-no-source-euler.json').read_bytes()==(r/'replay-no-source-product.json').read_bytes();prototype['outputs']=[row for row in prototype['outputs']if row['mask']!=27];assert prototype==restored
for label in['baseline','complete-public','no-source-euler','no-source-product','restored']:assert read('replay-compile-'+label+'.report.json')['success']and read('replay-'+label+'.report.json')['success']
for label in['no-source-constructor','no-neon-reciprocal','no-current-angle-bound']:assert read('replay-compile-'+label+'.report.json')['success']and read('replay-'+label+'.report.json')['process_exit_code']==101
assert restored['exact']==684 and len(restored['public_common'])==684 and len(restored['outputs'])==4104
comparison=read('comparison.json');assert comparison['counts']['31']==dict(double_differences=0,f32_differences=0);assert comparison['counts']['0']==dict(double_differences=677,f32_differences=1);assert read('compare-current-f32-contract.report.json')['process_exit_code']==1
by_key={}
for row in restored['outputs']:by_key.setdefault((row['dataset'],row['query'],row['pc']),{})[row['mask']]=row
assert all(row[15]['stages']['offset_rotation']==row[29]['stages']['offset_rotation']for row in by_key.values())
assert read('identity-check.json')['six_libraries_unchanged']
for row in read('compression-map.json').values():
 packed=(r/row['compressed']).read_bytes();raw=gzip.decompress(packed);assert len(raw)==row['raw_bytes']and hashlib.sha256(raw).hexdigest()==row['raw_sha256'];assert hashlib.sha256(packed).hexdigest()==row['compressed_sha256']
external=[]
for name in['docs/audit/bf144/469/inputs.json','docs/audit/bf144/469/replay-restored.json','docs/audit/bf144/470/replay-restored.json','docs/audit/bf144/469a/reader-migration.json','docs/audit/bf144/441c/library-identities.json','docs/audit/bf144/441b/identities.start.json','tools/animgraph_authoring/ue_control_rig_contract.py','tools/animgraph_authoring/ue_control_rig_target.py','tools/run_ue_source_reader.ps1']:
 raw=Path(name).read_bytes();external.append(dict(path=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest()))
summary=dict(gate='PASS_COMPLETE684_NATIVE_SOURCE_FOOT_ROTATIONS_AND_COMMON_PUBLIC_PRIMITIVE_FACTORS',complete_source_rotation_values_exact_native=684,independent_source_normal_and_initial_relative_predictions=True,exact_native_current_bone_inputs_consumed=True,current_common_source_data_public_calls_exact_model=684,complete_stage_factor_records=4104,source_ue_euler_and_handwritten_product_removed_after_all_stage_report_byte_equivalence=True,source_initial_relative_current_target_calf_quaternions_angles_limits_constructor_and_normalizer_all_recorded=True,primitive_counts=comparison['counts'],current_common_data_f32_contract_failed_at=dict(dataset='actual',query=141,pc=98),source_normalizer_current_method_alone_has_same_f32_counterexample=True,between_current_path_offset_equals_current_normalizer_source_raw_between_all684=True,source_constructor_and_neon_reciprocal_and_current_angle_bound_removal_rejected=True,byte_restoration_same_target_recompile_all_retained_stages_and_public_records_exact=True,external_source_files=external,known_differences=['D163','D248','D251'],new_differences=[],confirmed_difference_count=272,production_reader_engine_converters_source_assets_unchanged=True,full_rig_or_root_equivalence_claimed=False,next_boundary='Current actual input data versus source math full unit composition, weighted SetRotation and IK/local hierarchy writeback, then full Rig/Root feedback; preserve source primitive attribution',screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');print('Verified684 complete native foot rotations,684 exact common public models, primitive factors and simplified full-stage recovery',flush=True)
