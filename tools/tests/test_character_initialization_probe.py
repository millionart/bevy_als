import argparse
import copy
import json
from pathlib import Path


def validate(document):
    cases = document['character_animation_probe']['cases']
    assert cases
    for case in cases:
        initial = case['initialization']
        assert initial['stage'] == 'after_begin_play_before_world_tick'
        assert initial['world_seconds'] == 0
        assert initial['update_counter'] == case['initial_update_counter']
        assert initial['evaluation_counter'] == case['initial_evaluation_counter']
        assert initial['getter'] == case['frames'][0]['getter_before_tick']
        assert initial['mesh_local_pose']
        assert [bone['name'] for bone in initial['mesh_local_pose']] == [bone['name'] for bone in case['frames'][0]['mesh_local_pose']]
        assert sum(instance['main'] for instance in initial['instances']) == 1
        states = {field['name']: field for field in initial['source_state']}
        assert {'LocomotionMode', 'Stance', 'bPendingUpdate', 'PoseState', 'FeetState'} <= states.keys()
        assert initial['curves']
    return {'cases': len(cases), 'initialization_stage_recorded': True}


def self_test():
    fields = [{'name': name} for name in ['LocomotionMode', 'Stance', 'bPendingUpdate', 'PoseState', 'FeetState']]
    initial = {'stage': 'after_begin_play_before_world_tick', 'world_seconds': 0, 'update_counter': 0, 'evaluation_counter': 0, 'getter': {}, 'mesh_local_pose': [{'name': 'root'}], 'instances': [{'main': True}], 'source_state': fields, 'curves': {'curve': 1}}
    case = {'initialization': initial, 'initial_update_counter': 0, 'initial_evaluation_counter': 0, 'frames': [{'getter_before_tick': {}, 'mesh_local_pose': [{'name': 'root'}]}]}
    fixture = {'character_animation_probe': {'cases': [case]}}
    assert validate(fixture)['cases'] == 1
    rejected = 0
    for index in range(5):
        sample = copy.deepcopy(fixture)
        value = sample['character_animation_probe']['cases'][0]['initialization']
        if index == 0:
            value['world_seconds'] = 1
        elif index == 1:
            value['stage'] = 'after_tick'
        elif index == 2:
            value['source_state'] = []
        elif index == 3:
            value['mesh_local_pose'][0]['name'] = 'wrong'
        else:
            value['update_counter'] = 1
        try:
            validate(sample)
        except (KeyError, AssertionError):
            rejected += 1
    assert rejected == 5
    return {'self_test': 'PASS', 'invalid_cases_rejected': rejected}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument('--input', type=Path)
    group.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    print(json.dumps(self_test() if args.self_test else validate(json.loads(args.input.read_text()))))
