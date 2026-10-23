import argparse
import copy
import json
from pathlib import Path


def validate(document):
    cases = document['character_animation_probe']['cases']
    assert cases
    for case in cases:
        initial = case['initialization']
        assert type(case['initial_engine_frame']) in (int, float) and case['initial_engine_frame'] >= 0
        mesh = initial['mesh_local_pose']
        rig = initial['rig_local_pose']
        assert mesh and len(mesh) == len(rig)
        assert {bone['name'] for bone in mesh} == {bone['name'] for bone in rig}
        assert len({bone['name'] for bone in rig}) == len(rig)
        assert all('local' in bone for bone in rig)
        assert 'RigInput' in initial['rig_variables']
    return {'cases': len(cases), 'mesh_and_evaluated_rig_boundaries_distinct': True}


def self_test():
    fixture = {'character_animation_probe': {'cases': [{'initial_engine_frame': 1, 'initialization': {'mesh_local_pose': [{'name': 'root', 'local': {}}], 'rig_local_pose': [{'name': 'root', 'local': {}}], 'rig_variables': {'RigInput': {}}}}]}}
    assert validate(fixture)['cases'] == 1
    rejected = 0
    for index in range(4):
        changed = copy.deepcopy(fixture)
        row = changed['character_animation_probe']['cases'][0]
        if index == 0:
            del row['initialization']['rig_local_pose']
        elif index == 1:
            row['initialization']['rig_local_pose'][0]['name'] = 'wrong'
        elif index == 2:
            row['initialization']['rig_variables'] = {}
        else:
            row['initial_engine_frame'] = -1
        try:
            validate(changed)
        except (KeyError, AssertionError):
            rejected += 1
    assert rejected == 4
    return {'self_test': 'PASS', 'invalid_cases_rejected': rejected}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument('--input', type=Path)
    group.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    print(json.dumps(self_test() if args.self_test else validate(json.loads(args.input.read_text()))))
