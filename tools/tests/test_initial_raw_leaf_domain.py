import argparse
import json
import math
from pathlib import Path


def validate(source, target, time):
    probe = source['sequence_pose_probe']
    assert probe['sampling_mode'] == 'raw_absolute_pose'
    assert len(probe['frames']) == 1
    frame = probe['frames'][0]
    assert frame['pose_evaluation_mode'] == 'raw_model'
    assert frame['time'] == target['timestamp'] == time
    bones = frame['bones']
    assert bones and len({bone['name'] for bone in bones}) == len(bones)
    actual = {bone['name']: bone for bone in target['bones']}
    assert len(actual) == len(target['bones'])
    for bone in bones:
        current = actual[bone['name']]
        assert current['present']
        for key, count in [('translation', 3), ('rotation', 4), ('scale', 3)]:
            values = current[key]
            assert isinstance(values, list) and len(values) == count
            assert all(math.isfinite(value) for value in values)
    return {'raw_absolute_bones': len(bones), 'same_time': time, 'sparse_additive_cache_rejected': True}


def self_test():
    source = {'sequence_pose_probe': {'sampling_mode': 'raw_absolute_pose', 'frames': [{'pose_evaluation_mode': 'raw_model', 'time': 0, 'bones': [{'name': 'root'}]}]}}
    target = {'timestamp': 0, 'bones': [{'name': 'root', 'present': True, 'translation': [0, 0, 0], 'rotation': [0, 0, 0, 1], 'scale': [1, 1, 1]}]}
    assert validate(source, target, 0)['raw_absolute_bones'] == 1
    rejected = 0
    for index in range(4):
        changed = json.loads(json.dumps(target))
        if index == 0:
            changed['timestamp'] = .5
        elif index == 1:
            changed['bones'][0]['scale'] = None
        elif index == 2:
            changed['bones'][0]['present'] = False
        else:
            changed['bones'][0]['rotation'][0] = float('nan')
        try:
            validate(source, changed, 0)
        except (KeyError, AssertionError):
            rejected += 1
    assert rejected == 4
    return {'self_test': 'PASS', 'invalid_absolute_domains_rejected': rejected}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--self-test', action='store_true', required=True)
    parser.parse_args()
    print(json.dumps(self_test()))
