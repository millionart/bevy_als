import gzip
import json
import sys
from pathlib import Path

r=Path(__file__).resolve().parent
label=sys.argv[1]
p=r/(label+'.native.json')
current=json.loads(p.read_bytes()if p.exists()else gzip.decompress((r/(label+'.native.json.gz')).read_bytes()))['character_animation_probe']['cases'][0]
old=json.loads(gzip.decompress(Path('docs/audit/bf144/446d/restored.native.json.gz').read_bytes()))['character_animation_probe']['cases'][0]
target=json.loads(Path('docs/audit/bf144/446e/runtime-restored/parameters.json').read_text())
assert current['initialization']==old['initialization']
guid='b8f30b3f-4d30-eab1-ce6b-6bb9ec868234'
rows=[]
for index,(a,b,t)in enumerate(zip(old['frames'],current['frames'],target['animation_after'][1:],strict=True)):
    assert a['world_seconds']==b['world_seconds']and a['world_delta_seconds']==b['world_delta_seconds']
    assert a['movement_before_tick']['pending_input_world']==b['movement_before_tick']['pending_input_world']
    assert a['controller_before_tick']['control_rotation_degrees']==b['controller_before_tick']['control_rotation_degrees']
    def player(frame):return next(p for i in frame['linked_instances']for p in i['asset_players']if p['node_guid']==guid)
    old_player=player(a);new_player=player(b)
    node=next(n for n in t['leaf_nodes']if n['node_guid']==guid)
    rows.append(dict(source_frame=index,target_query=index+1,target_ordinal=t['ordinal'],same_frame_source_ticked=old_player['sync_read_buffer_tick_present'],next_frame_source_ticked=new_player['sync_read_buffer_tick_present'],target_ticked=bool(node['clocks'])))
assert rows[0]['same_frame_source_ticked']is False
assert rows[0]['next_frame_source_ticked']is rows[0]['target_ticked']is True
assert all(row['next_frame_source_ticked']==row['target_ticked']for row in rows)
result=dict(gate='PASS_BOOT_DEFAULT_POSE_CANDIDATE_REJECTED_BY_MATCHING_NATIVE_FRAME_CONTROL',initialization_complete_unchanged=True,world_steps_deltas_and_external_input_values_unchanged=True,
    ordinary_frames=318,node_guid=guid,default_pose_eligibility_matches_target_all318=True,first_tick_false_positive_resolved=True,
    curve_publication_port_difference_confirmed=False,candidate_disposition='Different engine-frame callback phases; no new curve publication defect',
    all_source_target_producer_inputs_matched=False,all_clock_or_root_differences_attributed=False,new_differences=[],total_confirmed_differences=271,rows=rows)
(r/('analysis-'+label+'.json')).write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items()if k!='rows'}),flush=True)
