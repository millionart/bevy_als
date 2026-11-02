import gzip
import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent;read=lambda name:json.loads((r/name).read_bytes())
assert(r/'replay.rs').read_bytes()==(r/'replay.rs.final').read_bytes();assert hashlib.sha256((r/'replay.rs').read_bytes()).hexdigest()==read('final-source.json')['sha256'];assert(r/'replay-baseline.json').read_bytes()==(r/'replay-no-source-product.json').read_bytes()==(r/'replay-restored.json').read_bytes()
for label in['baseline','no-source-product','restored']:assert read('replay-compile-'+label+'.report.json')['success']and read('replay-'+label+'.report.json')['success']
for label in['no-source-basis','no-source-data-simplified']:assert read('replay-compile-'+label+'.report.json')['success']and read('replay-'+label+'.report.json')['process_exit_code']==101
assert read('check-current-contract.report.json')['process_exit_code']==1 and read('identity-check.json')['six_libraries_unchanged']
life=read('lifecycle.json');assert life['calls']==684 and life['streams']==10 and life['source_before_first_call_defaults_verified']==10
comparison=read('comparison.json');assert comparison['source_relative_values_exact']==684 and comparison['actual_current_cache_consumers_exact']==1272
assert comparison['counts']['source_data_source_product']==dict(double_differences=0,f32_differences=0)
for name,counts in comparison['counts'].items():
 if name!='source_data_source_product':assert counts==dict(double_differences=684,f32_differences=0)
assert read('simplification.json')['handwritten_neon_swizzle_product_removed']
for row in read('compression-map.json').values():
 packed=(r/row['compressed']).read_bytes();raw=gzip.decompress(packed);assert len(raw)==row['raw_bytes']and hashlib.sha256(raw).hexdigest()==row['raw_sha256'];assert hashlib.sha256(packed).hexdigest()==row['compressed_sha256']
external=[]
for name in['docs/audit/bf144/469/inputs.json','docs/audit/bf144/469a/restored.native.json.gz','docs/audit/bf144/469a/controls-restored.native.json.gz','docs/audit/bf144/469a/reader-migration.json','docs/audit/bf144/441c/library-identities.json','docs/audit/bf144/441b/identities.start.json','tools/animgraph_authoring/ue_control_rig_contract.py','tools/animgraph_authoring/ue_control_rig_target.py','tools/run_ue_source_reader.ps1']:
 raw=Path(name).read_bytes();external.append(dict(path=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest()))
summary=dict(gate='PASS_ACTUAL684_INITIAL_RELATIVE_SOURCE_QUATERNION_DATA_AND_BASIS_ORDER_FACTORS',source_values_exact=684,current_observed_cache_consumers_exact=1272,source_before_first_call_defaults_verified=10,initial_relative_cache_lifetime_streams=10,source_initial_inputs_and_cache_constant_per_stream=True,source_after_cache_values_not_injected=True,complete_data_basis_matrix=comparison['counts'],native_input_data_and_ue_basis_product_both_required=True,current_common_source_data_contract_rejected=True,source_basis_and_data_physical_ablations_rejected=True,unnecessary_handwritten_simd_scalar_formula_removed_after_complete_report_bytes_identical=True,existing_library_product_qualified_only_in_this_initial_input_domain=True,no_general_product_or_calf_angle_equivalence_claimed=True,source_bytes_restored_same_target_recompiled_complete_matrix_bytes_exact=True,external_source_files=external,known_differences=['D242','D163'],new_differences=[],confirmed_difference_count=272,production_reader_engine_converters_source_assets_unchanged=True,full_foot_rotation_rig_or_root_equivalence_claimed=False,next_boundary='Source current and target calf-space quaternion chains, quaternion-to-Euler source primitives and angle limits, then final quaternion and full Rig feedback',screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');print('Verified684 native initial relative values,1272 actual caches, data/basis factor necessity and simplified byte-exact recovery',flush=True)
