import argparse
import copy
import json
import math
from pathlib import Path


def check(document, request):
    actual = document['geometry_query_probe']['samples']
    assert len(actual) == len(request['samples'])
    for row, sample in zip(actual, request['samples']):
        assert row['input'] == sample
        if 'float_range_map' in sample:
            assert math.isfinite(row['float_range_mapped'])


def self_test():
    request = {'samples': [{'rotation_xyzw': [0, 0, 0, 1], 'float_range_map': {'input': 5, 'input_range': [0, 10], 'output_range': [0, 1]}}]}
    document = {'geometry_query_probe': {'samples': [{'input': request['samples'][0], 'float_range_mapped': .5}]}}
    check(document, request)
    rejected = []
    for name in ['missing_value', 'nonfinite_value', 'missing_sample']:
        changed = copy.deepcopy(document)
        if name == 'missing_value':
            del changed['geometry_query_probe']['samples'][0]['float_range_mapped']
        elif name == 'nonfinite_value':
            changed['geometry_query_probe']['samples'][0]['float_range_mapped'] = math.inf
        else:
            changed['geometry_query_probe']['samples'] = []
        try:
            check(changed, request)
        except (AssertionError, KeyError):
            rejected.append(name)
    assert len(rejected) == 3
    return {'self_test': 'PASS', 'rejected_controls': rejected}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--self-test', action='store_true')
    parser.add_argument('--input', type=Path)
    parser.add_argument('--request', type=Path)
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(self_test()))
    else:
        assert args.input and args.request
        check(json.loads(args.input.read_text()), json.loads(args.request.read_text()))
        print(json.dumps({'float_range_map_probe': 'PASS'}))


if __name__ == '__main__':
    main()
