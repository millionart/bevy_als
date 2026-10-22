import argparse
import copy
import json
import math
import struct
from pathlib import Path


def validate(document):
    counts = dict.fromkeys(['euler', 'product', 'distance'], 0)
    for row in document['geometry_query_probe']['samples']:
        request = row['input']
        for field, trigger, category in [('rotation_from_euler', 'euler_degrees', 'euler'), ('normalized_rotation_product', 'rotation_factors', 'product')]:
            if trigger in request:
                value = row[field]
                assert set(value) == set('xyzw')
                assert all(math.isfinite(number) for number in value.values())
                assert abs(sum(number * number for number in value.values()) - 1) < 1e-8
                counts[category] += 1
        if 'scaled_distance' in request:
            value = row['scaled_distance']
            assert math.isfinite(value) and value >= 0
            assert row['scaled_distance_float'] == struct.unpack('f', struct.pack('f', value))[0]
            counts['distance'] += 1
    return counts


def self_test():
    fixture = {'geometry_query_probe': {'samples': [{'input': {'euler_degrees': [0, 0, 0], 'rotation_factors': [], 'scaled_distance': {}}, 'rotation_from_euler': dict(zip('xyzw', [0, 0, 0, 1])), 'normalized_rotation_product': dict(zip('xyzw', [0, 0, 0, 1])), 'scaled_distance': 5.0, 'scaled_distance_float': 5.0}]}}
    assert validate(fixture) == {'euler': 1, 'product': 1, 'distance': 1}
    rejected = []
    for index in range(5):
        case = copy.deepcopy(fixture)
        row = case['geometry_query_probe']['samples'][0]
        if index < 3:
            row.pop(['rotation_from_euler', 'normalized_rotation_product', 'scaled_distance_float'][index])
        elif index == 3:
            row['rotation_from_euler']['w'] = float('inf')
        else:
            row['scaled_distance_float'] = 4.0
        try:
            validate(case)
        except (KeyError, AssertionError):
            rejected.append(index)
    assert len(rejected) == 5
    return {'self_test': 'PASS', 'invalid_output_cases_rejected': 5}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument('--input', type=Path)
    group.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    print(json.dumps(self_test() if args.self_test else validate(json.loads(args.input.read_text()))))
