import argparse
import json
from pathlib import Path


def validate(document):
    samples = document['blend_list_probe']['samples']
    assert samples
    full = 0
    for sample in samples:
        assert sample['frames']
        for frame in sample['frames']:
            inputs = frame['input_poses']
            weights = frame['weights']
            output = frame['pose']
            assert len(inputs) == len(weights) > 0 and output['bones']
            assert isinstance(output['curves'], dict)
            active = [index for index, weight in enumerate(weights) if weight > 1e-5]
            if len(active) == 1 and weights[active[0]] == 1:
                assert output == inputs[active[0]]
                full += 1
    assert full
    return {'samples': len(samples), 'full_weight_passthrough_frames': full}


def self_test():
    pose = {'bones': [{'name': 'root'}], 'curves': {'PoseGrounded': 1}}
    fixture = {'blend_list_probe': {'samples': [{'frames': [{'weights': [1, 0], 'input_poses': [pose, pose], 'pose': pose}]}]}}
    assert validate(fixture)['full_weight_passthrough_frames'] == 1
    rejected = 0
    for index in range(4):
        changed = json.loads(json.dumps(fixture))
        frame = changed['blend_list_probe']['samples'][0]['frames'][0]
        if index == 0:
            frame.pop('pose')
        elif index == 1:
            frame['pose']['bones'][0]['name'] = 'wrong'
        elif index == 2:
            frame['pose']['curves'] = {}
        else:
            frame['input_poses'] = []
        try:
            validate(changed)
        except (KeyError, AssertionError):
            rejected += 1
    assert rejected == 4
    return {'self_test': 'PASS', 'invalid_pose_boundaries_rejected': rejected}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument('--input', type=Path)
    group.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    print(json.dumps(self_test() if args.self_test else validate(json.loads(args.input.read_text()))))
