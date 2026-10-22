import argparse
import copy
import json
import math
from pathlib import Path


def validate(document):
    hierarchy = document['rig_cdo_hierarchy']
    bones = hierarchy['bones']
    elements = hierarchy['elements']
    assert bones and hierarchy['element_count'] == len(elements)
    names = set()
    for bone in bones:
        assert bone['name'] not in names
        assert bone['parent'] == 'None' or bone['parent'] in names
        names.add(bone['name'])
        for key in ['initial_local_transform', 'initial_global_transform']:
            for channel, axes in [('translation', 'xyz'), ('rotation', 'xyzw'), ('scale', 'xyz')]:
                values = bone[key][channel]
                assert set(values) == set(axes)
                assert all(type(values[axis]) in [int, float] and math.isfinite(values[axis]) for axis in axes)
    assert names == {element['name'] for element in elements if element['type'] == 'Bone'}
    return {'bones': len(bones), 'elements': len(elements)}


def self_test():
    transform = {'translation': dict.fromkeys('xyz', 0), 'rotation': dict(zip('xyzw', [0, 0, 0, 1])), 'scale': dict.fromkeys('xyz', 1)}
    fixture = {'rig_cdo_hierarchy': {'bones': [{'name': 'root', 'parent': 'None', 'initial_local_transform': transform, 'initial_global_transform': transform}], 'elements': [{'name': 'root', 'type': 'Bone'}], 'element_count': 1}}
    assert validate(fixture) == {'bones': 1, 'elements': 1}
    rejected = []
    for index in range(6):
        case = copy.deepcopy(fixture)
        hierarchy = case['rig_cdo_hierarchy']
        if index == 0:
            case.pop('rig_cdo_hierarchy')
        elif index == 1:
            hierarchy['element_count'] = 2
        elif index == 2:
            hierarchy['bones'].append(copy.deepcopy(hierarchy['bones'][0]))
        elif index == 3:
            hierarchy['bones'][0]['parent'] = 'missing'
        elif index == 4:
            hierarchy['bones'][0]['initial_local_transform']['rotation']['w'] = float('nan')
        else:
            hierarchy['bones'][0]['initial_global_transform'].pop('scale')
        try:
            validate(case)
        except (AssertionError, KeyError):
            rejected.append(index)
    assert len(rejected) == 6
    return {'self_test': 'PASS', 'invalid_cases_rejected': 6}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--input', type=Path)
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    print(json.dumps(self_test() if args.self_test else validate(json.loads(args.input.read_text()))))
