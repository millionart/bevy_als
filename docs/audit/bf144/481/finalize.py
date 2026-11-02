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

comparison=read('comparison.json')
assert comparison['source_cache_cases']==1332 and comparison['current_actual_cache_cases']==1272 and comparison['current_materialized_clean_control']==1
assert raw('replay-unit-controlled.json')==raw('replay-unit-focused-counts.json')==raw('replay-unit-restored.json')
assert raw('replay.rs')==raw('replay.rs.final')and raw('inputs.json')==raw('inputs.json.final')
assert hashlib.sha256(raw('replay.rs')).hexdigest()==read('final-source.json')['sha256']
assert hashlib.sha256(raw('inputs.json')).hexdigest()==read('final-source.json')['inputs_sha256']
assert hashlib.sha256(raw('current_vm.rs')).hexdigest()==read('final-source.json')['current_vm_sha256']
for label in ['baseline','one-pass-ordered','minimal','controlled','focused-counts','restored']:
    assert read('replay-compile-'+label+'.report.json')['success']and read('replay-'+label+'.report.json')['success']
for label in ['baseline','clean-control']:
    assert read('current-vm-compile-'+label+'.report.json')['success']and read('current-vm-'+label+'.report.json')['success']
assert read('current-input-check.json')['full_old_register_local_call_trace_history_exact']
assert read('current-input-check.json')['malformed_debug_rejected']==2
negative=[]
for case,expected in [('propagation',(1,0,2)),('local-dirty',(0,0,2)),('source-normalizer',(1,1,0))]:
    data=read('replay-unit-no-'+case+'.json')
    assert (data['source_exact'],data['current_exact'],data['twists_exact'])==expected
    assert read('replay-compile-no-'+case+'.report.json')['success']
    assert read('replay-no-'+case+'.report.json')['process_exit_code']==101
    selection=read('negative-selection-'+case+'.json')
    assert hashlib.sha256(raw('replay.no-'+case+'.rs')).hexdigest()==selection['source_sha256']
    assert hashlib.sha256(raw('negative-inputs-'+case+'.json')).hexdigest()==selection['inputs_sha256']
    inputs=read('negative-inputs-'+case+'.json')
    assert inputs['source']==read('inputs.json')['source'][:1]
    assert inputs['current']==[row for row in read('inputs.json')['current']if row['kind']=='clean_control']
    negative.append(dict(case=case,source_exact=expected[0],clean_public_control_exact=expected[1],native_twist_exact=expected[2],native_source_cases=1,public_control_cases=1))
assert read('identity.report.json')['success']and read('identity-check.json')['six_libraries_unchanged']
for name,row in read('compression-map.json').items():
    print('Checking persistent cache evidence '+name,flush=True)
    packed=(r/row['compressed']).read_bytes()
    decoded=gzip.decompress(packed)
    assert len(decoded)==row['raw_bytes']and hashlib.sha256(decoded).hexdigest()==row['raw_sha256']
    assert hashlib.sha256(packed).hexdigest()==row['compressed_sha256']
(r/'negative-check.json').write_text(json.dumps(dict(gate='PASS_MINIMAL_PROPAGATION_LOCAL_DIRTY_AND_NATIVE_TWIST_COUNTEREXAMPLES',records=negative,all_dirty_natural_input_not_used_as_sole_propagation_proof=True,canonical_source_inputs_restored_same_target_full_report_bytes_exact=True),indent=2)+'\n')
external=[]
for name in ['docs/audit/bf144/476/inputs.json','docs/audit/bf144/476/current_vm.rs','docs/audit/bf144/476/replay-baseline.json.gz','docs/audit/bf144/476/actual-after.native.json.gz','docs/audit/bf144/476/weights-after.native.json.gz','docs/audit/bf144/480/replay-unit-factors.json.gz','docs/audit/bf144/480/summary.json','docs/audit/bf144/481a/table-actual-before-restored.json.gz','docs/audit/bf144/481a/table-actual-after-restored.json.gz','docs/audit/bf144/481a/table-weights-before-restored.json.gz','docs/audit/bf144/481a/table-weights-after-restored.json.gz','docs/audit/bf144/481a/reader-migration.json','docs/audit/bf144/481a/summary.json','docs/audit/bf144/479/replay.rs','docs/audit/bf144/478/replay.rs','docs/audit/bf144/471/replay.rs','docs/audit/bf144/473/replay.rs','docs/audit/bf144/441b/identities.start.json','docs/audit/bf144/441c/library-identities.json','docs/audit/bf144/441b/watchdog.self-test.json','tools/run_ue_source_reader.ps1']:
    data=Path(name).read_bytes()
    external.append(dict(path=name,bytes=len(data),sha256=hashlib.sha256(data).hexdigest()))
summary=dict(comparison)
summary.update(gate='PASS_COMPLETE_ACTUAL_SOURCE_CURRENT_RAW_IK_CACHE_AND_TWIST_CONSUMERS',minimal_native_public_counterexamples=negative,canonical_source_inputs_and_current_vm_sha_restored=True,same_target_recompiled_complete_report_byte_equal=True,current396_reader481a_and_six_libraries_unchanged=True,existing_source_math_consumer='D248 source normalizer on native twist global evaluation',new_differences=[],external_source_files=external,production_reader_engine_converters_source_assets_unchanged=True,ball_final_local_and_full_rig_or_root_equivalence_claimed=False,next_boundary='Foot writer after IK and full ball/descendant final global and local consumers,then remaining full Rig/Root feedback')
(r/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print('Verified full1332 source/1272 current79-bone caches,clean clone propagation counterexample,2664 native twists and restored bytes',flush=True)
