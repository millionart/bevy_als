import argparse
import copy
import json
import math
import struct
from pathlib import Path


def validate(document, request):
    probe = document['character_braking_probe']
    assert probe['format'] == 'ue_character_braking_probe_v1'
    assert probe['character_class'] and probe['movement_class']
    assert probe['movement_registered'] is True
    assert probe['has_animation_root_motion'] is False
    assert probe['world_ticked'] is False
    assert len(probe['samples']) == len(request['samples']) > 0
    keys = ['delta_seconds', 'friction', 'braking_deceleration_cm_s2', 'friction_factor', 'braking_substep_seconds']
    for actual, wanted in zip(probe['samples'], request['samples']):
        assert actual['name'] == wanted['name']
        assert actual['has_valid_data_before'] is True and actual['has_valid_data_after'] is True
        for key in keys:
            value = wanted[key]
            assert type(value) in [int, float] and math.isfinite(value)
            assert actual[key] == struct.unpack('f', struct.pack('f', value))[0]
        assert abs(wanted['delta_seconds']) <= 0.25
        wanted_velocity = wanted['velocity_cm_s']
        assert set(wanted_velocity) == {'x', 'y', 'z'}
        assert all(type(v) in [int, float] and math.isfinite(v) for v in wanted_velocity.values())
        for key in ['before_velocity_cm_s', 'after_velocity_cm_s']:
            vector = actual[key]
            assert set(vector) == {'x', 'y', 'z'}
            assert all(type(v) in [int, float] and math.isfinite(v) for v in vector.values())
        assert actual['before_velocity_cm_s'] == wanted['velocity_cm_s']
    return {'samples': len(probe['samples']), 'valid_native_component_and_exact_inputs': True}


def self_test():
    sample = {'name': 'zero', 'delta_seconds': 0, 'friction': 0, 'braking_deceleration_cm_s2': 1500, 'friction_factor': 0, 'braking_substep_seconds': 0.03, 'velocity_cm_s': {'x': 100, 'y': 0, 'z': 0}}
    actual = dict(sample, braking_substep_seconds=struct.unpack('f', struct.pack('f', 0.03))[0], before_velocity_cm_s=sample['velocity_cm_s'], after_velocity_cm_s=sample['velocity_cm_s'], has_valid_data_before=True, has_valid_data_after=True)
    request = {'samples': [sample]}
    document = {'character_braking_probe': {'format': 'ue_character_braking_probe_v1', 'character_class': 'character', 'movement_class': 'movement', 'movement_registered': True, 'has_animation_root_motion': False, 'world_ticked': False, 'samples': [actual]}}
    assert validate(document, request)['samples'] == 1
    rejected = 0
    for index in range(5):
        bad = copy.deepcopy(document)
        probe = bad['character_braking_probe']
        if index == 0:
            probe['samples'][0]['has_valid_data_before'] = False
        elif index == 1:
            probe['world_ticked'] = True
        elif index == 2:
            probe['samples'][0]['after_velocity_cm_s']['x'] = float('nan')
        elif index == 3:
            probe['samples'] = []
        else:
            probe['samples'][0]['delta_seconds'] = 0.1
        try:
            validate(bad, request)
        except AssertionError:
            rejected += 1
    assert rejected == 5
    for key, value in [('delta_seconds', '0'), ('velocity_cm_s', {'x': True, 'y': 0, 'z': 0})]:
        bad = copy.deepcopy(request)
        actual = copy.deepcopy(document)
        if key == 'velocity_cm_s':
            actual['character_braking_probe']['samples'][0]['before_velocity_cm_s'] = {'x': 1, 'y': 0, 'z': 0}
        bad['samples'][0][key] = value
        try:
            validate(actual, bad)
        except AssertionError:
            rejected += 1
    assert rejected == 7
    return {'self_test': 'PASS', 'invalid_native_packets_rejected': rejected}


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
