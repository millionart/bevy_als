import copy
import gzip
import json
import math
import sys
from pathlib import Path

r=Path(__file__).resolve().parent


def validate(data):
    case=data['character_animation_probe']['cases'][0]
    total=active=0
    bs_current_seconds=0
    stale_weight=0
    for index,frame in enumerate(case['frames']):
        assert frame['update_counter']==index+1
        for instance in frame['linked_instances']:
            for player in instance['asset_players']:
                total+=1
                present=player['sync_read_buffer_tick_present']
                assert isinstance(present,bool)
                assert player['sync_read_buffer_tick_matches']==int(present)
                if present:
                    active+=1
                    for k in ['tick_weight','tick_previous','tick_delta','tick_accumulator']:
                        assert math.isfinite(player[k])
                    assert isinstance(player['tick_evaluator'],bool)
                    if 'BlendSpace' in player['node_type'] and player.get('blend_samples'):
                        highest=max(player['blend_samples'],key=lambda v:v['weight'])
                        assert player['current_asset_time']==highest['cached_time']
                        bs_current_seconds+=1
                elif player['cached_blend_weight']>0:
                    stale_weight+=1
    assert total==20400 and active>0 and bs_current_seconds>0
    return dict(total_player_rows=total,actual_tick_rows=active,blendspace_current_time_confirmed_seconds=bs_current_seconds,cached_positive_but_not_ticked_rows=stale_weight)


if __name__=='__main__':
    label=sys.argv[1]
    path=r/f'{label}.native.json'
    data=json.loads(path.read_bytes()if path.exists()else gzip.decompress((r/f'{label}.native.json.gz').read_bytes()))
    result=validate(data)
    rejected=0
    for field,value in [('sync_read_buffer_tick_present',None),('sync_read_buffer_tick_matches',7),('tick_accumulator',float('nan'))]:
        bad=copy.deepcopy(data)
        row=next(p for f in bad['character_animation_probe']['cases'][0]['frames']for i in f['linked_instances']for p in i['asset_players']if p.get('sync_read_buffer_tick_present'))
        if value is None:
            del row[field]
        else:
            row[field]=value
        try:
            validate(bad)
        except (AssertionError,KeyError):
            rejected+=1
            continue
        raise AssertionError(field)
    result['negative_fixtures_rejected']=rejected
    (r/f'{label}-check.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result),flush=True)
