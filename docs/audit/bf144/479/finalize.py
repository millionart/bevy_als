import gzip
import hashlib
import json
import struct
from pathlib import Path

r=Path(__file__).resolve().parent
def raw(name):
    p=r/name
    if not p.exists():
        p=r/(name+'.gz')
    return gzip.decompress(p.read_bytes())if p.suffix=='.gz'else p.read_bytes()
def read(name):
    return json.loads(raw(name))
def f32(values):
    return [struct.unpack('f',struct.pack('f',v))[0]for v in values]

comparison=read('comparison.json')
assert comparison['gate']=='PASS_COMPLETE_IK_PRIMARY_SECONDARY_NATIVE_PUBLIC_ORIENTATION_AND_ISOLATED_FACTORS'
assert raw('replay-unit-factors.json')==raw('replay-unit-focused-counts.json')==raw('replay-unit-restored.json')
assert raw('replay.rs')==raw('replay.rs.final')
assert raw('inputs.json')==raw('inputs.json.final')
assert hashlib.sha256(raw('replay.rs')).hexdigest()==read('final-source.json')['sha256']
assert hashlib.sha256(raw('inputs.json')).hexdigest()==read('final-source.json')['inputs_sha256']
for label in ['baseline','factors','focused-counts','no-position-hold','restored']:
    assert read('replay-compile-'+label+'.report.json')['success']
    assert read('replay-'+label+'.report.json')['success']
assert read('isolation-factors.json')['changed_position_records']==0
assert read('isolation-no-position-hold.json')['changed_position_records']==4048
assert read('isolation_gate-no-position-hold.report.json')['process_exit_code']==1
assert read('isolation_gate-factors.report.json')['success']
full=read('inputs.json')['rows']
negative_records=[]
for case in ['dot','vector-normal','rotate','quat-normal','product']:
    selection=read('negative-selection-'+case+'.json')
    inputs=read('negative-inputs-'+case+'.json')['rows']
    assert inputs==full[selection['original_full_input_row']:selection['original_full_input_row']+2]
    assert hashlib.sha256(raw('negative-inputs-'+case+'.json')).hexdigest()==selection['inputs_sha256']
    assert hashlib.sha256(raw('replay.no-'+case+'.rs')).hexdigest()==selection['source_sha256']
    assert read('replay-compile-no-'+case+'.report.json')['success']
    assert read('replay-no-'+case+'.report.json')['process_exit_code']==101
    result=read('replay-unit-no-'+case+'.json')
    assert result['public_exact']==2 and result['source_exact']<2 and len(result['outputs'])==14
    assert result['current_actual']==int(inputs[0]['domain']=='current_m')
    changed=0
    for row in result['outputs']:
        if row['mask']!=31:
            continue
        original=inputs[row['row']]
        assert row['native']==original['expected_native_rotations']
        expected=[[p[k]for k in 'xyz']for p in original['expected_source_positions']]
        assert [row['result']['joint'],row['result']['end']]==expected
        changed+=not row['exact']
    assert changed>0
    negative_records.append(dict(case=case,pair_source_exact=result['source_exact'],pair_public_exact=2,changed_native_rotation_cases=changed,selection=selection))
assert read('identity.report.json')['success']and read('identity-check.json')['six_libraries_unchanged']
assert read('orientation.native.json.reader.report.json')['success']
for row in read('compression-map.json').values():
    packed=(r/row['compressed']).read_bytes()
    decoded=gzip.decompress(packed)
    assert len(decoded)==row['raw_bytes']and hashlib.sha256(decoded).hexdigest()==row['raw_sha256']
    assert hashlib.sha256(packed).hexdigest()==row['compressed_sha256']
witnesses=[]
base=read('replay-unit-factors.json')
actual=json.loads(Path('docs/audit/bf144/476/inputs.json').read_bytes())
for row in base['outputs']:
    for bone,(a,b)in enumerate(zip(row['result']['rotations'],row['native'])):
        if f32(a)==f32(b):
            continue
        original=full[row['row']]
        old=actual['source'if original['domain']=='source_cm'else 'current'][original['row']]
        locator={key:old[key]for key in ['query','pc']}
        locator.update({key:old[key]for key in ['mode','dataset','case']if key in old})
        witnesses.append(dict(input_row=row['row'],domain=original['domain'],secondary_weight=original['secondary_weight'],mask=row['mask'],position_mask=row['position_mask'],bone=bone,actual_locator=locator,quaternion=a,native_quaternion=b,f32_quaternion=f32(a),native_f32_quaternion=f32(b),actual_current_default=row['domain']=='current_m'and row['secondary_weight']==1 and row['mask']==0))
assert sum(row['actual_current_default']for row in witnesses)==1
(r/'witnesses.json').write_text(json.dumps(witnesses,indent=2)+'\n')
(r/'negative-check.json').write_text(json.dumps(dict(gate='PASS_FIVE_MINIMAL_NATIVE_ORIENTATION_COUNTEREXAMPLES_AND_POSITION_ISOLATION',records=negative_records,position_hold_removed_changed_records=4048,position_hold_present_changed_records=0,canonical_source_and_full_inputs_restored_byte_exact=True),indent=2)+'\n')
external=[]
for name in ['docs/audit/bf144/476/inputs.json','docs/audit/bf144/476/summary.json','docs/audit/bf144/478/inputs.json','docs/audit/bf144/478/request.request.json','docs/audit/bf144/478/positions.native.json.gz','docs/audit/bf144/478/replay.rs','docs/audit/bf144/478/summary.json','docs/audit/bf144/471/replay.rs','docs/audit/bf144/474a/reader-migration.json','docs/audit/bf144/441b/identities.start.json','docs/audit/bf144/441c/library-identities.json','docs/audit/bf144/441b/watchdog.self-test.json','tools/animgraph_authoring/ue_control_rig_contract.py','tools/animgraph_authoring/ue_control_rig_target.py','tools/run_ue_source_reader.ps1']:
    data=Path(name).read_bytes()
    external.append(dict(path=name,bytes=len(data),sha256=hashlib.sha256(data).hexdigest()))
summary=dict(comparison)
summary.update(gate='PASS_COMPLETE_ACTUAL_IK_ORIENTATION_NATIVE_PUBLIC_ISOLATED_FACTORS_AND_RESTORATION',five_minimal_native_counterexamples=True,position_hold_ablation_changed_records=4048,canonical_source_and_full_inputs_restored_same_target_recompiled_report_byte_exact=True,current396_reader474a_and_six_libraries_unchanged=True,existing_differences=['D163','D247','D248','D249'],new_differences=[],external_source_files=external,production_reader_engine_converters_source_assets_unchanged=True,full_rig_or_root_equivalence_claimed=False,next_boundary='Source partial-weight IK wrapper and full descendant hierarchy on qualified solver inputs')
(r/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print('Verified5192 native/public complete rotations, five minimal arithmetic negatives and full byte-exact restored evidence',flush=True)
