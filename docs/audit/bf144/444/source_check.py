import gzip
import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
projection=gzip.decompress((r/'source-animation-phases.json.gz').read_bytes())
data=json.loads(projection)
compression=json.loads((r/'projection-compression.json').read_text())
assert len(projection)==compression['uncompressed_bytes']
assert hashlib.sha256(projection).hexdigest()==compression['uncompressed_sha256']
frames=data['frames']
assert len(frames)==240
assert data['initialization']['update_counter']==0
assert data['initialization']['evaluation_counter']==0
slots=set()
for i,frame in enumerate(frames):
    assert frame['frame']==i and not frame['pose_ticked_before_world']
    assert frame['update_counter']==frame['evaluation_counter']==i+1
    for key,count in [('instances_before_tick',i),('linked_instances',i+1)]:
        main=[v for v in frame[key] if v['main']]
        assert len(main)==1 and main[0]['update_counter']==main[0]['evaluation_counter']==count
    assert frame['world_delta_seconds']==0.03333333507180214
    for instance in frame['linked_instances']:
        for player in instance['asset_players']:
            slots.add((instance['class'],player['node_guid']))
            assert isinstance(player['current_asset_time'],(float,int))
assert len(slots)==85
result=dict(gate='PASS_NATIVE_CURRENT240_ANIMATION_PHASE_AND_CLOCK_ENTRY_QUALIFICATION',frames=240,main_updates_per_world_step=1,main_evaluations_per_world_step=1,
    initial_update_counter=0,source_asset_player_slots=len(slots),exact_native_asset_clock_fields_preserved=True,source_state_text_precision='six decimal places; not an exact scalar or vector oracle',
    projection_sha256=hashlib.sha256(projection).hexdigest(),source_target_same_animation_history_claimed=False)
(r/'source-check.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result),flush=True)
