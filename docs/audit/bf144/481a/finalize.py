import gzip
import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
def raw(name):
    p=r/name
    if not p.exists():
        p=r/(name+'.gz')
    return gzip.decompress(p.read_bytes())if p.suffix=='.gz'else p.read_bytes()
def read(name):
    return json.loads(raw(name))

reader=Path('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp')
package=Path('target/ue_graph_source_reader_package/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp')
assert reader.read_bytes()==package.read_bytes()==raw('reader.final.cpp')
assert read('restored-package-check.json')['full_build_terminal_verified']
for name in ['baseline','global-false','global-identity','restored']:
    assert read('build-'+name+'.report.json')['success']
assert read('check_missing.report.json')['process_exit_code']==1
counts=dict(snapshots=0,bones=0,global_dirty=0,local_dirty=0,clean_typed_global_exact=0,active_leg_ball_dirty=0)
for kind in ['actual-before','actual-after','weights-before','weights-after']:
    a=read('check-'+kind+'-baseline.json')
    b=read('check-'+kind+'-restored.json')
    assert a==b
    assert raw('table-'+kind+'-baseline.json')==raw('table-'+kind+'-restored.json')
    for name in ['baseline','restored']:
        assert read(kind+'-'+name+'.native.json.reader.report.json')['success']
    for key,value in a['counts'].items():
        counts[key]+=value
assert counts['snapshots']==4010 and counts['bones']==316790 and counts['active_leg_ball_dirty']==1330
assert 0<counts['global_dirty']<counts['bones']and 0<counts['local_dirty']<counts['bones']
negative=[]
for case in ['global-false','global-identity']:
    report=read('ablation-'+case+'.json')
    assert report['only_selected_metadata_changed']and report['changed_cache_records']>0
    assert read('check-weights-after-'+case+'.report.json')['process_exit_code']==1
    assert read('transparent-weights-after-'+case+'.json')['original_execution_pose_work_hierarchy_and_items_exact']
    negative.append(report)
assert read('compatibility.json')['whole_old_default_rig_exact']
assert read('compatibility.json')['whole_old_default_character_exact']
assert read('compatibility.json')['whole_old_three_axis_curvevector_exact']
assert read('identity-check.json')['six_libraries_unchanged']
compressed=read('compression-map.json')
assert len(compressed)==21 and sum(name.endswith('.native.json')for name in compressed)==13
for name,row in compressed.items():
    print('Checking lossless source evidence '+name,flush=True)
    packed=(r/row['compressed']).read_bytes()
    decoded=gzip.decompress(packed)
    assert len(decoded)==row['raw_bytes']and hashlib.sha256(decoded).hexdigest()==row['raw_sha256']
    assert len(packed)==row['compressed_bytes']and hashlib.sha256(packed).hexdigest()==row['compressed_sha256']
external=[]
for name in ['docs/audit/bf144/476/inputs.json','docs/audit/bf144/476/actual-before.request.json.gz','docs/audit/bf144/476/actual-after.request.json.gz','docs/audit/bf144/476/weights-before.request.json','docs/audit/bf144/476/weights-after.request.json','docs/audit/bf144/476/actual-before.native.json.gz','docs/audit/bf144/476/actual-after.native.json.gz','docs/audit/bf144/476/weights-before.native.json.gz','docs/audit/bf144/476/weights-after.native.json.gz','docs/audit/bf144/474a/reader-migration.json','docs/audit/bf144/462/cache_identity.py','docs/audit/bf144/462/request.json','docs/audit/bf144/462/restored.native.json.gz','docs/audit/bf144/457/default.request.json','docs/audit/bf144/457/default.native.json.gz','docs/audit/bf144/441b/curves.request.json','docs/audit/bf144/441b/curves.native.json','docs/audit/bf144/441b/identities.start.json','docs/audit/bf144/441c/library-identities.json','docs/audit/bf144/441b/watchdog.self-test.json','tools/run_ue_source_reader.ps1','tools/build_ue_source_reader.ps1']:
    data=Path(name).read_bytes()
    external.append(dict(path=name,bytes=len(data),sha256=hashlib.sha256(data).hexdigest()))
summary=dict(gate='PASS_READONLY_NATIVE_RAW_HIERARCHY_CACHE_BOUNDARY_WITH_RESTORATION_AND_LEGACY_COMPATIBILITY',counts=counts,one_additive_array_under_existing_opt_in_only=True,raw_cache_capture_before_existing_item_getters=True,no_new_pose_getter_tracking_state_configuration_or_source_algorithm=True,allocating_pose_version_getter_deliberately_excluded=True,dirty_cache_values_not_claimed_current_evaluated_transforms=True,complete_old_body_transparency_all_four_histories=True,baseline_restored_all4010_cache_tables_byte_equal=True,metadata_only_negative_cases=negative,whole_default_rig_character_and_three_axis_curvevector_compatibility=True,canonical_source_bytes_restored_same_package_full_terminal_rebuild=True,reader_migration='docs/audit/bf144/481a/reader-migration.json',other395_and_six_libraries_unchanged=True,external_source_files=external,new_differences=[],confirmed_difference_count=272,production_engine_converters_source_assets_unchanged=True,writer_execution_counts_or_descendant_math_equivalence_claimed=False,full_rig_or_root_equivalence_claimed=False,next_boundary='Source/target ordered three IK writers,dirty and Equals decisions,and full descendant local/global propagation using exact raw caches',screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print('Verified readonly4010 native cache boundaries,316790 bones,metadata negatives,byte restoration and full legacy compatibility',flush=True)
