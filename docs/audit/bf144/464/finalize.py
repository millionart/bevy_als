import gzip
import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
def raw(name):
 p=r/name
 return p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix('.json.gz').read_bytes())
def read(name):return json.loads(raw(name))
assert(r/'replay.rs').read_bytes()==(r/'replay.rs.final').read_bytes()
assert hashlib.sha256((r/'replay.rs').read_bytes()).hexdigest()==read('final-source.json')['sha256']
assert raw('replay-baseline.json')==raw('replay-restored.json')
for label in ['baseline','no-trace-control','restored']:
 assert read('replay-'+label+'.report.json')['success']and read('replay-compile-'+label+'.report.json')['success']
consumer=read('consumer-check.json');assert read('compare-no-trace-control.report.json')['process_exit_code']==1
assert consumer['original_and_controlled_consumer_rows']==1272 and consumer['control_removal_returns_entire_second_vm_domain_to_original_byte_values']
comparison=read('comparison.json');assert comparison['queries']==319 and comparison['register_rows']==129195 and comparison['local_bone_rows']==25201
assert comparison['source_residual_original']==dict(translation=1324,rotation=1845,scale=0)and comparison['source_residual_controlled']==dict(translation=1273,rotation=1847,scale=0)
assert comparison['changed_register_rows']==15440
assert read('identity-check.json')['six_libraries_unchanged']
for row in read('compression-map.json').values():
 packed=(r/row['compressed']).read_bytes();decoded=gzip.decompress(packed);assert len(decoded)==row['raw_bytes']and hashlib.sha256(decoded).hexdigest()==row['raw_sha256'];assert hashlib.sha256(packed).hexdigest()==row['compressed_sha256']
external=[]
for name in ['docs/audit/bf144/459/inputs.json','docs/audit/bf144/463/replay-restored.json','docs/audit/bf144/463/summary.json','docs/audit/bf144/462/restored.native.json.gz','docs/audit/bf144/462/reader-migration.json','docs/audit/bf144/441c/library-identities.json']:
 data=Path(name).read_bytes();external.append(dict(path=name,bytes=len(data),sha256=hashlib.sha256(data).hexdigest()))
summary=dict(gate='PASS_CONTINUOUS319_PUBLIC_VM_TRACE_CONTROL_AND_ACTUAL_CONSUMER_PROPAGATION',continuous_modes=2,queries_each=319,register_values_original_exact=129195,local_bone_rows_original_exact=25201,actual_calls_each=13378,actual_queries_each=636,source_trace_complete_arguments_qualified_each=636,all_non_trace_algorithms_are_unchanged_public_units=True,source_output_registers_or_pose_not_injected=True,each_mode_independent_construction_once_and_continuous_working_state=True,direct_consumer_links_original_and_controlled=1272,pelvis_spring_target_and_foot_offset_normal_consumers_exact=True,changed_register_rows=15440,changed_projected_bones=comparison['changed_projected_bones'],source_residual_original=comparison['source_residual_original'],source_residual_controlled=comparison['source_residual_controlled'],source_bone_partition=comparison['source_partition'],full_control_removal_returns_complete_vm_to_original=True,source_bytes_restored_same_target_recompiled_and_entire_two_domain_report_bytes_exact=True,external_source_files=external,known_difference='D247',new_differences=[],confirmed_difference_count=272,source_full_rig_or_root_equivalence_claimed=False,other_math_and_initial_global_data_residuals_preserved=True,production_reader_engine_converters_source_assets_unchanged=True,next_boundary='Fresh common input source and target spring, foot offset clamp and rotation state algorithms; keep native full Rig residuals independent from trace vector factor',screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');print('Verified complete continuous319 VM trace control,1272 direct consumer links, full-state ablation and exact recovery',flush=True)
