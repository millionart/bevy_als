import argparse
import json
from pathlib import Path


def validate(document):
    probe = document['sequence_pose_probe']
    assert probe['sampling_mode'] == 'raw_additive_pose'
    assert probe['required_bones_raw'] is True
    frames = probe['frames']
    assert frames
    names = [bone['name'] for bone in frames[0]['bones']]
    assert names and len(set(names)) == len(names)
    for frame in frames:
        assert frame['pose_evaluation_mode'] == 'raw_model'
        assert [bone['name'] for bone in frame['bones']] == names
        assert 'source_stages' not in frame
    return {'frames': len(frames), 'bones': len(names), 'actual_required_bones_raw_flag': True}


def self_test():
    fixture = {'sequence_pose_probe': {'sampling_mode': 'raw_additive_pose', 'required_bones_raw': True, 'frames': [{'pose_evaluation_mode': 'raw_model', 'bones': [{'name': 'root'}]}]}}
    assert validate(fixture)['bones'] == 1
    rejected = 0
    for index in range(4):
        changed = json.loads(json.dumps(fixture))
        probe = changed['sequence_pose_probe']
        if index == 0:
            probe['sampling_mode'] = 'raw_absolute_pose'
        elif index == 1:
            probe['required_bones_raw'] = False
        elif index == 2:
            probe['frames'][0]['pose_evaluation_mode'] = 'compressed'
        else:
            probe['frames'][0]['source_stages'] = {}
        try:
            validate(changed)
        except AssertionError:
            rejected += 1
    assert rejected == 4
    return {'self_test': 'PASS', 'invalid_raw_additive_domains_rejected': rejected}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument('--input', type=Path)
    group.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    print(json.dumps(self_test() if args.self_test else validate(json.loads(args.input.read_text()))))
