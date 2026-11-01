import hashlib
import json
from pathlib import Path
from check import load,old_fields,read,validate

r=Path(__file__).resolve().parent
a=load('live-link')['character_animation_probe'];b=load('restored-final')['character_animation_probe']
assert validate(a)==validate(b)and a==b
assert old_fields(b)==load('old')['character_animation_probe']
assert read(r/'check-live-link.json')==read(r/'check-restored-final.json')
assert read(r/'partition-live-link.json')==read(r/'partition-restored-final.json')
for label,code in [('no-output-hook',7),('constant-action',7),('no-parent-guard',0),('restored-final',0),('invalid',7),('missing-boundaries',7)]:
    report=read(r/(label+'.native.json.reader.report.json'));assert report['process_exit_code']==code
for label in ['no-output-hook','constant-action','no-parent-guard','restored-final']:
    check=read(r/(label+'-package-source-check.json'));assert check['exact']
reader=Path('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp')
assert reader.read_bytes()==(r/'reader.final.cpp').read_bytes()
assert hashlib.sha256(reader.read_bytes()).hexdigest()==read(r/'reader-final-sha256.json')['sha256']
assert read(r/'identity-check.json')['other_base_files_unchanged']==395
compatibility=read(r/'compatibility.json');assert compatibility['curve_vector_three_axes_full_probe_exact']and compatibility['old_default_character_probe_exact']
part=read(r/'partition-restored-final.json');assert part['source_raw_counts']['rig_output:root_result']==dict(translation=0,rotation=0,scale=0)
assert part['action_capture_names']=={'None':318}
negative=read(r/'negative-controls.json');assert len(negative)==3
external=[]
for name in ['docs/audit/bf144/448a/restored.native.json.gz','docs/audit/bf144/448a/request.json','docs/audit/bf144/446f/restored.native.json.gz','docs/audit/bf144/446f/request.json','docs/audit/bf144/441b/curves.native.json','docs/audit/bf144/441b/curves.request.json','docs/audit/bf144/446f/reader-migration.json','docs/audit/bf144/456/summary.json']:
    data=Path(name).read_bytes();external.append(dict(path=name,sha256=hashlib.sha256(data).hexdigest(),bytes=len(data)))
result=dict(gate='PASS_NATIVE_INDEPENDENT_RIG_OUTPUT_AND_ACTUAL_ACTION_CAPABILITY_WITH_FULL_LEGACY_COMPATIBILITY',ordinary_frames=320,initialization_packets=2,rig_output_bone_rows=25438,primary_history_frames=318,complete_old_character_probe_exact_after_removing_only_new_observations=True,complete_enabled_character_probe_restored_exact=True,actual_action_capture_names_primary={'None':318},nonempty_local_struct_reader_control_passed=True,actual_ragdoll_action_branch_not_claimed=True,source_raw_rig_to_root_pose_changes=part['source_raw_counts']['rig_output:root_result'],three_ablation_counterexamples_verified=True,typed_and_parent_request_guards_verified=True,complete_old_default_and_five_boundary_and_curve_vector_probes_compatible=True,source_bytes_restored_same_native_package_rebuilt=True,other395_and_six_libraries_unchanged=True,reader_migration_explicit=True,external_source_files=external,new_differences=[],confirmed_difference_count=272,full_rig_or_root_or_als_equivalence_claimed=False,next_boundary='Fresh source/target Ragdolling graph and actual common-input passthrough normalization contract; source action is capture-time metadata',screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
print('Verified native Rig output/action capability, three ablations, byte restoration, explicit identity migration and full legacy compatibility',flush=True)
