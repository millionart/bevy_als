import argparse
import copy
import json
import math
from pathlib import Path


def check(document, request):
    data = document['mesh_additive_probe']
    assert data['node_guid'] == request['node_guid'] and data['node_property']
    assert len(data['samples']) == len(request['samples'])
    for actual, wanted in zip(data['samples'], request['samples']):
        assert actual['name'] == wanted['name'] and actual['alpha'] == wanted['alpha']
        assert isinstance(actual['root_space_additive'], bool)
        pose = actual['pose']
        assert [b['name'] for b in pose['bones']] == request['bone_names']
        assert all(math.isfinite(v) for v in pose['curves'].values())
        for bone in pose['bones']:
            transform = bone['local_transform']
            values = [transform[k][a] for k, axes in [('translation', 'xyz'), ('rotation', 'xyzw'), ('scale', 'xyz')] for a in axes]
            assert all(math.isfinite(v) for v in values)
            assert abs(sum(v * v for v in values[3:7]) - 1) < .01


def self_test():
    transform = {'translation': dict.fromkeys('xyz', 0), 'rotation': dict(zip('xyzw', [0, 0, 0, 1])), 'scale': dict.fromkeys('xyz', 1)}
    request = {'node_guid': 'node', 'bone_names': ['root'], 'samples': [{'name': 'sample', 'alpha': 1}]}
    document = {'mesh_additive_probe': {'node_guid': 'node', 'node_property': 'property', 'samples': [{'name': 'sample', 'alpha': 1, 'root_space_additive': False, 'pose': {'bones': [{'name': 'root', 'local_transform': transform}], 'curves': {}}}]}}
    check(document, request)
    rejected = []
    for mode in ['wrong_node', 'missing_bone', 'invalid_rotation', 'nonfinite_curve']:
        changed = copy.deepcopy(document)
        row = changed['mesh_additive_probe']['samples'][0]
        if mode == 'wrong_node':
            changed['mesh_additive_probe']['node_guid'] = 'wrong'
        elif mode == 'missing_bone':
            row['pose']['bones'] = []
        elif mode == 'invalid_rotation':
            row['pose']['bones'][0]['local_transform']['rotation'] = dict.fromkeys('xyzw', 0)
        else:
            row['pose']['curves']['curve'] = math.inf
        try:
            check(changed, request)
        except AssertionError:
            rejected.append(mode)
    assert len(rejected) == 4
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
        print(json.dumps({'mesh_additive_probe': 'PASS'}))


if __name__ == '__main__':
    main()
