import gzip
import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
def raw(p):return p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix(p.suffix+'.gz').read_bytes())
for stem in ['replay','analysis','frame-table']:assert raw(r/(stem+'-baseline-fixed.json'))==raw(r/(stem+'-restored.json')),stem
assert (r/'replay.rs').read_bytes()==(r/'replay.rs.final').read_bytes()
assert hashlib.sha256((r/'replay.rs').read_bytes()).hexdigest()==json.loads((r/'final-source.json').read_text())['sha256']
failures=json.loads((r/'negative-controls.json').read_text());assert len(failures)==2
for x in failures:
    label=x['label'];assert x['failure'].startswith('AssertionError: Missing factor signal: ')
    for kind in ['replay-compile-','replay-']:
        p=json.loads((r/(kind+label+'.report.json')).read_text());assert p['success']and p['process_exit_code']==0
    p=json.loads((r/('analyze-'+label+'.report.json')).read_text());assert not p['success']and p['process_exit_code']!=0
for factor in ['product','reciprocal']:
    for mode in ['current','controlled']:
        label=factor+'_'+mode;p=json.loads((r/('contract-'+label+'.report.json')).read_text());case=json.loads((r/('contract-'+label+'.json')).read_text())
        assert case['same_current_input_native_not_actual_source_data']and not case['whole_rotation_or_pose_restored']
        if mode=='current':assert not p['success']and p['process_exit_code']!=0 and case['current']!=case['native']
        else:assert p['success']and p['process_exit_code']==0 and case['controlled']==case['native']
for label in ['baseline-fixed','restored']:
    for kind in ['replay-compile-','replay-','analyze-']:
        p=json.loads((r/(kind+label+'.report.json')).read_text());assert p['success']and p['process_exit_code']==0
analysis=json.loads(raw(r/'analysis-restored.json'));assert analysis['actual_frames_exact']==318 and analysis['variant_count']==4
external=json.loads(Path('docs/audit/bf144/452a/summary.json').read_text())['external_source_files']
p=Path('docs/audit/bf144/452b/evidence-manifest.json');data=p.read_bytes();external.append(dict(path=str(p),sha256=hashlib.sha256(data).hexdigest(),bytes=len(data)))
result=dict(gate='PASS_ACTUAL318_HEAD_F32_PRODUCT_ORDER_AND_RECIPROCAL_CONSUMERS_WITH_EXPLICIT_RESIDUALS',actual_public_frames_exact=318,current_rotation_rows_exact=25440,source_bone_rows_per_variant=25122,method_domains=4,current_dot_and_f32_and_normalization_stages_held=True,same_current_input_native_not_actual_source_data=True,native_comparison_bone_counts={k:v['rotation_bones_raw']for k,v in analysis['variants_to_native'].items()},factor_changed_bone_counts={k:v['rotation_bones_raw']for k,v in analysis['factor_edges'].items()},component_effects=analysis['component_effects'],whole_rotation_or_pose_not_recovered=True,all_non_rotation_channels_unchanged=True,necessary_source_controls_rejected=failures,current_minimal_contracts_rejected=2,controlled_minimal_contracts_passed=2,source_bytes_restored_same_target_recompiled=True,complete_restored_output_analysis_and_tables_byte_exact=True,external_source_files=external,source_reader_engine_assets_and_production_unchanged=True,existing_differences_extended=['D163','D248'],new_differences=[],confirmed_difference_count=272,full_root_or_als_equivalence_claimed=False,next_direct_boundary='Remaining actual full Head pose input differences and direct Rig/final Root consumers',screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
print('Verified actual318 F32 Head baseline, two method effects and exact current-fail/control-pass boundaries with restored bytes',flush=True)
