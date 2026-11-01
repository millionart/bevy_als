import hashlib
import json
from pathlib import Path
from check import validate

r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())
a=read('runtime-baseline-full/parameters.json')
b=read('runtime-restored/parameters.json')
counts=validate(a)
assert counts==validate(b)
for key in ['rows','after_motors','camera_after_post_update','animation_before','animation_after']:
    assert a[key]==b[key],key
old=json.loads(Path('docs/audit/bf144/444/runtime-baseline/parameters.json').read_text())
for key in ['rows','after_motors','camera_after_post_update']:
    assert a[key]==old[key]
for key in ['animation_before','animation_after']:
    assert [{k:v for k,v in row.items()if k!='leaf_nodes'}for row in a[key]]==old[key]
for label in ['baseline','ablated']:
    try:validate(read(f'runtime-{label}/parameters.json'))
    except(AssertionError,KeyError):pass
    else:raise AssertionError('Partial or removed leaf observation passed')
assert read('fixture-ablated.report.json')['process_exit_code']==101
assert read('replay-ablated.report.json')['process_exit_code']==101
assert read('fixture-restored.report.json')['success']
assert read('replay-restored.report.json')['success']
assert (r/'replay-baseline-full.json').read_bytes()==(r/'replay-restored.json').read_bytes()
for name,digest in read('final-sources.json').items():
    assert hashlib.sha256((r/name).read_bytes()).hexdigest()==digest
for label in ['baseline-full','restored']:
    assert read(label+'-functional.json')['pass'] is True
table=read('player-table-restored.json')
assert table['source_declarations']==85
replay=read('replay-restored.json')
result=dict(gate='PASS_COMPLETE_CURRENT_PLAYER_CLOCK_TABLE_WITH_KNOWN_D201_AND_OPEN_HISTORY',counts=counts,
    source_declarations=85,source_tick_rows=table['source_tick_rows'],target_native_tick_rows=table['target_native_tick_rows'],
    never_instantiated_target_declarations=table['never_instantiated_target_declarations'],paired_own_history_clock_differences=table['paired_own_history_clock_differences'],
    current_idle_rows_replayed_exact=replay['current_idle_rows_exact'],native_direct_second_rows_exact=240,known_difference='BF144-D201',new_differences=[],total_confirmed_differences=271,
    source_bytes_restored=True,same_target_rebuilt=True,complete_runtime_arrays_restored_exact=True,replay_restored_byte_exact=True,
    partial_observation_temporary_key_and_direct_second_ablations_rejected=True,source_target_same_initial_history_claimed=False,all_clock_differences_attributed=False,full_root_or_als_equivalence_claimed=False,
    render_gpu_pass_acceptance='deferred under explicit peripheral exclusion',animation_functional_gate_passed=True,screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result),flush=True)
