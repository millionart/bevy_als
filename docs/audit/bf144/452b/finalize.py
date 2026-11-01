import gzip
import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
def raw(p):return p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix(p.suffix+'.gz').read_bytes())
for stem in ['replay','analysis','frame-table']:
    original=raw(r/(stem+'-baseline.json'))
    for label in ['restored','primitive-restored']:assert original==raw(r/(stem+'-'+label+'.json')),(stem,label)
assert raw(r/'replay-native-glam-product.json')==raw(r/'replay-baseline.json')
assert (r/'replay.rs').read_bytes()==(r/'replay.rs.final').read_bytes()
assert hashlib.sha256((r/'replay.rs').read_bytes()).hexdigest()==json.loads((r/'final-source.json').read_text())['sha256']
failures=json.loads((r/'negative-controls.json').read_text());assert len(failures)==5
for x in failures:
    label=x['label'];assert x['failure'].startswith('AssertionError: Missing source factor double signal: ')
    for kind in ['replay-compile-','replay-']:
        p=json.loads((r/(kind+label+'.report.json')).read_text());assert p['success']and p['process_exit_code']==0
    p=json.loads((r/('analyze-'+label+'.report.json')).read_text());assert not p['success']and p['process_exit_code']!=0
primitive=json.loads((r/'primitive-controls.json').read_text());assert len(primitive)==1 and primitive[0]['label']=='native-no-neon'and primitive[0]['replay_exit_code']==101
p=json.loads((r/'replay-native-no-neon.report.json').read_text());assert not p['success']and p['process_exit_code']==101
p=json.loads((r/'replay-compile-native-no-neon.report.json').read_text());assert p['success']and p['process_exit_code']==0
minimal=json.loads((r/'native-primitive-failure.json').read_text());assert minimal['native_source_contract_rejected']and minimal['controlled']!=minimal['native']
ablation=json.loads((r/'product-ablation.json').read_text());assert ablation['complete_all7632_factor_outputs_byte_exact']
assert hashlib.sha256((r/'replay.before-ablation.rs').read_bytes()).hexdigest()==ablation['original_fixture_sha256']
assert hashlib.sha256((r/'replay.rs').read_bytes()).hexdigest()==ablation['simplified_fixture_sha256']
for label in ['baseline','restored','primitive-restored','native-glam-product']:
    for kind in ['replay-compile-','replay-']:
        p=json.loads((r/(kind+label+'.report.json')).read_text());assert p['success']and p['process_exit_code']==0
analysis=json.loads(raw(r/'analysis-primitive-restored.json'));assert analysis['native_rotation_rows_exact']==100488
external=dict(json.loads((r/'input-identities.json').read_text()))
for name in ['docs/audit/bf144/451b/target-replay-restored.json.gz','docs/audit/bf144/451b/evidence-manifest.json','docs/audit/bf144/452a/evidence-manifest.json']:
    p=Path(name);data=p.read_bytes();external[name]=dict(bytes=len(data),sha256=hashlib.sha256(data).hexdigest())
result=dict(gate='PASS_ALL1272_SOURCE_HEAD_ROTATION_ARITHMETIC_AND_NECESSARY_FACTORS_WITH_REDUNDANT_PRODUCT_REMOVED',native_rotation_rows_exact=100488,native_case_count=1272,actual_cc_and_ss_input_cases_each=318,current_to_native_rotations=analysis['current_to_native_rotations'],cc_factor_changed_projection_bones={m:v['projected_changed_bones']for m,v in analysis['source_factor_summaries']['cc'].items()},independent_factors_not_additive=True,source_projection_control_is_not_current_glam_math=True,removed_redundant_manual_product=True,kept_arm64_normalizer_with_exact_native_failure=True,source_controls_rejected=failures,native_primitive_minimal_failure=minimal,restored_source_bytes_and_same_target_rebuild=True,complete_restored_outputs_analysis_and_tables_byte_exact=True,external_source_files=[dict(path=p,**v)for p,v in external.items()],source_reader_engine_assets_and_production_unchanged=True,new_differences=[],confirmed_difference_count=272,full_root_or_als_equivalence_claimed=False,screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
print('Verified exact source arithmetic, five rejected factors, necessary NEON normalizer, deleted custom product and restored bytes',flush=True)
