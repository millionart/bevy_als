import argparse
import copy
import json
import math
from pathlib import Path


def validate(document, request=None):
    count = 0
    for case in document['character_animation_probe']['cases']:
        previous = case['initialization']['movement_component']
        snapshots = [previous]
        for frame in case['frames']:
            before = frame['movement_before_tick']
            after = frame['movement_after_tick']
            assert {k: v for k, v in before.items() if k != 'pending_input_world'} == {k: v for k, v in previous.items() if k != 'pending_input_world'}
            assert after['last_input_world'] == before['pending_input_world']
            assert after['pending_input_world'] == {'x': 0, 'y': 0, 'z': 0}
            assert after['velocity_cm_s'] == frame['actor_velocity_cm_s']
            snapshots.extend([before, after])
            previous = after
        for snapshot in snapshots:
            mode = snapshot['movement_mode']
            assert type(mode) is int and 0 <= mode <= 6
            assert snapshot['movement_mode_name'] == ['MOVE_None', 'MOVE_Walking', 'MOVE_NavWalking', 'MOVE_Falling', 'MOVE_Swimming', 'MOVE_Flying', 'MOVE_Custom'][mode]
            assert type(snapshot['custom_movement_mode']) is int and 0 <= snapshot['custom_movement_mode'] <= 255
            assert snapshot['character_locomotion_mode'].startswith('(TagName=')
            for key in ['registered', 'begun_play', 'tick_registered', 'tick_enabled', 'walkable_floor']:
                assert type(snapshot[key]) is bool
            distance = snapshot['floor_distance_cm']
            assert type(distance) in [float, int] and math.isfinite(distance)
            maximum = snapshot['max_acceleration_cm_s2']
            assert type(maximum) in [float, int] and math.isfinite(maximum) and maximum >= 0
            for key in ['velocity_cm_s', 'last_update_location_cm', 'last_update_velocity_cm_s', 'pending_input_world', 'last_input_world', 'acceleration_cm_s2']:
                assert set(snapshot[key]) == {'x', 'y', 'z'}
                assert all(type(v) in [float, int] and math.isfinite(v) for v in snapshot[key].values())
            count += 1
    assert count > 0
    if request is not None:
        cases = document['character_animation_probe']['cases']
        assert len(cases) == len(request['cases'])
        for actual, wanted in zip(cases, request['cases']):
            assert actual['initial_engine_frame'] == request.get('initial_engine_frame', 0)
            assert len(actual['frames']) == len(wanted['frames'])
            for frame, step in zip(actual['frames'], wanted['frames']):
                vector = step.get('movement_input_world', {'x': 0, 'y': 0, 'z': 0})
                assert set(vector) == {'x', 'y', 'z'}
                assert all(type(v) in [float, int] and math.isfinite(v) for v in vector.values())
                if 'movement_input_world' in step:
                    assert wanted.get('local_controller') is True
                assert frame['movement_before_tick']['pending_input_world'] == vector
    return {'movement_snapshots': count, 'continuous_before_after_fields_exact': True}


def self_test():
    snapshot = {'movement_mode': 3, 'movement_mode_name': 'MOVE_Falling', 'custom_movement_mode': 0, 'character_locomotion_mode': '(TagName="Als.LocomotionMode.InAir")', 'registered': True, 'begun_play': True, 'tick_registered': True, 'tick_enabled': True, 'walkable_floor': False, 'floor_distance_cm': 0, 'velocity_cm_s': {'x': 0, 'y': 0, 'z': -1}, 'last_update_location_cm': {'x': 0, 'y': 0, 'z': 150}, 'last_update_velocity_cm_s': {'x': 0, 'y': 0, 'z': -1}}
    snapshot.update({'pending_input_world': {'x': 0, 'y': 0, 'z': 0}, 'last_input_world': {'x': 0, 'y': 0, 'z': 0}, 'acceleration_cm_s2': {'x': 0, 'y': 0, 'z': 0}, 'max_acceleration_cm_s2': 100})
    case = {'initialization': {'movement_component': snapshot}, 'frames': [{'movement_before_tick': snapshot, 'movement_after_tick': snapshot, 'actor_velocity_cm_s': snapshot['velocity_cm_s']}]}
    fixture = {'character_animation_probe': {'cases': [case]}}
    assert validate(fixture)['movement_snapshots'] == 3
    rejected = 0
    for index in range(6):
        sample = copy.deepcopy(fixture)
        row = sample['character_animation_probe']['cases'][0]['frames'][0]
        if index == 0:
            del row['movement_after_tick']
        elif index == 1:
            row['movement_after_tick'] = dict(row['movement_after_tick'], movement_mode=7)
        elif index == 2:
            row['movement_after_tick'] = dict(row['movement_after_tick'], tick_enabled=1)
        elif index == 3:
            row['movement_after_tick'] = dict(row['movement_after_tick'], floor_distance_cm=float('nan'))
        elif index == 4:
            row['movement_before_tick'] = dict(row['movement_before_tick'], movement_mode=1)
        else:
            row['actor_velocity_cm_s'] = {'x': 0, 'y': 0, 'z': 0}
        try:
            validate(sample)
        except (KeyError, AssertionError):
            rejected += 1
    assert rejected == 6
    request = {'initial_engine_frame': 17, 'cases': [{'local_controller': True, 'frames': [{'movement_input_world': {'x': 1, 'y': 0, 'z': 0}}]}]}
    active = copy.deepcopy(fixture)
    case = active['character_animation_probe']['cases'][0]
    case['initial_engine_frame'] = 17
    case['frames'][0]['movement_before_tick'] = dict(snapshot, pending_input_world={'x': 1, 'y': 0, 'z': 0})
    case['frames'][0]['movement_after_tick'] = dict(snapshot, last_input_world={'x': 1, 'y': 0, 'z': 0})
    assert validate(active, request)['movement_snapshots'] == 3
    for index in range(4):
        bad = copy.deepcopy(request)
        vector = bad['cases'][0]['frames'][0]['movement_input_world']
        if index == 0:
            vector['x'] = '1'
        elif index == 1:
            vector['y'] = float('nan')
        elif index == 2:
            del vector['z']
        else:
            bad['cases'][0]['local_controller'] = False
        try:
            validate(active, bad)
        except (KeyError, AssertionError):
            rejected += 1
    assert rejected == 10
    for key in ['last_input_world', 'max_acceleration_cm_s2']:
        bad = copy.deepcopy(active)
        after = bad['character_animation_probe']['cases'][0]['frames'][0]['movement_after_tick']
        after[key] = {'x': 0, 'y': 0, 'z': 0} if key == 'last_input_world' else float('inf')
        try:
            validate(bad, request)
        except AssertionError:
            rejected += 1
    assert rejected == 12
    return {'self_test': 'PASS', 'invalid_movement_packets_and_requests_rejected': rejected}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--input', type=Path)
    parser.add_argument('--request', type=Path)
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    if args.self_test:
        result = self_test()
    else:
        assert args.input
        result = validate(json.loads(args.input.read_text()), json.loads(args.request.read_text()) if args.request else None)
    print(json.dumps(result))
