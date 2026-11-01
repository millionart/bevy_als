import gzip
import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
def raw(name):
 p=r/name
 return p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix(p.suffix+'.gz').read_bytes())
def read(name):return json.loads(raw(name))
assert raw('replay.rs')==raw('replay.rs.final')and raw('prepare.py')==raw('prepare.py.final')
assert hashlib.sha256(raw('replay.rs')).hexdigest()==read('final-source.json')['sha256']
assert raw('inputs.json')==raw('inputs.canonical.json')and raw('lifetime-proof.json')==raw('lifetime-proof.canonical.json')
assert raw('replay-typed-source.json')==raw('replay-restored-final.json')
a=read('arguments-check.json');assert a['calls']==636 and a['arguments']==6360
assert a['differences']['OffsetLocationZ']==588 and a['differences']['OffsetNormal']==636 and all(v==0 for k,v in a['differences'].items()if k not in ['OffsetLocationZ','OffsetNormal'])
l=read('lifetime-proof.json');assert l['all_arguments_preserved_after_actual_call']and l['initial_source_construction_prefix']==15
assert read('prepare-bad-lifetime.report.json')['process_exit_code']==1
assert read('prepare-bad-lifetime-no-guard.report.json')['process_exit_code']==0
assert read('lifetime-guard-removed.fixture.json')['representative_lifetimes'][0]['overwriting_writes']
assert read('replay-no-source-normal.report.json')['process_exit_code']==101
ablation=read('replay-no-source-normal.json');assert ablation['source_normal_restored']==ablation['normal_only_changed_offsets']==0
v=read('replay-restored-final.json');assert v['calls']==v['actual_requests_and_regenerated_hits_exact']==v['public_complete_arguments_exact']==v['post_hit_current_model_exact']==v['source_normal_restored']==636
assert v['normal_only_changed_offsets']==576
for label in ['typed-source','restored-final']:
 assert read('replay-'+label+'.report.json')['success']and read('replay-compile-'+label+'.report.json')['success']
partition=read('offset-partition.json');assert partition==dict(preserved=48,recovered=44,regressed=0,still_different=544)
assert read('identity-check.json')['six_libraries_unchanged']
assert read('rig.native.json.reader.report.json')['success']
external=[]
for name in ['docs/audit/bf144/459/inputs.json','docs/audit/bf144/460/common-rig.native.json.gz','docs/audit/bf144/460/summary.json','docs/audit/bf144/457/reader-migration.json','docs/audit/bf144/441c/library-identities.json']:
 data=Path(name).read_bytes();external.append(dict(path=name,bytes=len(data),sha256=hashlib.sha256(data).hexdigest()))
summary=dict(gate='PASS_ACTUAL636_SOURCE_TRACE_ABI_LIFETIME_PUBLIC_UNIT_AND_KNOWN_NORMAL_FACTOR',source_frames=319,source_calls=636,source_argument_values=6360,all_eight_input_arguments_match_by_declared_types_and_physical_basis=True,source_trace_arguments_no_later_aliased_write=True,source_initial_visited_prefix_constructor15_forward75_qualified=True,actual_target_requests_hits_complete_unit_arguments_and_post_hit_model_exact=636,source_normal_restored_by_only_source_inverse_rotate=636,normal_only_changed_offsets=576,source_offset_partition=partition,known_difference='D247',source_scalar_output_differences_before_control=588,source_normal_output_differences_before_control=636,source_internal_query_world_start_end_or_hits_not_captured=True,full_source_query_equivalence_not_claimed=True,source_float_centimeters_not_treated_as_identical_raw_runtime_float_meters=True,two_physical_ablation_signals_and_exact_source_input_restoration=True,same_six_library_target_recompiled_after_complete_restoration=True,complete_restored_output_bytes_exact=True,external_source_files=external,new_differences=[],confirmed_difference_count=272,production_reader_engine_assets_unchanged=True,next_boundary='Native API world ray and hit equivalence for lifetime-proven source trace inputs, then isolate remaining query and source centimeter float offset stages; full continuous Rig output still open',screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print('Verified complete636 trace ABI, no later source overwrite, actual public unit/query and known normal-only factor',flush=True)
