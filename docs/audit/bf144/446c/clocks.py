import gzip
import json
from pathlib import Path

r=Path(__file__).resolve().parent
target=json.loads(Path('docs/audit/bf144/446/runtime-restored/parameters.json').read_bytes())['animation_after']
source=json.loads(gzip.decompress(Path('docs/audit/bf144/446b/restored.native.json.gz').read_bytes()))['character_animation_probe']['cases'][0]
decl=json.loads(Path('docs/audit/bf144/445/declarations.json').read_text())
table={key:dict(source=value,target=decl['target_declarations'][key],source_tick_frames=[],target_tick_frames=[],clock_differences=[],eligibility_differences=[])for key,value in decl['source_declarations'].items()}
idle='95e7e1d3-44eb-9b93-3f7f-ba8a92e9cc0e'
idle_source=[]
idle_target=[]
for index,(native,current)in enumerate(zip(source['frames'],target[1:],strict=True)):
    active={}
    for node in current['leaf_nodes']:
        if node['node_guid']in table:
            active.setdefault(node['node_guid'],[]).extend(node['clocks'])
    for instance in native['linked_instances']:
        for player in instance['asset_players']:
            key=player['node_guid']
            assert key in table
            ticked=player['sync_read_buffer_tick_present']
            clocks=active.get(key,[])
            row=table[key]
            if ticked:row['source_tick_frames'].append(index)
            if clocks:row['target_tick_frames'].append(index)
            if ticked!=bool(clocks):row['eligibility_differences'].append(index)
            if ticked and len(clocks)==1:
                clock=clocks[0]
                actual=clock['sample_time']if 'BlendSpace'in player['node_type']else clock['node_time']
                if actual!=player['current_asset_time']:
                    row['clock_differences'].append(dict(source_frame=index,target_query=index+1,target_ordinal=current['ordinal'],source_seconds=player['current_asset_time'],target_seconds=actual))
            if key==idle:
                assert ticked and len(clocks)==1
                idle_source.append(dict(time=player['current_asset_time'],previous=player['tick_previous'],delta=player['tick_delta']))
                idle_target.append(dict(time=clocks[0]['node_time'],previous=clocks[0]['relevant']['previous_seconds'],delta=clocks[0]['sync'][0]['delta_seconds']))
assert len(idle_source)==len(idle_target)==318
(r/'idle-input.json').write_text(json.dumps(dict(source=idle_source,target=idle_target,duration=2.6666667461395264),indent=2)+'\n')
result=dict(gate='PASS_COMPLETE318_ACTUAL_PLAYER_TABLE_WITH_PRODUCER_BOUNDARIES',source_declarations=len(table),source_tick_rows=sum(len(v['source_tick_frames'])for v in table.values()),target_tick_rows=sum(len(v['target_tick_frames'])for v in table.values()),
    eligibility_differences=sum(len(v['eligibility_differences'])for v in table.values()),clock_differences=sum(len(v['clock_differences'])for v in table.values()),
    matched_external_inputs=True,all_physical_and_animation_producer_inputs_matched=False,all_differences_attributed=False,table=table)
(r/'player-table.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items()if k!='table'}),flush=True)
