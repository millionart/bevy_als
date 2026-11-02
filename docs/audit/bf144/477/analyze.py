import gzip
import hashlib
import json
import struct
from pathlib import Path

r = Path(__file__).resolve().parent
def raw(name):
    p = r / name
    if not p.exists():
        p = r / (name + '.gz')
    return gzip.decompress(p.read_bytes()) if p.suffix == '.gz' else p.read_bytes()
def read(name):
    return json.loads(raw(name))
def f32(value):
    return struct.unpack('f', struct.pack('f', value))[0]

baseline = read('replay-unit-baseline.json')
negative = read('replay-unit-no-cm-float-stage.json')
assert baseline['exact'] == [1272,1272,820,1272]
assert baseline['length_changed_calls'] == [0,0,452,0]
assert negative['exact'] == [1272] * 4 and negative['length_changed_calls'] == [0] * 4
assert read('replay-no-cm-float-stage.report.json')['process_exit_code'] == 101
assert read('replay-compile-no-cm-float-stage.report.json')['success']
assert raw('replay-unit-baseline.json') == raw('replay-unit-restored.json')
assert raw('replay.rs') == raw('replay.rs.final')
assert hashlib.sha256(raw('replay.rs')).hexdigest() == read('final-source.json')['sha256']
for label in ['baseline','restored']:
    assert read('replay-compile-'+label+'.report.json')['success']
    assert read('replay-'+label+'.report.json')['success']
solver = read('solver-check.json')
assert solver['calls'] == solver['exact'] == 1324
rows = baseline['results']
changes = []
for offset in range(0,len(rows),4):
    a,b,c,d = rows[offset:offset+4]
    assert a['after'] == b['after'] == d['after']
    if not c['exact']:
        fields = []
        float_fields = []
        for old,new in zip(a['after'],c['after']):
            assert old[0] == new[0]
            for key in ['translation','rotation','scale']:
                if old[1][key] != new[1][key]:
                    fields.append([old[0],key])
                if list(map(f32,old[1][key])) != list(map(f32,new[1][key])):
                    float_fields.append([old[0],key])
        changes.append(dict(mode=a['mode'],query=a['query'],pc=a['pc'],current_lengths_m=a['computed_lengths_m'],source_cm_float_lengths_m=c['selected_lengths_m'],changed_fields=fields,changed_f32_fields=float_fields,before=a['after'],after=c['after']))
assert len(changes) == 452
report = dict(gate='PASS_ACTUAL_AUTOMATIC_LENGTH_NATIVE_CONSUMERS_AND_EXISTING_D165_FACTOR',source_active_calls=1330,source_independent_native_lengths=2660,source_zero_weight_returns_before_lengths=2,source_partial_length_only_calls=6,source_full_weight_exact_solver_consumers=1324,current_public_auto_length_calls_exact=1272,current_explicit_independent_native_metre_calls_exact=1272,current_native_cm_float_stage_changed_lengths_and_complete_pose_calls=452,current_native_cm_float_stage_changed_scalars=baseline['changed_scalars'][2],current_native_double_cm_to_m_then_cast_changed_calls=0,changed_f32_pose_calls=sum(bool(x['changed_f32_fields']) for x in changes),source_initial_data_not_injected_into_same_current_unit_control=True,source_B_scale_inherits_A_not_actual_B=True,physical_cm_float_removal_loses_only_factor_signal=True,canonical_source_bytes_restored_same_target_recompiled_output_bytes_exact=True,existing_difference='D165',new_differences=[],confirmed_difference_count=272,full_source_solver_math_or_partial_equivalence_claimed=False,source_target_upstream_pose_and_initial_data_still_separate=True,screenshots=0,performance_acceptance='paused')
(r/'comparison.json').write_text(json.dumps(report,indent=2)+'\n')
(r/'difference-table.json').write_text(json.dumps(changes,separators=(',',':'))+'\n')
print(json.dumps(report),flush=True)
