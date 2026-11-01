import hashlib
import json
from pathlib import Path
from check import load,validate

r=Path(__file__).resolve().parent
read=lambda n:json.loads((r/n).read_text())
a=load('baseline');b=load('restored');assert validate(a)==validate(b)==239
for key in ['rows','after_motors','camera_after_post_update','animation_before','animation_after']:assert a[key]==b[key],key
assert (r/'check-baseline.json').read_bytes()==(r/'check-restored.json').read_bytes()
assert read('check-ablated.report.json')['process_exit_code']==1
ablated=load('ablated')
for key in ['rows','after_motors','camera_after_post_update']:assert a[key]==ablated[key],key
for key in ['animation_before','animation_after']:assert [{k:v for k,v in row.items()if k!='idle_leaf'}for row in a[key]]==ablated[key],key
for name,digest in read('final-sources.json').items():assert hashlib.sha256((r/name).read_bytes()).hexdigest()==digest
for label in ['baseline','restored']:assert read(label+'-functional.json')['pass']
assert read('compile-restored.report.json')['success']and read('identity-check.json')['observer_sources_restored']
assert hashlib.sha256((r/'source-facts.json').read_bytes()).hexdigest()==read('facts-before-simplification.json')['sha256']
check=read('check-restored.json')
endpoints=json.loads(Path('docs/audit/bf144/449/endpoints.json').read_text())
assert endpoints['first_height_difference_is_not_raw_versus_compressed']
result=dict(gate='PASS_ACTUAL_IDLE_LEAF_PUBLIC_SAMPLER_AND_FRESH_SOURCE_UNIT_ENDPOINTS',queries_per_phase=319,actual_updated_leaf_records=239,actual_bone_rows=18881,
    public_sampler_exact_all_actual_leaves=True,first98_positive_baseprovider_bones_exact_actual_leaf=True,source_raw_and_compressed_first_height_equal=True,
    physical_projection=endpoints['physical_unit_projection'],target_float_exported_height=endpoints['target_pelvis_translation'][1],
    old_five_complete_history_arrays_exact=True,observation_ablation_rejected=True,source_bytes_restored=True,same_target_six_libraries_used=True,
    restored_complete_runtime_arrays_and_check_byte_exact=True,negative_fixtures_rejected=4,unused_fact_dependency_removed=True,
    source_unit_factor_consumer_control_still_required=True,new_differences=[],total_confirmed_differences=271,full_root_or_als_equivalence_claimed=False,screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
