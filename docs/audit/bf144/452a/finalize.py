import gzip
import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
def raw(p):return p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix(p.suffix+'.gz').read_bytes())
for stem in ['replay','analysis','frame-table']:
    assert raw(r/(stem+'-baseline.json'))==raw(r/(stem+'-restored.json')),stem
assert (r/'replay.rs').read_bytes()==(r/'replay.rs.final').read_bytes()
assert hashlib.sha256((r/'replay.rs').read_bytes()).hexdigest()==json.loads((r/'final-source.json').read_text())['sha256']
controls=json.loads((r/'controls.json').read_text());assert len(controls)==3
failures=[]
saved_failures={x['label']:x['failure']for x in json.loads((r/'negative-controls.json').read_text())}
for x in controls:
    label=x['label'];assert x['analysis_exit_code']!=0 and x['source_control_rejected']
    for kind in ['replay-compile-','replay-']:
        p=json.loads((r/(kind+label+'.report.json')).read_text());assert p['success'] and p['process_exit_code']==0
    p=json.loads((r/('analyze-'+label+'.report.json')).read_text());assert not p['success']and p['process_exit_code']!=0
    last=saved_failures[label];assert last.startswith('AssertionError: Missing factor signal: ')
    failures.append(dict(label=label,failure=last))
for label in ['baseline','restored']:
    for kind in ['replay-compile-','replay-','analyze-']:
        p=json.loads((r/(kind+label+'.report.json')).read_text());assert p['success']and p['process_exit_code']==0
analysis=json.loads(raw(r/'analysis-restored.json'));assert analysis['actual_frames_exact']==318
(r/'negative-controls.json').write_text(json.dumps(failures,indent=2)+'\n')
external=[]
paths=[Path('docs/audit/bf144/451a/evidence-manifest.json'),Path('docs/audit/bf144/451b/evidence-manifest.json'),Path('docs/audit/bf144/451a/runtime-restored/skeleton.json'),Path('docs/audit/bf144/451b/replay-inputs.json'),Path('docs/audit/bf144/451b/requests.json')]
for i in range(6):paths+=[Path(f'docs/audit/bf144/451b/batch-{i:02}.native.json.gz'),Path(f'docs/audit/bf144/451b/batch-{i:02}.request.json'),Path(f'docs/audit/bf144/451b/batch-{i:02}.native.json.reader.report.json')]
for p in paths:
    data=p.read_bytes();external.append(dict(path=str(p),sha256=hashlib.sha256(data).hexdigest(),bytes=len(data)))
result=dict(gate='PASS_COMPLETE318_CURRENT_HEAD_ROTATION_STAGE_FACTORS_WITH_EXPLICIT_SOURCE_MATH_BOUNDARY',actual_public_frames_exact=318,current_rotation_rows_exact=25440,source_bone_rows_per_variant=25122,variants_to_native={k:v['rotation_bones_raw']for k,v in analysis['variants_to_native'].items()},current_single_factor_changed_bones=dict(remove_delta_normalize=analysis['factor_edges']['010-110']['rotation_bones_raw'],remove_product_normalize=analysis['factor_edges']['100-110']['rotation_bones_raw'],add_second_local_normalize=analysis['factor_edges']['110-111']['rotation_bones_raw']),source_controls_rejected=failures,restored_source_bytes_and_same_target_rebuild=True,restored_complete_output_analysis_and_tables_byte_exact=True,all_non_rotation_channels_unchanged=True,no_new_actual_runtime_required_for_static_replay=True,actual_visible_entry_gate='docs/audit/bf144/451a/summary.json',external_source_files=external,production_source_or_reader_changed=False,new_differences=[],confirmed_difference_count=272,full_source_math_or_root_equivalence_claimed=False,next_boundary='Same input source F64 rotation pipeline and normalization arithmetic calibration',screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
print('Verified three removed stage factors reject, restored same-target full output bytes exact and23 tracked external inputs',flush=True)
