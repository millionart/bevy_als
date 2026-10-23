import argparse
import copy
import json
from pathlib import Path


KEYS = ['PoseStanding', 'PoseGrounded', 'PoseInAir', 'FootLeftIk', 'FootRightIk', 'FootLeftLock', 'FootRightLock']


def validate(source, baseline, control):
    assert len(baseline) == len(control) > 0
    for index, (left, right) in enumerate(zip(baseline, control)):
        for key in ['actor_position', 'actor_velocity', 'graph_frame_before']:
            assert left['before'][key] == right['before'][key]
        assert left['graph_frame_after'] == right['graph_frame_after']
        assert left['pending_after'] == right['pending_after'] == False
        assert left['rule_inputs_after']['root_time'] == right['rule_inputs_after']['root_time']
        if index:
            assert left['rule_inputs_after']['LocomotionMode'] == right['rule_inputs_after']['LocomotionMode']
    assert baseline[0]['rule_inputs_after']['LocomotionMode'] == 'Als.LocomotionMode.InAir'
    assert control[0]['rule_inputs_after']['LocomotionMode'] == 'Als.LocomotionMode.Grounded'
    expected = source['curves']
    base = baseline[0]['root_pose']['curves']
    restored = control[0]['root_pose']['curves']
    failures = [name for name in KEYS if base.get(name, 0) != expected.get(name, 0)]
    assert failures
    assert all(restored.get(name, 0) == expected.get(name, 0) for name in KEYS)
    return {'frames': len(baseline), 'baseline_seed_failures': failures, 'controlled_seed_failures': 0, 'pending_and_physics_unchanged': True}


def self_test():
    before = {'actor_position': [0, 1.5, 0], 'actor_velocity': [0, -.3, 0], 'graph_frame_before': 0}
    source = {'curves': {'PoseStanding': 1, 'PoseGrounded': 1, 'FootLeftIk': 1, 'FootRightIk': 1, 'FootLeftLock': 1, 'FootRightLock': 1}}
    row = {'before': before, 'graph_frame_after': 1, 'pending_after': False, 'rule_inputs_after': {'root_time': 'Absolute(0)', 'LocomotionMode': 'Als.LocomotionMode.InAir'}, 'root_pose': {'curves': {'PoseStanding': 1, 'PoseInAir': 1}}}
    control = copy.deepcopy(row)
    control['rule_inputs_after']['LocomotionMode'] = 'Als.LocomotionMode.Grounded'
    control['root_pose']['curves'] = dict(source['curves'])
    assert validate(source, [row], [control])['controlled_seed_failures'] == 0
    rejected = 0
    for index in range(4):
        changed = copy.deepcopy(control)
        if index == 0:
            changed['root_pose']['curves']['FootLeftIk'] = 0
        elif index == 1:
            changed['before']['actor_position'][1] = 2
        elif index == 2:
            changed['pending_after'] = True
        else:
            changed['rule_inputs_after']['root_time'] = 'Delta(.03)'
        try:
            validate(source, [row], [changed])
        except (AssertionError, KeyError):
            rejected += 1
    assert rejected == 4
    return {'self_test': 'PASS', 'invalid_cases_rejected': rejected}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--source', type=Path)
    parser.add_argument('--baseline', type=Path)
    parser.add_argument('--control', type=Path)
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    if args.self_test:
        result = self_test()
    else:
        assert args.source and args.baseline and args.control
        document = json.loads(args.source.read_text())['character_animation_probe']['cases'][0]['initialization']
        result = validate(document, json.loads(args.baseline.read_text()), json.loads(args.control.read_text()))
    print(json.dumps(result))
