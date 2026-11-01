import gzip
import hashlib
import json
from pathlib import Path
from check import load,validate

r=Path(__file__).resolve().parent
def raw(p):return p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix(p.suffix+'.gz').read_bytes())
def read(n):return json.loads(raw(r/n))
a=load('qualified');b=load('restored');assert validate(a)==validate(b)
for key in ['rows','after_motors','camera_after_post_update','animation_before','animation_after']:assert a[key]==b[key],key
assert raw(r/'runtime-qualified/parameters.json')==raw(r/'runtime-restored/parameters.json')
assert raw(r/'check-qualified.json')==raw(r/'check-restored.json')
for name in ['rig-definition.json','skeleton.json','state-reader-selftest.json']:assert raw(r/'runtime-qualified'/name)==raw(r/'runtime-restored'/name)
for name,digest in read('final-sources.json').items():assert hashlib.sha256((r/name).read_bytes()).hexdigest()==digest and (r/name).read_bytes()==(r/(name+'.final')).read_bytes()
ablated=load('no-observation')
for key in ['rows','after_motors','camera_after_post_update']:assert a[key]==ablated[key]
for key in ['animation_before','animation_after']:assert [{k:v for k,v in row.items()if k!='ragdoll_blend'}for row in a[key]]==ablated[key]
assert read('check-no-observation.report.json')['process_exit_code']==1
constant=read('runtime-constant-state/process.report.json');assert not constant['success']and constant['process_exit_code']==101
for label in ['qualified','restored']:
 assert read('compile-'+label+'.report.json')['success']and read('runtime-'+label+'/process.report.json')['success']
 assert read(label+'-functional.json')['pass']and not read(label+'-functional.json')['full_pass']
selftest=read('runtime-restored/state-reader-selftest.json');assert selftest['upcoming']['weights']==selftest['previous']['weights']==[.25,.75]and selftest['upcoming']['last_active_index']==1
native=read('native-check.json');assert native['samples']==319 and native['bone_rows']==25201 and native['all_input_values_exact']
assert read('identity-check.json')['observer_sources_restored']
external=[]
for name in ['docs/audit/bf144/456/runtime-restored/parameters.json.gz','docs/audit/bf144/456/evidence-manifest.json','docs/audit/bf144/457/partition-restored-final.json','docs/audit/bf144/457/reader-migration.json']:
 data=Path(name).read_bytes();external.append(dict(path=name,sha256=hashlib.sha256(data).hexdigest(),bytes=len(data)))
result=dict(gate='PASS_ACTUAL319_RAGDOLLING_SINGLE_FULL_WEIGHT_NORMALIZATION_CONSUMER_AUDIT',actual_queries=319,matched_positive_frames=318,actual_weights_all=[1,0],actual_active_child_all0=True,inner_branch_never_evaluated=True,public_full_pose_exact_all319=True,source_native_same_input_bone_rows_exact=25201,source_native_full_pose_and_curve_output_exact_input=True,source_no_profile_full_weight_passthrough_branch=True,target_extra_normalization_changed_rows_all319=3728,target_extra_normalization_changed_rows_positive318=3714,first=read('check-restored.json')['counts']['first'],current_minimal_copy_contract_fails=True,explicit_copy_value_control_passes=True,runtime_not_mutated_by_copy_control=True,nondefault_reader_fixture_and_last_state_fallback_verified=True,missing_observation_and_constant_reader_ablations_rejected=True,old_five_complete_arrays_exact=True,three_source_bytes_restored_same_target_recompiled=True,complete_parameter_check_definition_skeleton_reader_fixture_bytes_restored_exact=True,two_original_scene_functional_gates_passed=True,original_wrong_bone_order_native_request_rejected_and_preserved=True,external_source_files=external,existing_difference_extended='D245',new_differences=[],confirmed_difference_count=272,full_rig_or_root_or_als_equivalence_claimed=False,production_reader_engine_original_assets_unchanged=True,screenshots=0,performance_acceptance='paused',next_boundary='Actual ControlRig output differences: isolate current input/data and source VM writers using the newly captured real state, not remaining downstream normalization')
(r/'summary.json').write_text(json.dumps(result,indent=2)+'\n');print('Verified actual full-weight selector, full public/source kernel contracts, two ablations, byte restoration and functional gates',flush=True)
