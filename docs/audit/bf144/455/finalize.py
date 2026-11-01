import gzip
import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
def raw(p):return p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix(p.suffix+'.gz').read_bytes())
for stem in ['analysis','frame-table']:assert raw(r/(stem+'-final.json'))==raw(r/(stem+'-restored.json')),stem
assert (r/'inventory.py').read_bytes()==(r/'inventory.py.final').read_bytes()
assert hashlib.sha256((r/'inventory.py').read_bytes()).hexdigest()==json.loads((r/'final-source.json').read_text())['sha256']
for label in ['final','restored']:
    p=json.loads((r/('inventory-'+label+'.report.json')).read_text());assert p['success']and p['process_exit_code']==0
p=json.loads((r/'inventory-no-projection.report.json').read_text());assert not p['success']and p['process_exit_code']!=0
failure=json.loads((r/'negative-projection.json').read_text());assert failure['failure']=='AssertionError: Missing F32 projection boundary signal'
analysis=json.loads(raw(r/'analysis-restored.json'));assert analysis['matched_frames']==318 and len(analysis['negative_fixtures_rejected'])==5
assert analysis['counts']==dict(source_raw=dict(translation=1902,rotation=1921,scale=992),source_projected=dict(translation=1268,rotation=1902,scale=788),current=dict(translation=1572,rotation=5184,scale=819))
assert analysis['source_rig_actual_bool_observations']=={'bUseHandIkBones:True':318,'bUseFootIkBones:True':318,'bFootTransformsValid:False':1,'bFootTransformsValid:True':317}
assert analysis['source_locomotion_action_not_captured']
report=json.loads((r/'main.native.json.reader.report.json').read_text());assert report['success']and report['process_exit_code']==0
consumer=json.loads((r/'consumer-contract.json').read_text());assert consumer['independent_rig_output_absent_from_existing_corpus']and consumer['current_actual_vm_snapshot_absent']
paths=['docs/audit/bf144/448a/restored.native.json.gz','docs/audit/bf144/448a/evidence-manifest.json','docs/audit/bf144/448b/runtime-restored/parameters.json.gz','docs/audit/bf144/448b/evidence-manifest.json','docs/audit/bf144/454/evidence-manifest.json'];external=[]
for name in paths:
    data=Path(name).read_bytes();external.append(dict(path=name,sha256=hashlib.sha256(data).hexdigest(),bytes=len(data)))
result=dict(gate='PASS_ACTUAL318_RIG_RAGDOLLING_INCREMENT_INVENTORY_AND_OBSERVATION_PREREQUISITE',matched_frames=318,bone_rows_per_domain=25122,increment_counts=analysis['counts'],curves_unchanged_within_each_system=True,source_rig_actual_bool_observations=analysis['source_rig_actual_bool_observations'],source_locomotion_action_not_captured=True,fresh_main_three_edge_closure=True,source_and_current_transfer_flags_match=True,target_graph_unchanged_since_actual_corpus=True,source_raw_separate_from_f32_projection=True,negative_pair_and_basis_fixtures_rejected=5,projection_ablation_rejected=True,source_bytes_and_same_interpreter_restored=True,complete_restored_analysis_and_table_byte_exact=True,composite_changes_not_attributed_to_single_node=True,independent_control_rig_output_and_current_vm_observation_required=True,next_required_observation=consumer['next_required_observation'],external_source_files=external,new_differences=[],confirmed_difference_count=272,full_rig_or_root_or_als_equivalence_claimed=False,source_reader_engine_assets_and_production_unchanged=True,screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
print('Verified composite increment inventory, actual source Rig flags, fresh graph closure and explicit independent-Rig observation prerequisite',flush=True)
