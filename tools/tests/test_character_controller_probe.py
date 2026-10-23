import argparse
import copy
import json
import math
from pathlib import Path


def validate(document, request):
    count = 0
    assert len(document['character_animation_probe']['cases']) == len(request['cases']) > 0
    for case, wanted in zip(document['character_animation_probe']['cases'], request['cases']):
        assert len(case['frames']) == len(wanted['frames']) > 0
        requested = wanted.get('local_controller', False)
        assert type(requested) is bool
        for frame in wanted['frames']:
            if 'control_rotation_degrees' in frame:
                rotation = frame['control_rotation_degrees']
                assert requested and set(rotation) == {'pitch', 'yaw', 'roll'}
                assert all(type(v) in [float, int] and math.isfinite(v) for v in rotation.values())
        assert case['initial_engine_frame'] == request.get('initial_engine_frame', 0)
        for row in [case['initialization']] + case['frames']:
            if not requested:
                assert 'controller_input' not in row
                continue
            if 'controller_before_tick' in row:
                before = row['controller_before_tick']
                assert before['possesses_source_character'] and before['is_local_controller']
            controller = row['controller_input']
            assert controller['requested'] is True
            assert controller['possesses_source_character'] is True
            assert controller['is_local_controller'] is True
            assert controller['character_is_locally_controlled'] is True
            assert controller['has_authority'] is True
            assert controller['controller_class'] and controller['game_mode_class']
            rotation = controller['control_rotation_degrees']
            assert set(rotation) == {'pitch', 'yaw', 'roll'}
            assert all(type(v) in [float, int] and math.isfinite(v) for v in rotation.values())
            if 'DesiredStance' in controller:
                assert controller['DesiredStance'].startswith('(TagName=')
                assert controller['DesiredGait'].startswith('(TagName=')
            if 'ViewState' in controller:
                assert controller['ViewState'] and controller['HeadState']
            count += 1
    return {'controller_packets': count, 'actual_local_possession_verified': True}


def self_test():
    controller = {'requested': True, 'possesses_source_character': True, 'is_local_controller': True, 'character_is_locally_controlled': True, 'has_authority': True, 'controller_class': 'controller', 'game_mode_class': 'mode', 'control_rotation_degrees': {'pitch': 0, 'yaw': 0, 'roll': 0}}
    fixture = {'character_animation_probe': {'cases': [{'initial_engine_frame': 17, 'initialization': {'controller_input': controller}, 'frames': [{'controller_input': controller}]}]}}
    request = {'initial_engine_frame': 17, 'cases': [{'local_controller': True, 'frames': [{}]}]}
    assert validate(fixture, request)['controller_packets'] == 2
    rejected = 0
    for key in ['possesses_source_character', 'is_local_controller', 'character_is_locally_controlled']:
        sample = copy.deepcopy(fixture)
        sample['character_animation_probe']['cases'][0]['frames'][0]['controller_input'][key] = False
        try:
            validate(sample, request)
        except AssertionError:
            rejected += 1
    truncated = copy.deepcopy(fixture)
    truncated['character_animation_probe']['cases'] = []
    try:
        validate(truncated, request)
    except AssertionError:
        rejected += 1
    wrong_request = copy.deepcopy(request)
    wrong_request['cases'][0]['frames'][0]['control_rotation_degrees'] = {'pitch': '30', 'yaw': 0, 'roll': 0}
    try:
        validate(fixture, wrong_request)
    except AssertionError:
        rejected += 1
    assert rejected == 5
    return {'self_test': 'PASS', 'invalid_controller_states_rejected': rejected}


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
