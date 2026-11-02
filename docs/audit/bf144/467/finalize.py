import gzip
import hashlib
import json
import struct
from pathlib import Path

r=Path(__file__).resolve().parent
def raw(name):
 p=r/name;return p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix(p.suffix+'.gz').read_bytes())
def read(name):return json.loads(raw(name))
reader=Path('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp');assert reader.read_bytes()==raw('reader.final.cpp');assert hashlib.sha256(reader.read_bytes()).hexdigest()==read('reader-final-sha256.json')['sha256']
assert read('restored-package-source-check.json')['restoration_process_finished_before_build_started']and read('restored-package-source-check.json')['full_build_terminal_verified']
for label in['build-baseline','build-zero-global','build-restored-prototype','build-no-type-guard','build-restored']:assert read(label+'.report.json')['success']
for label in['before','zero-global','premature-f32']:assert read('check-'+label+'.report.json')['process_exit_code']==1
for label in['baseline','restored-prototype','restored']:
 x=read('check-'+label+'.json');assert x['frames']==638 and x['compact_local_bones_exact']==50402 and x['thigh_translation_call_fields_qualified']==1272 and x['whole_old_probe_exact_after_removing_snapshot_and_instance_hash']
assert read('check-restored-prototype.json')['complete_global_snapshot_exact_prototype']and read('check-restored.json')['complete_global_snapshot_exact_prototype']and read('check-restored.json')['redundant_current_local_removed_without_losing_compact_local_data']
assert raw('table-baseline.json')==raw('table-restored-prototype.json')==raw('table-restored.json')
assert read('compatibility.json')['whole_old_default_character_probe_exact']and read('compatibility.json')['whole_old_three_axis_curve_vector_probe_exact']
assert read('identity-check.json')['six_libraries_unchanged']
prototype=raw('reader.with-local.cpp').decode();zero=prototype.replace('Bone->SetObjectField(TEXT("current_global_transform"), CaptureTransform(Hierarchy->GetGlobalTransform(Key)));','Bone->SetObjectField(TEXT("current_global_transform"), CaptureTransform(FTransform::Identity));');assert hashlib.sha256(zero.encode()).hexdigest()==read('package-zero-global.json')['sha256']
for row in read('compression-map.json').values():
 packed=raw(row['compressed']);decoded=gzip.decompress(packed);assert len(decoded)==row['raw_bytes']and hashlib.sha256(decoded).hexdigest()==row['raw_sha256'];assert hashlib.sha256(packed).hexdigest()==row['compressed_sha256']
baseline=read('baseline.native.json')['rig_node_history_probe']['cases'][0]['frames'][0];pelvis=next(x for x in baseline['after']['bones']if x['name']=='pelvis')['local_transform']['translation']['x'];quantized=struct.unpack('f',struct.pack('f',pelvis))[0];assert pelvis!=quantized
external=[]
for name in['docs/audit/bf144/465a/restored.native.json.gz','docs/audit/bf144/465a/request.json','docs/audit/bf144/465a/reader-migration.json','docs/audit/bf144/466/rig.native.json.gz','docs/audit/bf144/462/restored.native.json.gz','docs/audit/bf144/462/request.json','docs/audit/bf144/457/default.native.json.gz','docs/audit/bf144/457/default.request.json','docs/audit/bf144/441b/curves.native.json','docs/audit/bf144/441b/curves.request.json','docs/audit/bf144/462/cache_identity.py']:
 data=Path(name).read_bytes();external.append(dict(path=name,bytes=len(data),sha256=hashlib.sha256(data).hexdigest()))
summary=dict(gate='PASS_NATIVE_CURRENT_GLOBAL_HIERARCHY_CAPTURE_FIELD_LIFETIME_AND_FULL_LEGACY_COMPATIBILITY',strict_opt_in='capture_current_hierarchy',source_frames=638,current_global_bone_transforms=50402,existing_compact_local_double_bones_exact=50402,thigh_translation_call_fields_qualified=1272,whole_thigh_transform_at_reader_call_not_claimed=True,source_pelvis_height_within_native_hierarchy_write_skip_interval=True,zero_global_getter_ablation_rejected_at_query=1,prototype_getter_source_bytes_restored_complete_native_table_exact=True,duplicate_current_local_field_removed=True,complete_current_global_snapshots_exact_with_prototype=True,existing_native_compact_is_double=True,premature_f32_counterexample=dict(query=0,bone='pelvis',translation_x=pelvis,incorrect_f32_value=quantized),type_guard_removal_accepts_invalid_string_request=True,final_type_guard_rejects_invalid_string_request=True,complete_old_rig_and_character_and_curvevector_probes_exact=True,full_old_opt_in_spring_history_unchanged=True,source_current_global_getter_added_without_source_memory_or_algorithm_writes=True,final_reader_bytes_restored_build_terminal_and_package_exact=True,reader_migration='docs/audit/bf144/467/reader-migration.json',other395_and_six_libraries_unchanged=True,external_source_files=external,new_differences=[],confirmed_difference_count=272,production_engine_converters_source_assets_unchanged=True,full_foot_location_or_rig_or_root_equivalence_claimed=False,next_boundary='Use qualified actual source thigh translation and existing actual arguments to audit final foot leg-vector clamp and full FootLocation',screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');print('Verified native exact current global hierarchy, field lifetime, minimal getter, full old compatibility and byte restoration',flush=True)
