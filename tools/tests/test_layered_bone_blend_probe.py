import argparse
import copy
import json
import math
from pathlib import Path


def check(document, request):
    data = document['layered_bone_blend_probe']
    assert data['node_guid'] == request['node_guid'] and data['node_property']
    assert len(data['samples']) == len(request['samples'])
    for actual, wanted in zip(data['samples'], request['samples']):
        assert actual['name'] == wanted['name']
        assert len(actual['bone_transforms']) == len(request['bone_names'])
        assert len(actual['bone_weights']) == len(request['bone_names'])
        assert len(actual['update_weights']) == len(wanted['layers']) + 1
        assert len(actual['root_motion_weight_modifiers']) == len(wanted['layers']) + 1
        assert isinstance(actual['curve_sources'], dict) and isinstance(actual['curves'], dict)
        assert all(math.isfinite(value) for value in actual['curves'].values())
        for transform, weight in zip(actual['bone_transforms'], actual['bone_weights']):
            assert set(transform) == {'translation', 'rotation', 'scale'}
            values = [transform[k][axis] for k, axes in [('translation', 'xyz'), ('rotation', 'xyzw'), ('scale', 'xyz')] for axis in axes]
            assert all(math.isfinite(value) for value in values)
            assert abs(sum(value * value for value in values[3:7]) - 1) < 0.01
            assert 0 <= weight['source'] < len(wanted['layers']) and 0 <= weight['weight'] <= 1


def self_test():
    transform = {'translation': dict.fromkeys('xyz', 0), 'rotation': dict(zip('xyzw', [0, 0, 0, 1])), 'scale': dict.fromkeys('xyz', 1)}
    request = {'node_guid': 'probe', 'bone_names': ['root'], 'samples': [{'name': 'case', 'layers': [{}]}]}
    document = {'layered_bone_blend_probe': {'node_guid': 'probe', 'node_property': 'property', 'samples': [{'name': 'case', 'bone_transforms': [transform], 'bone_weights': [{'source': 0, 'weight': 1}], 'update_weights': [1, 1], 'root_motion_weight_modifiers': [1, 0], 'curve_sources': {}, 'curves': {}}]}}
    check(document, request)
    failures = []
    for mode in ['missing_weights', 'invalid_rotation', 'nonfinite_curve']:
        changed = copy.deepcopy(document)
        row = changed['layered_bone_blend_probe']['samples'][0]
        if mode == 'missing_weights':
            row['bone_weights'] = []
        elif mode == 'invalid_rotation':
            row['bone_transforms'][0]['rotation'] = dict.fromkeys('xyzw', 0)
        else:
            row['curves']['curve'] = math.inf
        try:
            check(changed, request)
        except AssertionError:
            failures.append(mode)
    assert len(failures) == 3
    return {'self_test': 'PASS', 'rejected_controls': failures}


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
        print(json.dumps({'layered_bone_blend_probe': 'PASS'}))


if __name__ == '__main__':
    main()
