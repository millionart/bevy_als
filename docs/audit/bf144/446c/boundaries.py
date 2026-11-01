import gzip
import json
from pathlib import Path

r=Path(__file__).resolve().parent
source=json.loads(gzip.decompress(Path('docs/audit/bf144/446b/restored.native.json.gz').read_bytes()))['character_animation_probe']['cases'][0]
target=json.loads(Path('docs/audit/bf144/446/runtime-restored/parameters.json').read_bytes())
guid='b8f30b3f-4d30-eab1-ce6b-6bb9ec868234'
snapshots=[]
for index in range(2):
    f=source['frames'][index]
    pose=next(v for v in f['source_state_after_tick']if v['name']=='PoseState')
    standing=next(v['numeric_value']for v in pose['fields']if v['name']=='StandingAmount')
    player=next(p for i in f['linked_instances']for p in i['asset_players']if p['node_guid']==guid)
    node=next(n for n in target['animation_after'][index+1]['leaf_nodes']if n['node_guid']==guid)
    snapshots.append(dict(source_frame=index,target_query=index+1,source_pose_standing_amount=standing,source_public_pose_standing_curve=f['curves']['PoseStanding'],source_player_ticked=player['sync_read_buffer_tick_present'],target_player_completed_keys=node['clocks']))
assert snapshots[0]['source_pose_standing_amount']==0 and not snapshots[0]['source_player_ticked']and snapshots[0]['target_player_completed_keys']
assert snapshots[1]['source_pose_standing_amount']==1 and snapshots[1]['source_player_ticked']
result=dict(gate='RECORDED_FRESH_BOOT_CURVE_HISTORY_CONSUMER_BOUNDARY',node_guid=guid,snapshots=snapshots,
    source_initialization_public_pose_standing=source['initialization']['curves']['PoseStanding'],
    source_internal_proxy_animation_curve_map_observed=False,target_actual_prior_curve_global_input_observed=False,
    candidate_registered_as_difference=False,new_difference_number=None,
    next_required_fact='Observe the exact internal proxy curve input consumed by RefreshPose and the target prior-curve input before the first ordinary evaluation',
    first_cold_body_position_boundary=dict(source_frame=18,target_query=19,existing_source_floor_band_constant=True,known_related_difference='BF144-D31',actual_cold_motor_coefficients_observed=False),
    full_root_equivalence_claimed=False)
(r/'open-producer-boundaries.json').write_text(json.dumps(result,indent=2)+'\n')
print('Recorded exact first Default Pose eligibility boundary; internal curve inputs remain unobserved',flush=True)
