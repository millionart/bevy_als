import argparse
import copy
import json
import math
import struct
from pathlib import Path
import test_character_braking_probe as braking


def validate(document, request):
    probe = document['character_velocity_probe']
    assert probe['format'] == 'ue_character_velocity_probe_v1'
    result = braking.validate({'character_braking_probe': dict(probe, format='ue_character_braking_probe_v1')}, request)
    keys = ['max_speed_cm_s', 'max_acceleration_cm_s2', 'min_analog_speed_cm_s', 'analog_input_modifier']
    actual_keys = ['actual_max_speed_cm_s', 'actual_max_acceleration_cm_s2', 'actual_min_analog_speed_cm_s', 'actual_analog_input_modifier']
    for actual, wanted in zip(probe['samples'], request['samples']):
        assert actual['movement_mode_name'] == 'MOVE_Walking'
        for key, actual_key in zip(keys, actual_keys):
            value = wanted[key]
            assert type(value) in [int, float] and math.isfinite(value)
            expected = struct.unpack('f', struct.pack('f', value))[0]
            assert actual[key] == actual[actual_key] == expected
        vector = wanted['acceleration_cm_s2']
        assert set(vector) == {'x', 'y', 'z'}
        assert all(type(v) in [int, float] and math.isfinite(v) for v in vector.values())
        assert actual['acceleration_cm_s2'] == vector
    result['exact_calc_velocity_inputs_and_walking_mode'] = True
    return result


def self_test():
    sample = {'name': 'cap', 'delta_seconds': 0, 'friction': 0, 'braking_deceleration_cm_s2': 1500, 'friction_factor': 0, 'braking_substep_seconds': 0.03, 'velocity_cm_s': {'x': 375, 'y': 0, 'z': 0}, 'acceleration_cm_s2': {'x': 2000, 'y': 0, 'z': 0}, 'max_speed_cm_s': 375, 'max_acceleration_cm_s2': 2000, 'min_analog_speed_cm_s': 25, 'analog_input_modifier': 1}
    actual = dict(sample, braking_substep_seconds=struct.unpack('f', struct.pack('f', 0.03))[0], before_velocity_cm_s=sample['velocity_cm_s'], after_velocity_cm_s=sample['velocity_cm_s'], has_valid_data_before=True, has_valid_data_after=True, actual_max_speed_cm_s=375, actual_max_acceleration_cm_s2=2000, actual_min_analog_speed_cm_s=25, actual_analog_input_modifier=1, movement_mode_name='MOVE_Walking')
    request = {'samples': [sample]}
    document = {'character_velocity_probe': {'format': 'ue_character_velocity_probe_v1', 'character_class': 'character', 'movement_class': 'movement', 'movement_registered': True, 'has_animation_root_motion': False, 'world_ticked': False, 'samples': [actual]}}
    assert validate(document, request)['samples'] == 1
    rejected = 0
    for index in range(8):
        bad = copy.deepcopy(document)
        wanted = copy.deepcopy(request)
        probe = bad['character_velocity_probe']
        row = probe['samples'][0]
        if index == 0:
            del row['acceleration_cm_s2']
        elif index == 1:
            wanted['samples'][0]['max_speed_cm_s'] = '375'
        elif index == 2:
            wanted['samples'][0]['acceleration_cm_s2']['x'] = True
        elif index == 3:
            row['movement_mode_name'] = 'MOVE_Flying'
        elif index == 4:
            row['actual_max_speed_cm_s'] = 600
        elif index == 5:
            row['has_valid_data_after'] = False
        elif index == 6:
            probe['world_ticked'] = True
        else:
            probe['format'] = 'ue_character_braking_probe_v1'
        try:
            validate(bad, wanted)
        except (KeyError, AssertionError):
            rejected += 1
    assert rejected == 8
    return {'self_test': 'PASS', 'invalid_calc_velocity_packets_rejected': rejected}


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
