import hashlib
import json
from pathlib import Path
from check import validate

r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())
a=read('runtime-baseline-active/parameters.json');b=read('runtime-restored/parameters.json')
assert validate(a)==validate(b)
for key in ['rows','after_motors','camera_after_post_update','animation_before','animation_after','physics_motors','physics_coefficients','physics_raw']:assert a[key]==b[key],key
old=json.loads(Path('docs/audit/bf144/446e/runtime-restored/parameters.json').read_text())
for key in ['rows','after_motors','camera_after_post_update','animation_before','animation_after']:assert a[key]==old[key]
try:validate(read('runtime-ablated/parameters.json'))
except KeyError:pass
else:raise AssertionError('Missing physical phase observations passed')
assert read('check-ablated.report.json')['process_exit_code']!=0
assert read('replay-ablated.report.json')['process_exit_code']==101
for name,digest in read('final-sources.json').items():assert hashlib.sha256((r/name).read_bytes()).hexdigest()==digest
assert (r/'replay-baseline-active.json').read_bytes()==(r/'replay-restored.json').read_bytes()
assert (r/'analysis-baseline-active.json').read_bytes()==(r/'analysis-restored.json').read_bytes()
for label in ['baseline-active','restored']:assert read(label+'-functional.json')['pass']is True
assert read('identity-check.json')['observer_sources_restored']
model=read('replay-restored.json');consumer=read('analysis-restored.json')
assert model['force_rows_exact']==model['raw_physics_rows_exact']==319
assert model['cold_spring_rows_exact']==79 and model['first']['query']==19
result=dict(gate='PASS_COLD319_ACTUAL_FORCE_PHYSICS_AND_D31_FIRST_CONSUMER',force_rows_exact=319,raw_physics_rows_exact=319,cold_spring_rows_exact=79,
    cold_raw_and_final_position_rows_equal=79,ready_post_floor_rows_reused_from_current443b=240,
    sleeping_without_solver_rows_qualified=model['sleeping_without_solver_rows_qualified'],sense_only_rows_qualified=model['sense_only_animation_rows'],
    first_position_query=19,first_position_application_frame=24,first_source_floor_distance_cm=consumer['source_floor_distance_cm'],known_difference='BF144-D31',
    observation_and_spring_only_factor_ablations_rejected=True,source_bytes_restored=True,same_target_rebuilt=True,
    complete_runtime_arrays_restored_exact=True,replay_and_analysis_restored_byte_exact=True,
    all_source_target_trajectory_or_root_equivalence_claimed=False,new_differences=[],total_confirmed_differences=271,
    render_gpu_pass_acceptance='deferred under explicit peripheral exclusion',screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result),flush=True)
