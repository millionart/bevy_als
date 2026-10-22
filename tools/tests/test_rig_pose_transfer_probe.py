import argparse
import copy
import json
from pathlib import Path


def validate(document):
    rows = document['rig_pose_transfer_probe']['samples']
    assert rows
    for row in rows:
        assert row['adapter_enabled'] and row['adapter_ready']
        assert row['compact_bones'] > 0
        before = row['before']['bones']
        after = row['after']['bones']
        transferred = row['transferred']
        assert len(before) == len(after) == len(transferred) == row['compact_bones']
        assert [bone['name'] for bone in before] == [bone['name'] for bone in after]
        assert {bone['name'] for bone in transferred} == {bone['name'] for bone in before}
        assert row['transfer_flags'] == {'input_pose': True, 'input_curves': True, 'global_space': False, 'reset_pose': False}
        assert len({curve['name'] for curve in row['hierarchy_curves']}) == len(row['hierarchy_curves'])
    return {'samples': len(rows), 'default_local_adapter_valid': True}


def self_test():
    row = {'adapter_enabled': True, 'adapter_ready': True, 'compact_bones': 1, 'before': {'bones': [{'name': 'root'}]}, 'after': {'bones': [{'name': 'root'}]}, 'transferred': [{'name': 'root'}], 'transfer_flags': {'input_pose': True, 'input_curves': True, 'global_space': False, 'reset_pose': False}, 'hierarchy_curves': [{'name': 'curve'}]}
    fixture = {'rig_pose_transfer_probe': {'samples': [row]}}
    assert validate(fixture)['samples'] == 1
    rejected = []
    for index in range(6):
        case = copy.deepcopy(fixture)
        value = case['rig_pose_transfer_probe']['samples'][0]
        if index == 0:
            value['adapter_enabled'] = False
        elif index == 1:
            value['adapter_ready'] = False
        elif index == 2:
            value['transferred'] = []
        elif index == 3:
            value['after']['bones'][0]['name'] = 'wrong'
        elif index == 4:
            value['transfer_flags']['global_space'] = True
        else:
            value['hierarchy_curves'].append({'name': 'curve'})
        try:
            validate(case)
        except (KeyError, AssertionError):
            rejected.append(index)
    assert len(rejected) == 6
    return {'self_test': 'PASS', 'invalid_cases_rejected': 6}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument('--input', type=Path)
    group.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    print(json.dumps(self_test() if args.self_test else validate(json.loads(args.input.read_text()))))
