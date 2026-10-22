import argparse
import copy
import json
from pathlib import Path


def validate(document):
    rows = document['rig_lifecycle_probe']['samples']
    assert rows
    for row in rows:
        assert row['initialization_events'] == 1
        assert row['construction_events_before_forward'] == row['construction_events']
        assert row['construction_events'] > 0
        assert row['forward_events'] >= 0
        executions = row['executions']
        assert executions and all(event['succeeded'] for event in executions)
        forward = [event for event in executions if event['entry'] == 'Forwards Solve']
        assert len(forward) == row['forward_events']
        assert all(event['instructions'] for event in executions)
        before = row['construction_pose']['bones']
        after = row['final_pose']['bones']
        assert before and [bone['name'] for bone in before] == [bone['name'] for bone in after]
        assert row['construction_variables'] and row['final_variables']
    return {'samples': len(rows), 'native_executions_valid': True}


def self_test():
    row = {'initialization_events': 1, 'construction_events_before_forward': 2, 'construction_events': 2, 'forward_events': 1, 'executions': [{'entry': 'Construction', 'succeeded': True, 'instructions': [150]}, {'entry': 'Forwards Solve', 'succeeded': True, 'instructions': [0]}], 'construction_pose': {'bones': [{'name': 'root'}]}, 'final_pose': {'bones': [{'name': 'root'}]}, 'construction_variables': {'FootHeight': '1'}, 'final_variables': {'FootHeight': '1'}}
    fixture = {'rig_lifecycle_probe': {'samples': [row]}}
    assert validate(fixture)['samples'] == 1
    rejected = []
    for index in range(6):
        case = copy.deepcopy(fixture)
        value = case['rig_lifecycle_probe']['samples'][0]
        if index == 0:
            value.pop('executions')
        elif index == 1:
            value['executions'][1]['succeeded'] = False
        elif index == 2:
            value['initialization_events'] = 2
        elif index == 3:
            value['forward_events'] = 2
        elif index == 4:
            value['executions'][1]['instructions'] = []
        else:
            value['final_pose']['bones'] = []
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
