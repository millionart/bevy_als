import argparse
import copy
import json
import math
from pathlib import Path


def check(document, request):
    probe = document['slot_probe']
    assert probe['node_guid'] == request['node_guid'] and probe['node_property'] and probe['slot_name']
    assert len(probe['frames']) == len(request['steps'])
    names = request['bone_names']
    for index, (frame, step) in enumerate(zip(probe['frames'], request['steps'])):
        assert frame['frame'] == index
        assert [bone['name'] for bone in frame['bones']] == names
        for bone in frame['bones']:
            transform = bone['local_transform']
            values = [transform[field][axis] for field, axes in [('translation', 'xyz'), ('rotation', 'xyzw'), ('scale', 'xyz')] for axis in axes]
            assert all(math.isfinite(value) for value in values)
            assert abs(sum(value * value for value in values[3:7]) - 1) < .01
        assert all(math.isfinite(value) for value in frame['curves'].values())
        for field in ['source_update_weight', 'slot_weight', 'source_weight', 'total_weight', 'montage_weight', 'montage_position']:
            assert math.isfinite(frame[field])
        assert 0 <= frame['slot_weight'] <= 1 and 0 <= frame['source_weight'] <= 1
        if index < request['dynamic_montage']['play_frame']:
            assert frame['slot_weight'] == frame['montage_weight'] == frame['total_weight'] == 0
            assert frame['source_weight'] == frame['source_update_weight'] == 1
            assert frame['curves'] == step['curves']
            for actual, wanted in zip(frame['bones'], step['bone_transforms']):
                transform = actual['local_transform']
                for field, source in [('translation', 'translation_cm'), ('rotation', 'rotation'), ('scale', 'scale')]:
                    assert list(transform[field].values()) == wanted[source]


def self_test():
    transform = {'translation_cm': [0, 0, 0], 'rotation': [0, 0, 0, 1], 'scale': [1, 1, 1]}
    pose = {'translation': dict.fromkeys('xyz', 0), 'rotation': dict(zip('xyzw', transform['rotation'])), 'scale': dict.fromkeys('xyz', 1)}
    request = {'node_guid': 'probe', 'bone_names': ['root'], 'dynamic_montage': {'play_frame': 1}, 'steps': [{'bone_transforms': [transform], 'curves': {}}, {}]}
    frame = {'frame': 0, 'bones': [{'name': 'root', 'local_transform': pose}], 'curves': {}, 'source_update_weight': 1, 'slot_weight': 0, 'source_weight': 1, 'total_weight': 0, 'montage_weight': 0, 'montage_position': 0}
    active = copy.deepcopy(frame)
    active.update(frame=1, slot_weight=.5, total_weight=.5, montage_weight=.5, montage_position=.1)
    document = {'slot_probe': {'node_guid': 'probe', 'node_property': 'property', 'slot_name': 'slot', 'frames': [frame, active]}}
    check(document, request)
    rejected = []
    for name in ['missing_bone', 'invalid_rotation', 'missing_clock', 'nonfinite_curve']:
        changed = copy.deepcopy(document)
        row = changed['slot_probe']['frames'][1]
        if name == 'missing_bone':
            row['bones'] = []
        elif name == 'invalid_rotation':
            row['bones'][0]['local_transform']['rotation'] = dict.fromkeys('xyzw', 0)
        elif name == 'missing_clock':
            del row['montage_position']
        else:
            row['curves']['value'] = math.inf
        try:
            check(changed, request)
        except (AssertionError, KeyError):
            rejected.append(name)
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
        print(json.dumps({'slot_probe': 'PASS'}))


if __name__ == '__main__':
    main()
