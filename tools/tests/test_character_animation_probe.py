import argparse
import copy
import json
import struct
from pathlib import Path


def validate(document, request):
    cases = document['character_animation_probe']['cases']
    assert len(cases) == len(request['cases']) > 0
    assert len({case['name'] for case in cases}) == len(cases)
    count = 0
    for case, wanted in zip(cases, request['cases']):
        assert case['name'] == wanted['name']
        assert case['floor_dynamic'] == wanted['dynamic_base']
        assert case['source_character_bound'] and case['mesh_tick_after_character']
        assert case['character_class'] and case['animation_class'] and case['settings']
        assert len(case['frames']) == len(wanted['frames']) > 0
        update = case['initial_update_counter']
        evaluation = case['initial_evaluation_counter']
        forward = 0
        previous_time = 0
        for index, (frame, expected) in enumerate(zip(case['frames'], wanted['frames'])):
            assert frame['frame'] == index
            delta = struct.unpack('f', struct.pack('f', expected['delta_seconds']))[0]
            assert frame['world_delta_seconds'] == delta
            assert frame['gravity_cm_s2'] == request['gravity_cm_s2']
            assert frame['world_seconds'] > previous_time
            previous_time = frame['world_seconds']
            assert frame['actor_begun_play']
            if not frame.get('pose_ticked_before_world', False):
                assert frame['recently_rendered'] == expected['rendered']
            if expected['rendered'] and not frame.get('pose_ticked_before_world', False):
                update += 1
                evaluation += 1
                forward += 1
                assert frame['different_reference_bones'] > 0 and frame['curves']
                assert frame['rig_executions'] and all(event['succeeded'] for event in frame['rig_executions'])
            elif not expected['rendered']:
                assert frame['different_reference_bones'] == 0 and not frame['rig_executions']
            else:
                assert not frame['rig_executions']
            assert frame['update_counter'] == update
            assert frame['evaluation_counter'] == evaluation
            assert frame['rig_forward_events'] == forward
            assert len(frame['getter_before_tick']['fields']) == len(frame['getter_after_tick']['fields']) == len(frame['rig_variables_after_tick']['RigInput']['fields']) == 11
            bones = frame['mesh_local_pose']
            assert bones and len({bone['name'] for bone in bones}) == len(bones)
            instances = frame['linked_instances']
            assert instances and sum(instance['main'] for instance in instances) == 1
            assert all(instance['class'] for instance in instances)
            if frame['based_on_floor']:
                assert frame['base_relative_location'] == wanted['dynamic_base']
            count += 1
    return {'cases': len(cases), 'frames': count, 'complete_native_source_instances': True, 'original_target_pose_equivalence_claimed': False}


def self_test():
    fields = [{} for _ in range(11)]
    frame = {'frame': 0, 'world_delta_seconds': 0.125, 'gravity_cm_s2': -981, 'world_seconds': .125, 'actor_begun_play': True, 'recently_rendered': True, 'different_reference_bones': 1, 'curves': {'curve': 1}, 'rig_executions': [{'succeeded': True}], 'update_counter': 1, 'evaluation_counter': 1, 'rig_forward_events': 1, 'getter_before_tick': {'fields': fields}, 'getter_after_tick': {'fields': fields}, 'rig_variables_after_tick': {'RigInput': {'fields': fields}}, 'mesh_local_pose': [{'name': 'root'}], 'linked_instances': [{'class': 'source', 'main': True}], 'based_on_floor': True, 'base_relative_location': False}
    case = {'name': 'visible', 'floor_dynamic': False, 'source_character_bound': True, 'mesh_tick_after_character': True, 'character_class': 'character', 'animation_class': 'animation', 'settings': 'settings', 'initial_update_counter': 0, 'initial_evaluation_counter': 0, 'frames': [frame]}
    request = {'gravity_cm_s2': -981, 'cases': [{'name': 'visible', 'dynamic_base': False, 'frames': [{'delta_seconds': .125, 'rendered': True}]}]}
    fixture = {'character_animation_probe': {'cases': [case]}}
    assert validate(fixture, request)['frames'] == 1
    skipped = copy.deepcopy(fixture)
    first = skipped['character_animation_probe']['cases'][0]['frames'][0]
    first.update(pose_ticked_before_world=True, update_counter=0, evaluation_counter=0, rig_forward_events=0, rig_executions=[])
    assert validate(skipped, request)['frames'] == 1
    rejected = 0
    for index in range(8):
        sample = copy.deepcopy(fixture)
        row = sample['character_animation_probe']['cases'][0]
        if index == 0:
            row['source_character_bound'] = False
        elif index == 1:
            row['mesh_tick_after_character'] = False
        elif index == 2:
            row['frames'][0]['rig_forward_events'] = 0
        elif index == 3:
            row['frames'][0]['base_relative_location'] = True
        elif index == 4:
            row['frames'][0]['world_delta_seconds'] = .1
        elif index == 5:
            row['frames'][0]['getter_after_tick']['fields'] = []
        elif index == 6:
            row['frames'][0]['different_reference_bones'] = 0
        else:
            row['frames'][0]['evaluation_counter'] = 0
        try:
            validate(sample, request)
        except (KeyError, AssertionError):
            rejected += 1
    assert rejected == 8
    return {'self_test': 'PASS', 'invalid_cases_rejected': rejected}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--input', type=Path)
    parser.add_argument('--request', type=Path)
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    if args.self_test:
        result = self_test()
    else:
        assert args.input and args.request
        result = validate(json.loads(args.input.read_text()), json.loads(args.request.read_text()))
    print(json.dumps(result))
