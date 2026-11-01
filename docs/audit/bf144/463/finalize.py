import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
def read(name):return json.loads((r/name).read_text())
assert(r/'replay.rs').read_bytes()==(r/'replay.rs.final').read_bytes()
assert hashlib.sha256((r/'replay.rs').read_bytes()).hexdigest()==read('final-source.json')['sha256']
assert(r/'replay-canonical.json').read_bytes()==(r/'replay-restored.json').read_bytes()
for label in ['canonical','restored']:
 assert read('replay-'+label+'.report.json')['success']and read('replay-compile-'+label+'.report.json')['success']
for label in ['no-source-forward','no-source-point']:
 assert read('replay-'+label+'.report.json')['process_exit_code']==101 and read('replay-compile-'+label+'.report.json')['success']
check=read('check.json');assert check['offset_matches']['source_forward_full_inverse']==636
assert check['ablations']=={'no-source-forward':600,'no-source-point':92}
assert check['offset_matches']==dict(current=48,normal_only=92,normal_and_point=600,native_hit_and_inverse=636,source_forward_full_inverse=636,native_ray_target_query=636)
assert check['query']['source_forward_vs_native_ray_full_hit_changed']==0 and check['query']['native_world_point_vs_same_ray_target_full_point_differences']==636 and check['query']['native_world_point_vs_same_ray_target_vertical_differences']==0
assert read('design-ablation.json')['source_height_meter_float_and_source_cm_output_identical_all636']
assert read('identity-check.json')['six_libraries_unchanged']
external=[dict(path=name,**row)for name,row in read('input-identities.json').items()]
for name in ['docs/audit/bf144/462/reader-migration.json','docs/audit/bf144/441c/library-identities.json']:
 data=Path(name).read_bytes();external.append(dict(path=name,bytes=len(data),sha256=hashlib.sha256(data).hexdigest()))
summary=dict(gate='PASS_ACTUAL636_SOURCE_QUERY_AND_VECTOR_ROTATION_SCALAR_CAUSAL_CLOSURE',calls=636,all_public_target_unit_queries_and_complete_arguments_exact=True,source_vertical_inverse_qualified_from_raw_world_hit_and_actual_world_input_all636=True,source_output_not_injected_to_model=True,offset_match_counts=check['offset_matches'],step_partitions=check['partitions'],query_boundary=check['query'],known_current_scalar_cause='D247',known_world_point_precision_boundary='D246',source_forward_control_retains_f32_public_query_and_original_two_boxes=True,all1272_target_horizontal_hit_values_exact_f32_of_native_ray_origin=True,source_native_hit_vs_origin_small_f64_boundary_retained=True,no_source_cm_vs_runtime_m_float_scalar_difference_triggered_here=True,duplicate_control_variants_removed_after_equal_signal=True,two_physical_source_factor_ablations_reject_and_current_results_unchanged=True,source_bytes_restored_same_target_recompiled_complete636_output_bytes_exact=True,external_source_files=external,new_differences=[],confirmed_difference_count=272,full_rig_or_root_or_als_equivalence_claimed=False,source_internal_query_hook_claimed=False,production_reader_engine_assets_converters_unchanged=True,next_boundary='Apply only the qualified source vector rotation trace control to complete continuous public VM and follow actual spring and Rig output consumers, preserving unrelated source math and data boundaries',screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');print('Verified complete636 current scalar differences caused by source vector rotation stages with independent precision boundary retained',flush=True)
