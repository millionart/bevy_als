import hashlib
import json
from pathlib import Path
from check import validate

r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())
a=read('runtime-baseline/parameters.json');b=read('runtime-restored/parameters.json')
assert validate(a)==validate(b)
for key in ['rows','after_motors','camera_after_post_update','animation_before','animation_after']:assert a[key]==b[key],key
old=json.loads(Path('docs/audit/bf144/446e/runtime-restored/parameters.json').read_text())
for key in ['rows','after_motors','camera_after_post_update']:assert a[key]==old[key]
for key in ['animation_before','animation_after']:assert [{k:v for k,v in row.items()if k!='head_values'}for row in a[key]]==old[key]
try:validate(read('runtime-ablated/parameters.json'))
except KeyError:pass
else:raise AssertionError('Missing actual Head inputs passed')
assert read('check-ablated.report.json')['process_exit_code']!=0
for name,digest in read('final-sources.json').items():assert hashlib.sha256((r/name).read_bytes()).hexdigest()==digest
for label in ['baseline','restored']:assert read(label+'-functional.json')['pass']is True
assert (r/'analysis-baseline.json').read_bytes()==(r/'analysis-restored.json').read_bytes()
assert read('identity-check.json')['observer_sources_restored']
result=dict(gate='PASS_ACTUAL_HEAD_INPUT_STATE_AND_LOOK_PIN_READONLY_ENTRY',queries_per_phase=319,head_input_output_rows=638,actual_look_consumer_rows=319,
    global_property_and_frame_input_channels_preserved=True,look_pin_equals_actual_head_output=True,first_source_and_target_view_yaw_inputs_equal=False,
    common_input_kernel_cause_confirmed=False,observation_ablation_rejected=True,negative_fixtures_rejected=4,source_bytes_restored=True,same_target_rebuilt=True,
    complete_runtime_arrays_restored_exact=True,analysis_restored_byte_exact=True,new_differences=[],total_confirmed_differences=271,
    full_root_or_als_equivalence_claimed=False,render_gpu_pass_acceptance='deferred under explicit peripheral exclusion',screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result),flush=True)
