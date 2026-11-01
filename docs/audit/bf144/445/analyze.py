import gzip
import json
import sys
from pathlib import Path

r=Path(__file__).resolve().parent
label=sys.argv[1]
data=json.loads((r/f'runtime-{label}/parameters.json').read_text())
ready=[v for v in data['animation_after']if v['ordinal']is not None]
native=json.loads(gzip.decompress((r/'native-players.json.gz').read_bytes()))['frames']
decl=json.loads((r/'declarations.json').read_text())
table={key:dict(source=source,target=decl['target_declarations'][key],source_tick_frames=[],target_tick_frames=[],target_context_observed=False,paired_own_history_clock_differences=[])for key,source in decl['source_declarations'].items()}
for target,source in zip(ready,native,strict=True):
    for n in target['leaf_nodes']:
        if n['node_guid']in table:table[n['node_guid']]['target_context_observed']=True
    for player in source['players']:
        key=player['node_guid']
        row=table[key]
        clocks=[c for n in target['leaf_nodes']if n['node_guid']==key for c in n['clocks']]
        if player['sync_read_buffer_tick_present']:row['source_tick_frames'].append(source['frame'])
        if clocks:row['target_tick_frames'].append(target['ordinal'])
        if player['sync_read_buffer_tick_present']and len(clocks)==1:
            clock=clocks[0]
            time=clock['sample_time']if 'BlendSpace'in player['node_type']else clock['node_time']
            if time!=player['current_asset_time']:
                row['paired_own_history_clock_differences'].append(dict(frame=source['frame'],source_seconds=player['current_asset_time'],target_seconds=time))
idle='95e7e1d3-44eb-9b93-3f7f-ba8a92e9cc0e'
asset=json.loads((r/'idle.native.json').read_text())
properties={v['name']:v for v in asset['asset_properties']}
assert properties['AuthoredSyncMarkers']['exported_text']==''
assert properties['RateScale']['numeric_value']==1
duration=properties['SequenceLength']['numeric_value']
native_idle=[]
for frame in native:
    ticks=[p for p in frame['players']if p.get('sync_read_buffer_tick_present')and p.get('tick_group')=='Secondary Motion']
    assert len(ticks)==1 and ticks[0]['node_guid']==idle
    assert ticks[0]['tick_weight']==0.75 and ticks[0]['tick_delta']==0.03333333507180214
    native_idle.append(ticks[0])
(r/'idle-input.json').write_text(json.dumps(dict(node_guid=idle,duration=duration,delta=0.03333333507180214,native_ticks=native_idle),indent=2)+'\n')
result=dict(gate='CURRENT_FULL_PLAYER_TABLE_WITH_EXPLICIT_DIFFERENT_HISTORY_BOUNDARY',source_declarations=85,table=table,
    source_tick_rows=sum(len(v['source_tick_frames'])for v in table.values()),target_native_tick_rows=sum(len(v['target_tick_frames'])for v in table.values()),
    never_instantiated_target_declarations=sum(not v['target_context_observed']for v in table.values()),
    paired_own_history_clock_differences=sum(len(v['paired_own_history_clock_differences'])for v in table.values()),
    source_target_same_initial_history_claimed=False,source_target_all_clock_differences_attributed=False)
(r/f'player-table-{label}.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items()if k!='table'}),flush=True)
