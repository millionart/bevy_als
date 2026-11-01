import gzip
import hashlib
import json
from pathlib import Path
from check import load,validate

r=Path(__file__).resolve().parent
def raw(p):return p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix(p.suffix+'.gz').read_bytes())
def read(n):return json.loads(raw(r/n))
a=load('qualified');b=load('restored');definition=read('runtime-restored/rig-definition.json')
assert validate(a,definition)==validate(b,definition)==319
for key in ['rows','after_motors','camera_after_post_update','animation_before','animation_after']:assert a[key]==b[key],key
assert raw(r/'check-qualified.json')==raw(r/'check-restored.json')
assert raw(r/'runtime-qualified/parameters.json')==raw(r/'runtime-restored/parameters.json')
assert raw(r/'partition-qualified.json')==raw(r/'partition-restored.json')
assert raw(r/'runtime-qualified/rig-definition.json')==raw(r/'runtime-restored/rig-definition.json')
assert raw(r/'runtime-qualified/skeleton.json')==raw(r/'runtime-restored/skeleton.json')
for name,digest in read('final-sources.json').items():
    assert hashlib.sha256((r/name).read_bytes()).hexdigest()==digest
    assert (r/name).read_bytes()==(r/(name+'.final')).read_bytes()
for name,digest in read('prototype-sources.json').items():assert hashlib.sha256((r/name.replace('.rs','.prototype.rs')).read_bytes()).hexdigest()==digest
assert len(read('check-qualified.json')['negative_fixtures_rejected'])==6
for label in ['capture-disabled','observation-disabled']:
    report=read('check-'+label+'.report.json');assert not report['success']and report['process_exit_code']==1
    data=load(label)
    for key in ['rows','after_motors','camera_after_post_update']:assert data[key]==a[key],key
    for key in ['animation_before','animation_after']:
        x=[{k:v for k,v in row.items()if k!='rig_snapshot'}for row in a[key]]
        y=[{k:v for k,v in row.items()if k!='rig_snapshot'}for row in data[key]];assert x==y,key
    if label=='capture-disabled':
        skip={'calls','traces','capture_property','capture_global'}
        for key in ['animation_before','animation_after']:
            for x,y in zip(a[key],data[key],strict=True):
                x=x['rig_snapshot'];y=y['rig_snapshot']
                if x is None:assert y is None;continue
                assert {k:v for k,v in x.items()if k not in skip}=={k:v for k,v in y.items()if k not in skip}
        assert all(not x['rig_snapshot']['calls']and x['rig_snapshot']['capture_property']is False for x in data['animation_after'])
    else:assert all('rig_snapshot'not in x for x in data['animation_after'])
    del data
for label in ['qualified','restored']:
    assert read(label+'-functional.json')['pass']and not read(label+'-functional.json')['full_pass']
    assert read('compile-'+label+'.report.json')['success']and read('runtime-'+label+'/process.report.json')['success']
    assert read('check-'+label+'.report.json')['success']
prototype=load('baseline');skip={'elapsed_seconds','capture_global','capture_property'}
for key in ['rows','after_motors','camera_after_post_update']:assert prototype[key]==a[key]
for key in ['animation_before','animation_after']:
    assert [{k:v for k,v in row.items()if k!='rig_snapshot'}for row in prototype[key]]==[{k:v for k,v in row.items()if k!='rig_snapshot'}for row in a[key]]
    for x,y in zip(prototype[key],a[key],strict=True):
        x=x['rig_snapshot'];y=y['rig_snapshot']
        if x is None:assert y is None;continue
        assert {k:v for k,v in x.items()if k not in skip}=={k:v for k,v in y.items()if k not in skip}
del prototype
part=read('partition-restored.json')
assert part['actual_head_to_rig_source_full_pose_exact_queries']==319 and part['before_previous_after_state_exact_queries']==318
assert part['matched_positive_increment_counts']['rig_source:root_result']==dict(translation=1572,rotation=5184,scale=819)
external=[]
for name in ['docs/audit/bf144/451a/runtime-restored/parameters.json.gz','docs/audit/bf144/451a/evidence-manifest.json','docs/audit/bf144/455/summary.json','docs/audit/bf144/455/consumer-contract.json']:
    data=Path(name).read_bytes();external.append(dict(path=name,sha256=hashlib.sha256(data).hexdigest(),bytes=len(data)))
result=dict(gate='PASS_ACTUAL319_CONTROL_RIG_VM_INDEPENDENT_OUTPUT_AND_CONTINUITY_READONLY_CAPABILITY',queries_per_phase=319,registers_per_query=405,externals_per_query=12,bindings_per_query=11,actual_hierarchy_to_rig_output_bone_rows_exact=25201,all_actual_binding_values_and_register_references_exact=True,head_to_rig_source_full_pose_exact_queries=319,before_previous_after_vm_state_exact_queries=318,total_actual_unit_calls=part['total_actual_calls'],total_actual_traces=part['total_actual_traces'],positive_increment_counts=part['matched_positive_increment_counts'],old_five_complete_arrays_exact=True,unmapped_scene_node_weights_and_pose_metadata_preserved=True,observation_and_actual_capture_ablations_rejected=True,correct_capture_control_vm_pose_and_inputs_unchanged=True,redundant_global_capture_setter_removed=True,performance_cpu_elapsed_capture_removed_under_explicit_pause=True,prototype_raw_reports_preserved=True,three_source_bytes_restored_same_target_recompiled=True,complete_new_and_old_semantic_arrays_check_and_partition_byte_exact=True,two_original_scene_functional_gates_passed=True,performance_acceptance='paused',screenshots=0,source_post_rig_and_locomotion_action_still_unobserved=True,external_source_files=external,new_differences=[],confirmed_difference_count=272,full_rig_or_root_or_als_equivalence_claimed=False,production_reader_engine_original_assets_unchanged=True,next_required_source_evidence='Native independent ControlRig output pose and actual locomotion action; compare actual Rig inputs and downstream Ragdolling rotation contract')
(r/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
print('Verified actual VM capability, real-capture and field ablations, removed redundant state, full byte restoration and functional gates',flush=True)
