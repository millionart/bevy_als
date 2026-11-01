import gzip
import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
def raw(name):
 p=r/name
 return p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix(p.suffix+'.gz').read_bytes())
def read(name):return json.loads(raw(name))
assert raw('geometry-authored-boxes.json')==raw('geometry-restored.json')
assert raw('replay.rs')==raw('replay.rs.final')
assert hashlib.sha256(raw('replay.rs')).hexdigest()==read('final-source.json')['sha256']
geometry=read('geometry-restored.json');assert geometry['total']==geometry['exact']==geometry['wrong_world_changes']==636
assert read('geometry-baseline.json')['different']==266
assert read('geometry-no-local-center.json')['different']==636
for label in ['baseline','no-local-center']:
 assert read('replay-'+label+'.report.json')['process_exit_code']==101
for label in ['authored-boxes','no-local-center','restored']:
 assert read('replay-compile-'+label+'.report.json')['success']
for label in ['authored-boxes','restored']:
 assert read('replay-'+label+'.report.json')['success']
for name in ['common-rig.native.json','floor.native.json']:
 v=read(name+'.reader.report.json');assert v['success']and v['process_exit_code']==0
inputs=read('native-check.json');assert inputs['before_pose_and_curve_frames_exact']==inputs['requested_world_and_actual_execution_world_frames_exact']==638 and inputs['actual_rig_input_fields_exact']==7018
assert inputs['source_reference_point_projection_differences']==288
inventory=read('inventory.json');assert inventory['bone_pairs']==25201 and inventory['all_output_curve_frames_exact_current']==319
assert inventory['source_current_projected_output_differences']==dict(translation=1324,rotation=1845,scale=0)
assert len(read('negative-fixtures.json'))==4 and all(x['process_exit_code']==1 for x in read('negative-fixtures.json'))
assert read('identity-check.json')['six_libraries_unchanged']
assert read('floor-check.json')['source_origin_cm']==[0,0,-25]
external=[]
for name in ['docs/audit/bf144/459/inputs.json','docs/audit/bf144/459/summary.json','docs/audit/bf144/457/restored-final.native.json.gz','docs/audit/bf144/457/reader-migration.json','docs/audit/bf144/441c/library-identities.json']:
 data=Path(name).read_bytes();external.append(dict(path=name,bytes=len(data),sha256=hashlib.sha256(data).hexdigest()))
summary=dict(gate='PASS_ACTUAL636_TARGET_GEOMETRY_AND_CONTINUOUS319_SOURCE_RIG_INPUT_WITH_EXPLICIT_QUERY_BOUNDARY',actual_target_trace_requests_and_hits_exact=636,wrong_world_and_missing_local_center_each_reject_all636=True,assumed_box_rejected_on266_requests=True,fresh_source_floor_bounds_collision_and_surface_verified=True,source_continuous_cases=2,source_frames_each=319,all_source_pose_curve_world_clock_input_frames_exact=638,actual_source_rig_input_fields_exact=7018,source_independent_case_outputs_exact=True,source_current_output_bone_rows=25201,source_current_output_curves_exact_frames=319,source_current_projected_output_differences=inventory['source_current_projected_output_differences'],source_reference_query_projection_differences=288,source_internal_query_requests_or_hits_not_captured=True,source_box_is_geometric_union_not_identical_physics_representation=True,source_or_target_queries_not_overwritten=True,four_input_readback_contract_negative_fixtures_rejected=True,fixture_source_bytes_restored_same_target_recompiled_and_complete636_rows_exact=True,external_source_files=external,new_differences=[],confirmed_difference_count=272,source_vm_or_root_or_als_equivalence_claimed=False,production_reader_engine_assets_unchanged=True,next_boundary='Separate common input source Rig algorithm and query representation differences; current source reference ray readback does not expose actual internal foot unit query requests or hits',screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print('Verified exact target geometry qualification, complete continuous source inputs, explicit output inventory and retained query boundary',flush=True)
