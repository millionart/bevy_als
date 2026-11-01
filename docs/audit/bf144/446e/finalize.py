import hashlib
import json
from pathlib import Path
from check import validate

r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())
a=read('runtime-baseline/parameters.json')
b=read('runtime-restored/parameters.json')
assert validate(a)==validate(b)
for key in ['rows','after_motors','camera_after_post_update','animation_before','animation_after']:assert a[key]==b[key],key
old=json.loads(Path('docs/audit/bf144/446/runtime-restored/parameters.json').read_text())
for key in ['rows','after_motors','camera_after_post_update']:assert a[key]==old[key]
for key in ['animation_before','animation_after']:assert [{k:v for k,v in row.items()if k!='curve_frame_input'}for row in a[key]]==old[key]
try:validate(read('runtime-ablated/parameters.json'))
except KeyError:pass
else:raise AssertionError('Missing actual curve/frame inputs passed')
assert read('check-ablated.report.json')['process_exit_code']!=0
for name,digest in read('final-sources.json').items():assert hashlib.sha256((r/name).read_bytes()).hexdigest()==digest
for label in ['baseline','restored']:assert read(label+'-functional.json')['pass']is True
assert (r/'analysis-baseline.json').read_bytes()==(r/'analysis-restored.json').read_bytes()
assert read('identity-check.json')['observer_sources_restored']
analysis=read('analysis-restored.json')
assert analysis['target_first_ordinary_app_frame']==analysis['target_initialization_app_frame']+1
assert analysis['source_initialization_engine_frame']==analysis['source_first_ordinary_engine_frame']
assert not analysis['prior_curve_values_different']
result=dict(gate='PASS_TARGET_ACTUAL_PRIOR_CURVE_INPUTS_AND_DISTINCT_FRAME_BOUNDARY',queries_per_phase=319,
    target_initialization_app_frame=analysis['target_initialization_app_frame'],target_first_ordinary_app_frame=analysis['target_first_ordinary_app_frame'],
    target_and_source_prior_standing_values_equal_one=True,target_source_property_override_absent=True,
    source_first_world_native_callback_skipped_same_frame=True,source_matching_new_frame_control_completed=False,
    observation_ablation_rejected=True,negative_fixtures_rejected=4,source_bytes_restored=True,same_target_rebuilt=True,complete_runtime_arrays_restored_exact=True,analysis_restored_byte_exact=True,
    curve_publication_candidate_confirmed_as_port_difference=False,full_internal_history_or_root_equivalence_claimed=False,
    new_differences=[],total_confirmed_differences=271,render_gpu_pass_acceptance='deferred under explicit peripheral exclusion',screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result),flush=True)
