import argparse
import copy
import json
from pathlib import Path

from test_rig_node_history_probe import validate as validate_history


def transform(value):
    return [value[key][axis] for key, axes in [('translation', 'xyz'), ('rotation', 'xyzw'), ('scale', 'xyz')] for axis in axes]


def validate(document, request):
    result = validate_history(document, request)
    observed = 0
    for case, wanted in zip(document['rig_node_history_probe']['cases'], request['cases']):
        for actual, frame in zip(case['frames'], wanted['frames']):
            if 'mesh_to_world' not in frame:
                continue
            value = frame['mesh_to_world']
            expected = value['translation_cm'] + value['rotation'] + value['scale']
            assert transform(actual['mesh_to_world']) == expected, frame['name']
            forward = [v for v in actual['executions'] if v['entry'] == 'Forwards Solve']
            assert len(forward) == 1 and transform(forward[0]['rig_to_world']) == expected, frame['name']
            observed += 1
    assert observed
    return {**result, 'requested_world_transforms_consumed_exact': observed}


def self_test():
    expected = {'translation_cm': [1, 2, 3], 'rotation': [0, 0, 0, 1], 'scale': [1, 1, 1]}
    world = {'translation': dict(zip('xyz', [1, 2, 3])), 'rotation': dict(zip('xyzw', [0, 0, 0, 1])), 'scale': dict.fromkeys('xyz', 1)}
    frame = {'name': '0', 'rig_id': 1, 'delta_seconds_after_update': 0, 'leaf_update_weight': 1, 'forward_events': 1, 'construction_events': 1, 'attribute_count': 0, 'before': {'bones': [{'name': 'root'}]}, 'after': {'bones': [{'name': 'root'}]}, 'work_after': {'properties': [{'name': 'state'}]}, 'externals_after': {'input': {}}, 'reference_query_after_forward': {'blocking_hit': True}, 'mesh_to_world': world, 'executions': [{'entry': 'Forwards Solve', 'succeeded': True, 'instructions': [0, 1], 'rig_to_world': copy.deepcopy(world)}]}
    request = {'bone_names': ['root'], 'cases': [{'name': 'case', 'frames': [{'name': '0', 'delta_seconds': 0, 'mesh_to_world': expected}]}]}
    document = {'rig_node_history_probe': {'cases': [{'name': 'case', 'game_world': True, 'bound_component': True, 'adapter_enabled': True, 'adapter_ready': True, 'external_controls_stage': 'after_update_before_evaluate', 'frames': [frame]}]}}
    assert validate(document, request)['requested_world_transforms_consumed_exact'] == 1
    rejected = 0
    for index in range(4):
        data = copy.deepcopy(document)
        row = data['rig_node_history_probe']['cases'][0]['frames'][0]
        if index == 0:
            row.pop('mesh_to_world')
        elif index == 1:
            row['executions'][0].pop('rig_to_world')
        elif index == 2:
            row['mesh_to_world']['translation']['x'] = 0
        else:
            row['executions'][0]['rig_to_world']['translation']['z'] = 0
        try:
            validate(data, request)
        except (AssertionError, KeyError):
            rejected += 1
    assert rejected == 4
    return {'self_test': 'PASS', 'invalid_world_observations_rejected': rejected}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--input', type=Path)
    parser.add_argument('--request', type=Path)
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(self_test()))
    else:
        assert args.input and args.request
        print(json.dumps(validate(json.loads(args.input.read_text()), json.loads(args.request.read_text()))))
