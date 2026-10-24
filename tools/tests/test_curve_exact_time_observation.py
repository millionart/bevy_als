import argparse
import copy
import json
import math
import struct
from pathlib import Path


def validate(document, request):
    rows = document['blend_profile_probe']['samples']
    assert len(rows) == len(request['samples']) > 0
    points = 0
    keys = 0
    for row, sample in zip(rows, request['samples']):
        assert row['name'] == sample['name']
        times = sample['custom_curve_times']
        values = row['custom_curve_time_samples']
        assert len(times) == len(values) > 0
        for time, value in zip(times, values):
            assert value['time'] == struct.unpack('f', struct.pack('f', time))[0]
            assert type(value['value']) in [int, float] and math.isfinite(value['value'])
            points += 1
        assert row['custom_curve_keys']
        for key in row['custom_curve_keys']:
            for field in ['time', 'value', 'arrive_tangent', 'leave_tangent']:
                assert type(key[field]) in [int, float] and math.isfinite(key[field])
            keys += 1
    return {'exact_time_curve_samples': points, 'exact_float_curve_keys': keys}


def self_test():
    request = {'samples': [{'name': 'curve', 'custom_curve_times': [0, 150]}]}
    fixture = {'blend_profile_probe': {'samples': [{'name': 'curve', 'custom_curve_time_samples': [{'time': 0, 'value': 0.2}, {'time': 150, 'value': 1}], 'custom_curve_keys': [{'time': 0, 'value': 0.2, 'arrive_tangent': 0, 'leave_tangent': 0}]}]}}
    assert validate(fixture, request)['exact_time_curve_samples'] == 2
    rejected = 0
    for kind in ['missing_samples', 'wrong_time', 'missing_value', 'nonfinite', 'missing_key']:
        data = copy.deepcopy(fixture)
        row = data['blend_profile_probe']['samples'][0]
        if kind == 'missing_samples':
            row.pop('custom_curve_time_samples')
        elif kind == 'wrong_time':
            row['custom_curve_time_samples'][1]['time'] = 149
        elif kind == 'missing_value':
            row['custom_curve_time_samples'][0].pop('value')
        elif kind == 'nonfinite':
            row['custom_curve_time_samples'][0]['value'] = float('nan')
        else:
            row['custom_curve_keys'][0].pop('leave_tangent')
        try:
            validate(data, request)
        except (AssertionError, KeyError):
            rejected += 1
    assert rejected == 5
    return {'self_test': 'PASS', 'missing_or_inexact_curve_time_records_rejected': rejected}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--input', type=Path)
    parser.add_argument('--request', type=Path)
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    print(json.dumps(self_test() if args.self_test else validate(json.loads(args.input.read_text()), json.loads(args.request.read_text()))))
