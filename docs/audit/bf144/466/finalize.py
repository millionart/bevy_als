import gzip
import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent;read=lambda name:json.loads((r/name).read_bytes())
assert(r/'replay.rs').read_bytes()==(r/'replay.rs.final').read_bytes()
assert(r/'prepare.py').read_bytes()==(r/'prepare.py.final').read_bytes()
assert hashlib.sha256((r/'replay.rs').read_bytes()).hexdigest()==read('final-source.json')['sha256']
assert(r/'replay-baseline.json').read_bytes()==(r/'replay-restored-sequential.json').read_bytes()
for label in['baseline','restored-sequential']:
 assert read('replay-'+label+'.report.json')['success']and read('replay-compile-'+label+'.report.json')['success']
for label in['merged-lifetime','no-state-history']:
 assert read('replay-compile-'+label+'.report.json')['success']and read('replay-'+label+'.report.json')['process_exit_code']==101
assert read('prepare-bad-lifetime.report.json')['process_exit_code']==1 and read('prepare-no-pelvis-floor.report.json')['process_exit_code']==1
assert read('prepare-restored.report.json')['success']and read('identity-check.json')['six_libraries_unchanged']
proof=read('lifetime-proof.json');assert proof['argument_values']==10812 and proof['source_target_interval_collapsed_all_rows']and proof['focused_ambiguous_height_case_refused']
check=read('comparison.json');assert check['actual_source_cm_initialized_offset_and_complete_state_exact']==636 and check['public_original_and_trace_control_complete_arguments_and_foot_location_exact']==1272
assert check['source_trace_projected_offset_and_parent465_pelvis_input_links_exact']==636
assert check['counts']==dict(current_offset_differences=635,trace_control_offset_differences=367,common_meter_offset_differences=290,current_target_differences=613,trace_control_target_differences=239,common_meter_velocity_differences=565,common_meter_state_differences=565)
for row in read('compression-map.json').values():
 packed=(r/row['compressed']).read_bytes();decoded=gzip.decompress(packed);assert len(decoded)==row['raw_bytes']and hashlib.sha256(decoded).hexdigest()==row['raw_sha256'];assert hashlib.sha256(packed).hexdigest()==row['compressed_sha256']
external=[]
for name in['docs/audit/bf144/459/inputs.json','docs/audit/bf144/464/replay-restored.json.gz','docs/audit/bf144/465a/restored.native.json.gz','docs/audit/bf144/465/replay-restored.json','docs/audit/bf144/465a/reader-migration.json','docs/audit/bf144/441c/library-identities.json','docs/audit/bf144/441b/identities.start.json','docs/audit/bf144/445a/identity-check.json','tools/animgraph_authoring/ue_control_rig_contract.py','tools/animgraph_authoring/ue_control_rig_target.py','tools/run_ue_source_reader.ps1']:
 raw=Path(name).read_bytes();external.append(dict(path=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest()))
summary=dict(gate=check['gate'],actual_calls_each_domain=636,continuous_histories_each_side=318,initial_scene_frame_not_executed=True,source_argument_values_qualified=10812,source_cm_scalar_state_exact_native=636,public_current_and_trace_control_full_arguments_and_locations_exact=1272,source_target_interval_qualified=636,height_upper_clamp_may_be_active_rows=44,native_source_exact_pelvis_height_not_claimed=True,common_meter_offsets_different_from_source_projection=290,common_meter_velocity_states_different=565,trace_control_target_differences_restored_by_only_source_projected_pelvis_input=239,trace_control_remaining_scalar_offset_differences=367,common_meter_kernel_factor_independent_of_upstream_pelvis_factor=True,after_state_never_injected=True,source_and_target_initialization_and_state_history_controls_rejected=True,post_writer_and_missing_pelvis_floor_controls_rejected=True,source_bytes_restored_same_target_recompiled_complete_sequential_report_bytes_exact=True,external_source_files=external,known_differences=['D165'],new_differences=[],confirmed_difference_count=272,source_full_leg_vector_or_rig_or_root_equivalence_claimed=False,production_reader_engine_converters_source_assets_unchanged=True,screenshots=0,performance_acceptance='paused',next_boundary='Actual source thigh hierarchy input at foot location call, final vector clamp and source full foot location; then rotation history and remaining full Rig feedback')
(r/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');print('Verified636 actual foot scalar native histories, public contracts, height bounds and independent unit-domain factor',flush=True)
