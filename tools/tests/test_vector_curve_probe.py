import argparse
import copy
import json
import math
import struct
from pathlib import Path


def validate(document, request):
    rows = document['blend_profile_probe']['samples']
    assert len(rows) == len(request['samples']) > 0
    for row, sample in zip(rows, request['samples']):
        axis = sample['custom_curve_axis']
        assert type(axis) in [int, float] and math.isfinite(axis) and 0 <= axis <= 2 and axis == int(axis)
        assert sample['mode'] == 'Linear'
        assert row['name'] == sample['name'] and row['custom_curve_axis'] == axis
        assert row['custom_curve_class'] == '/Script/Engine.CurveVector'
        assert row['custom_curve_applied_to_blend'] is False
        for key in ['custom_curve_pre_extrap', 'custom_curve_post_extrap']:
            assert type(row[key]) is int
        keys = row['custom_curve_keys']
        assert len(keys) > 0
        for key in keys:
            for name in ['time', 'value', 'arrive_tangent', 'leave_tangent', 'arrive_weight', 'leave_weight']:
                assert type(key[name]) in [int, float] and math.isfinite(key[name])
            for name in ['interp_mode', 'tangent_mode', 'weight_mode']:
                assert type(key[name]) is int
        samples = row['custom_curve_time_samples']
        assert len(samples) == len(sample['custom_curve_times']) > 0
        for actual, time in zip(samples, sample['custom_curve_times']):
            assert actual['time'] == struct.unpack('f', struct.pack('f', time))[0]
            assert type(actual['value']) in [int, float] and math.isfinite(actual['value'])
    return {'axes': len(rows), 'exact_numeric_keys_modes_and_native_times': True}


def self_test():
    request = {'samples': [{'name': 'x', 'mode': 'Linear', 'custom_curve_axis': 0, 'custom_curve_times': [0]}]}
    row = {'name': 'x', 'custom_curve_axis': 0, 'custom_curve_class': '/Script/Engine.CurveVector', 'custom_curve_applied_to_blend': False, 'custom_curve_pre_extrap': 0, 'custom_curve_post_extrap': 0, 'custom_curve_keys': [{'time': 0, 'value': 20, 'arrive_tangent': 0, 'leave_tangent': 0, 'arrive_weight': 0, 'leave_weight': 0, 'interp_mode': 0, 'tangent_mode': 0, 'weight_mode': 0}], 'custom_curve_time_samples': [{'time': 0, 'value': 20}]}
    document = {'blend_profile_probe': {'samples': [row]}}
    assert validate(document, request)['axes'] == 1
    request['samples'][0]['custom_curve_axis'] = 0.0
    assert validate(document, request)['axes'] == 1
    rejected = 0
    for index in range(7):
        bad = copy.deepcopy(document)
        wanted = copy.deepcopy(request)
        actual = bad['blend_profile_probe']['samples'][0]
        if index == 0:
            wanted['samples'][0]['custom_curve_axis'] = True
        elif index == 1:
            wanted['samples'][0]['custom_curve_axis'] = 3
        elif index == 2:
            actual['custom_curve_axis'] = 1
        elif index == 3:
            actual['custom_curve_applied_to_blend'] = True
        elif index == 4:
            del actual['custom_curve_keys'][0]['interp_mode']
        elif index == 5:
            actual['custom_curve_time_samples'][0]['value'] = float('nan')
        else:
            wanted['samples'][0]['mode'] = 'Custom'
        try:
            validate(bad, wanted)
        except (KeyError, AssertionError):
            rejected += 1
    assert rejected == 7
    return {'self_test': 'PASS', 'invalid_vector_axis_contracts_rejected': rejected}


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
