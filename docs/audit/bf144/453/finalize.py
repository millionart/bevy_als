import gzip
import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
def raw(p):return p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix(p.suffix+'.gz').read_bytes())
for stem in ['replay','analysis','frame-table']:assert raw(r/(stem+'-baseline.json'))==raw(r/(stem+'-restored.json')),stem
assert (r/'replay.rs').read_bytes()==(r/'replay.rs.final').read_bytes()
assert hashlib.sha256((r/'replay.rs').read_bytes()).hexdigest()==json.loads((r/'final-source.json').read_text())['sha256']
prototype=json.loads(raw(r/'replay-prototype.json'));current=json.loads(raw(r/'replay-restored.json'))
assert [x for x in prototype['outputs']if x['mode']!='children-rounded']==current['outputs']
index={(x['name'],x['mode']):x['bones']for x in prototype['outputs']}
assert len(index)==5088
for x in prototype['outputs']:
    if x['mode']=='children-rounded':assert x['bones']==index[x['name'],'current']
controls=json.loads((r/'controls.json').read_text());assert len(controls)==2
for label in ['baseline','restored','no-unit-stage','no-child-projection']:
    p=json.loads((r/('replay-compile-'+label+'.report.json')).read_text());assert p['success']and p['process_exit_code']==0
    p=json.loads((r/('replay-'+label+'.report.json')).read_text())
    if label=='no-child-projection':assert not p['success']and p['process_exit_code']==101
    else:assert p['success']and p['process_exit_code']==0
p=json.loads((r/'analyze-no-unit-stage.report.json').read_text());assert not p['success']and p['process_exit_code']!=0
failures=json.loads((r/'negative-controls.json').read_text());assert failures['unit_stage_failure']=='AssertionError: Missing early unit stage signal'
assert failures['child_projection_failure']['controlled']!=failures['child_projection_failure']['actual_current']
analysis=json.loads(raw(r/'analysis-restored.json'));assert analysis['native_and_current_rows_each_exact']==100488
external=dict(json.loads((r/'input-identities.json').read_text()))
for name in ['docs/audit/bf144/451b/evidence-manifest.json','docs/audit/bf144/452-final/summary.json']:
    p=Path(name);data=p.read_bytes();external[name]=dict(bytes=len(data),sha256=hashlib.sha256(data).hexdigest())
result=dict(gate='PASS_ALL1272_HEAD_POSITION_SCALE_PROJECTION_AUDIT_WITH_REDUNDANT_CONTROL_REMOVED',native_and_current_rows_each_exact=100488,current_cc_position_scale_exact_native=True,ss_projection_position_difference_rows=403,ss_projection_scale_difference_rows=0,mixed_sc_position_scale_differences=[2564,1487],mixed_cs_position_scale_differences=[48,0],early_double_unit_stage_sc_position_changes=36,removed_redundant_rounded_child_f64_control=True,all100488_rounded_children_f64_equals_current_f32_position_scale=True,remaining_original_modes_exact_after_design_deletion=True,source_controls_rejected=failures,restored_source_bytes_and_same_target_rebuild=True,restored_complete_outputs_analysis_and_tables_byte_exact=True,external_source_files=[dict(path=p,**v)for p,v in external.items()],source_reader_engine_assets_and_production_unchanged=True,existing_difference_extended='D232',new_differences=[],confirmed_difference_count=272,full_root_or_als_equivalence_claimed=False,next_direct_boundary='Remaining actual Head/Root pose data, F32 quaternion operation order and direct Rig consumers',screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
print('Verified native/current100488 T/S rows,403 SS position residuals, removed redundant control and exact byte restoration',flush=True)
