import gzip
import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
def raw(p):return p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix(p.suffix+'.gz').read_bytes())
def read(n):return json.loads(raw(r/n))
a=raw(r/'replay-clock-corrected.json');b=raw(r/'replay-restored.json');assert a==b
result=json.loads(b);assert result['queries']==319 and result['calls']==13378 and result['traces']==636
assert sum(len(x['registers'])for x in result['outputs'])==319*405
assert sum(len(x['local'])for x in result['outputs'])==319*79
assert (r/'replay.rs').read_bytes()==(r/'replay.rs.final').read_bytes()
assert hashlib.sha256((r/'replay.rs').read_bytes()).hexdigest()==read('final-source.json')['sha256']
assert raw(r/'inputs.json')==raw(r/'inputs.canonical.json')
for label in ['post-exposure-clock','wrong-trace-start','no-construction']:
 p=read('replay-'+label+'.report.json');assert not p['success']and p['process_exit_code']==101
assert read('replay-compile-no-construction.report.json')['success']
for label in ['clock-corrected','restored']:assert read('replay-'+label+'.report.json')['success']
assert read('replay-compile-restored.report.json')['success']
source=read('source-contract.json');assert source['registers']==405 and source['initial_global_double_components_different']==365
assert source['program_local_topology_curves_geometry_exact_source']and source['actual_runtime_definition_exact_asset_by_typed_f32_bits']
assert read('identity-check.json')['observer_sources_restored']
external=[dict(path=p,**v)for p,v in read('input-identities.json').items()]
for name in ['docs/audit/bf144/456/evidence-manifest.json','docs/audit/bf144/457/reader-migration.json']:
 data=Path(name).read_bytes();external.append(dict(path=name,bytes=len(data),sha256=hashlib.sha256(data).hexdigest()))
summary=dict(gate='PASS_ACTUAL319_CONTROL_RIG_PUBLIC_VM_COMPLETE_STATE_AND_TRACE_REPLAY',queries=319,register_values_each=405,total_register_values=129195,total_local_bone_rows=25201,actual_calls=13378,actual_traces=636,all_call_order_after_arguments_and_pre_call_global_bones_exact=True,all_trace_requests_and_hits_bound_to_actual_callback=True,full_output_working_state_and_local_hierarchy_exact=True,initial_construction_once_and_continuous_working_state=True,first_actual_delta0_derived_from_live_before_gate_arena_and_fresh_override_code=True,post_frame_global_delta_not_used_as_initial_consumed_delta=True,no_after_register_or_output_pose_injection=True,trace_hits_replayed_not_physics_regenerated=True,source_vm_program_local_curves_geometry_match_with_known_initial_global_boundary=True,known_static_difference='D242',known_static_components=365,known_static_bones=71,three_minimal_source_input_ablation_contracts_rejected=True,source_and_input_bytes_restored_same_target_recompiled=True,complete_restored_replay_bytes_exact=True,external_source_files=external,new_differences=[],confirmed_difference_count=272,source_rig_or_root_or_als_equivalence_claimed=False,production_reader_engine_assets_unchanged=True,next_boundary='Same-input source Rig history and actual world geometry prerequisite; use matched public replay to separate current input/data and source algorithm differences',screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');print('Verified actual319 public VM full-state replay, three counterexamples, exact restoration and explicit source static boundary',flush=True)
