import json
import sys
import uuid
from pathlib import Path

for argument in sys.argv[1:]:
    source = json.loads(Path(argument).read_text(encoding='utf-8-sig'))
    inventory = source['state_machine_players']['asset_players']
    properties = {p['name']: p for p in source['blueprint_cdo_properties']}
    expected = {name for name, p in properties.items() if {'GroupName', 'GroupRole', 'Method'} <= {f['name'] for f in p.get('fields', [])}}
    assert {node['property'] for node in inventory} == expected
    assert len({node['node_index'] for node in inventory}) == len(inventory)
    for node in inventory:
        uuid.UUID(node['guid'])
        fields = {f['name']: f for f in properties[node['property']]['fields']}
        sync = node['synchronization']
        assert sync['group'] == fields['GroupName']['exported_text']
        assert sync['role'] == fields['GroupRole']['exported_text']
        assert sync['method'] == fields['Method']['exported_text']
        if 'bOverridePositionWhenJoiningSyncGroupAsLeader' in fields:
            assert sync['override_position'] == (fields['bOverridePositionWhenJoiningSyncGroupAsLeader']['exported_text'] == 'True')
        else:
            assert node['struct'].endswith('.AnimNode_SequenceEvaluator') and sync['override_position'] is False
        assert sync['role'] in {'CanBeLeader', 'AlwaysFollower', 'AlwaysLeader', 'TransitionLeader', 'TransitionFollower', 'ExclusiveAlwaysLeader'}
        assert sync['method'] in {'DoNotSync', 'SyncGroup', 'Graph'}
        assert len(sync['marker_names']) == len(set(sync['marker_names']))
        assert all(isinstance(name, str) and name for name in sync['marker_names'])
        assert isinstance(sync['asset_is_null'], bool)
        if sync['asset_is_null']:
            assert sync['asset_path'] == 'None' and not sync['marker_names']
        else:
            assert sync['asset_path'].startswith('/ALS/')
    print(f"Compiled asset players and effective sync getters PASS: {len(inventory)} players in {source['asset_path']}")
