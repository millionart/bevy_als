import gzip
import json
import sys
from pathlib import Path

r=Path(__file__).resolve().parent
label=sys.argv[1]
data=json.loads((r/f'runtime-{label}/parameters.json').read_text())
source=json.loads(gzip.decompress(Path('docs/audit/bf144/446d/restored.native.json.gz').read_bytes()))['character_animation_probe']['cases'][0]
before=data['animation_before'];after=data['animation_after']
init=next(v for v in source['initialization']['instances']if v['main'])
source_before=next(v for v in source['frames'][0]['instances_before_tick']if v['main'])
source_after=next(v for v in source['frames'][0]['linked_instances']if v['main'])
x=before[0]['curve_frame_input'];y=after[0]['curve_frame_input'];z=before[1]['curve_frame_input']
assert x['global_standing']==0 and y['projected_standing']==1
assert z['global_standing']==z['projected_standing']==1
assert z['source_property_standing']is None
assert source_before['proxy_attribute_curves']['PoseStanding']==1
assert init['proxy_observed_engine_frame']==source_before['proxy_observed_engine_frame']==source_before['proxy_native_update_engine_frame']
assert z['app_frame']==x['app_frame']+1
pose=next(v for v in source['frames'][0]['source_state_after_tick']if v['name']=='PoseState')
source_standing=next(v['numeric_value']for v in pose['fields']if v['name']=='StandingAmount')
assert source_standing==0
result=dict(gate='PASS_ACTUAL_PRIOR_CURVE_VALUES_WITH_DISTINCT_NATIVE_CALLBACK_FRAME_BOUNDARY',
    target_initialization_app_frame=x['app_frame'],target_first_ordinary_app_frame=z['app_frame'],target_first_ordinary_global_standing=z['global_standing'],
    target_actual_source_property_override=z['source_property_standing'],target_after_zero_projected_standing=y['projected_standing'],
    source_initialization_engine_frame=init['proxy_observed_engine_frame'],source_first_ordinary_engine_frame=source_before['proxy_observed_engine_frame'],source_last_native_update_engine_frame=source_before['proxy_native_update_engine_frame'],
    source_actual_prior_standing=source_before['proxy_attribute_curves']['PoseStanding'],source_pose_state_standing_after_first_ordinary=source_standing,
    prior_curve_values_different=False,source_native_callback_skipped_same_frame=True,target_first_ordinary_is_new_application_frame=True,
    matching_engine_frame_source_control_completed=False,curve_publication_candidate_confirmed_as_port_difference=False,
    actual_global_time_at_target_zero_query_ns=x['virtual_elapsed_ns'],full_history_or_root_equivalence_claimed=False,new_differences=[],total_confirmed_differences=271)
(r/f'analysis-{label}.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result),flush=True)
