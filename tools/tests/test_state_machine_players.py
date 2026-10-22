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
compiled_nodes = {}
for machine in metadata:
    assert [state['index'] for state in machine['states']] == list(range(len(machine['states'])))
    assert machine['initial_state'] in range(len(machine['states']))
    for state in machine['states']:
        assert isinstance(state['always_reset'], bool)
        for edge in state['exits']:
            transition = objects[edge['transition_path']]
            assert transition['class_path'] == '/Script/AnimGraph.AnimStateTransitionNode'
            transition_guid = next(p['exported_text'] for p in transition['properties'] if p['name'] == 'NodeGuid')
            assert str(uuid.UUID(transition_guid)) == edge['transition_guid']
            assert edge['source'] == state['index']
            assert edge['target'] in range(len(machine['states']))
            assert edge['logic'] in [0, 1, 2]
            assert math.isfinite(edge['duration']) and edge['duration'] >= 0
            assert math.isfinite(edge['min_reentry_time'])
            assert isinstance(edge['inertial_on_self'], bool)
            assert isinstance(edge['automatic'], bool) and isinstance(edge['only_active'], bool)
            assert isinstance(edge['desired_result'], bool)
            assert math.isfinite(edge['trigger'])
            assert edge['rule']['struct'].endswith('.AnimNode_TransitionResult')
        nodes = state['players'] + state['layers']
        nodes += [edge['rule'] for edge in state['exits']]
        if 'root' in state:
            nodes = [state['root']] + nodes
        for node in nodes:
            previous = compiled_nodes.setdefault(node['node_index'], node)
            assert previous == node
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
for node in compiled_nodes.values():
    if 'shared_origin_node_index' in node:
        peer = compiled_nodes[node['shared_origin_node_index']]
        assert node['node_index'] != peer['node_index']
        assert all(node[key] == peer[key] for key in ['guid', 'source_path', 'source_graph', 'struct'])
assert players > 0
print(f'Compiled state machine metadata PASS: {len(metadata)} machines, {players} player references')
