import json
import math
import sys
import uuid
from pathlib import Path

data = json.loads(Path(sys.argv[1]).read_text(encoding='utf-8-sig'))
metadata = data['state_machine_players']['machines']
properties = {p['name']: p for p in data['blueprint_cdo_properties']}
objects = {o['object_path']: o for o in data['asset_objects']}
indices = {
    int(next(f['exported_text'] for f in p['fields'] if f['name'] == 'StateMachineIndexInClass'))
    for p in properties.values() if p['cpp_type'] == 'FAnimNode_StateMachine'
}
assert indices == {machine['index'] for machine in metadata}
assert len(indices) == len(metadata)
players = 0
for machine in metadata:
    assert [state['index'] for state in machine['states']] == list(range(len(machine['states'])))
    for state in machine['states']:
        nodes = state['players'] + state['layers']
        if 'root' in state:
            nodes = [state['root']] + nodes
        for node in nodes:
            original = objects[node['source_path']]
            source = {p['name']: p for p in original['properties']}
            assert str(uuid.UUID(source['NodeGuid']['exported_text'])) == node['guid']
            assert node['source_graph'] == node['source_path'].rsplit('.', 1)[0]
            assert source['Node']['cpp_type'] == properties[node['property']]['cpp_type'], (node['property'], node['source_path'])
            assert node['struct'].rsplit('.', 1)[1] == properties[node['property']]['cpp_type'].removeprefix('F')
            if node['struct'].endswith('.AnimNode_SequencePlayer'):
                effective = node['effective']
                assert set(effective) == {'play_rate', 'basis', 'start', 'loop', 'ignore'}
                assert all(math.isfinite(effective[key]) for key in ['play_rate', 'basis', 'start'])
                assert isinstance(effective['loop'], bool) and isinstance(effective['ignore'], bool)
                fields = {p['name']: p['exported_text'] for p in properties[node['property']]['fields']}
                if fields['Sequence'] != 'None':
                    assert node['asset_path'] == fields['Sequence'].split("'")[1]
                    assert math.isfinite(node['asset_duration']) and node['asset_duration'] >= 0
                    assert math.isfinite(node['asset_rate_scale'])
        players += len(state['players'])
assert players > 0
print(f'Compiled state machine metadata PASS: {len(metadata)} machines, {players} player references')
