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
assert comparison['source_complete_tqs_exact']==1332 and comparison['public_total_complete_models_exact']==1302
assert raw('replay-unit-factors.json')==raw('replay-unit-focused-counts.json')==raw('replay-unit-restored.json')
assert raw('replay.rs')==raw('replay.rs.final')and raw('inputs.json')==raw('inputs.json.final')
assert hashlib.sha256(raw('replay.rs')).hexdigest()==read('final-source.json')['sha256']
assert hashlib.sha256(raw('inputs.json')).hexdigest()==read('final-source.json')['inputs_sha256']
for label in ['baseline','factors','focused-counts','no-core-hold','restored']:
    assert read('replay-compile-'+label+'.report.json')['success']and read('replay-'+label+'.report.json')['success']
assert read('isolation-factors.json')['changed_solver_records']==0
assert read('isolation-no-core-hold.json')['changed_solver_records']==126
assert read('core_isolation_gate-no-core-hold.report.json')['process_exit_code']==1
assert read('core_isolation_gate-factors.report.json')['success']
negative=[]
full=read('inputs.json')
for case in ['slerp','normalizer','vector','position-rebuild']:
    assert read('replay-compile-no-'+case+'.report.json')['success']
    assert read('replay-no-'+case+'.report.json')['process_exit_code']==101
    result=read('replay-unit-no-'+case+'.json')
    inputs=read('negative-inputs-'+case+'.json')
    selection=read('negative-selection-'+case+'.json')
    assert len(inputs['source'])==len(inputs['current'])==2
    assert result['source_exact']<2 and result['core_exact']==result['public_exact']==2 and result['actual_exact']==0
    assert all(row in full['source']for row in inputs['source'])
    assert all(row in full['current']for row in inputs['current'])
    assert hashlib.sha256(raw('replay.no-'+case+'.rs')).hexdigest()==selection['source_sha256']
    assert hashlib.sha256(raw('negative-inputs-'+case+'.json')).hexdigest()==selection['inputs_sha256']
    if case=='position-rebuild':
        assert all(a['rotation']==b['rotation']for row in result['source']for a,b in zip(row['result']['after'],row['native']))
        assert selection['public_control_zero_only']
    negative.append(dict(case=case,source_exact=result['source_exact'],native_mismatched_cases=2-result['source_exact'],pre_weight_core_exact=2,public_control_exact=2,public_control_zero_only=case=='position-rebuild'))
assert read('identity-check.json')['six_libraries_unchanged']and read('identity.report.json')['success']
assert read('core.native.json.reader.report.json')['success']
assert read('native-check.json')['native_calls']==1330
assert hashlib.sha256(raw('core.request.json')).hexdigest()==read('request-identity-before-metadata-fix.json')['sha256']
for row in read('compression-map.json').values():
    packed=(r/row['compressed']).read_bytes()
    decoded=gzip.decompress(packed)
    assert len(decoded)==row['raw_bytes']and hashlib.sha256(decoded).hexdigest()==row['raw_sha256']
    assert hashlib.sha256(packed).hexdigest()==row['compressed_sha256']
(r/'negative-check.json').write_text(json.dumps(dict(gate='PASS_PARTIAL_WRAPPER_MINIMAL_NATIVE_COUNTEREXAMPLES_AND_CORE_ISOLATION',records=negative,source_solver_hold_removal_changed_records=126,canonical_source_inputs_and_report_restored_byte_exact=True),indent=2)+'\n')
external=[]
for name in ['docs/audit/bf144/476/inputs.json','docs/audit/bf144/476/summary.json','docs/audit/bf144/477/inputs.json','docs/audit/bf144/479/inputs.json','docs/audit/bf144/479/replay.rs','docs/audit/bf144/479/summary.json','docs/audit/bf144/478/replay.rs','docs/audit/bf144/473/replay.rs','docs/audit/bf144/471/replay.rs','docs/audit/bf144/474a/reader-migration.json','docs/audit/bf144/441b/identities.start.json','docs/audit/bf144/441c/library-identities.json','docs/audit/bf144/441b/watchdog.self-test.json','tools/animgraph_authoring/ue_control_rig_contract.py','tools/animgraph_authoring/ue_control_rig_target.py','tools/run_ue_source_reader.ps1']:
    data=Path(name).read_bytes()
    external.append(dict(path=name,bytes=len(data),sha256=hashlib.sha256(data).hexdigest()))
summary=dict(comparison)
summary.update(gate='PASS_COMPLETE_IK_ZERO_PARTIAL_FULL_WRAPPER_AND_THREE_BONE_GLOBAL_CONSUMERS',minimal_native_slerp_normalizer_vector_and_position_rebuild_negatives=True,source_solver_input_hold_negative_records=126,canonical_source_full_inputs_and_same_target_recompiled_reports_byte_equal=True,current396_reader474a_and_six_libraries_unchanged=True,existing_differences=['D247','D248','D250'],new_differences=[],external_source_files=external,production_reader_engine_converters_source_assets_unchanged=True,full_rig_or_root_equivalence_claimed=False,next_boundary='Exact descendant propagation and local/global dirty/Equals execution on all source/current IK writers')
(r/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print('Verified complete1332 native weight wrappers,1302 public consumers,four minimal negatives and restored source/input bytes',flush=True)
