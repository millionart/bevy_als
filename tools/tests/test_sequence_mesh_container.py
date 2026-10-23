import argparse
import copy
import json
from pathlib import Path


def validate(document, request):
    probe = document['sequence_pose_probe']
    assert isinstance(request['mesh_asset'], str) and request['mesh_asset']
    assert request['animation_pose'] is True
    assert probe['required_bones_mesh'] == request['mesh_asset']
    assert probe['sampling_mode'] in ['animation_pose', 'raw_absolute_pose', 'raw_additive_pose']
    frames = probe['frames']
    assert frames and [frame['time'] for frame in frames] == request['times']
    names = [bone['name'] for bone in frames[0]['bones']]
    assert names and len(names) == len(set(names))
    assert all([bone['name'] for bone in frame['bones']] == names for frame in frames)
    if request.get('raw_additive_pose'):
        assert probe['required_bones_raw'] is True
    return {'frames': len(frames), 'bones': len(names), 'actual_required_bones_mesh': probe['required_bones_mesh']}


def self_test():
    request = {'animation_pose': True, 'mesh_asset': '/Fixture/Mesh.Mesh', 'times': [0]}
    document = {'sequence_pose_probe': {'sampling_mode': 'raw_absolute_pose', 'required_bones_mesh': request['mesh_asset'], 'frames': [{'time': 0, 'bones': [{'name': 'root'}]}]}}
    assert validate(document, request)['bones'] == 1
    rejected = 0
    for index in range(4):
        changed = copy.deepcopy(document)
        probe = changed['sequence_pose_probe']
        if index == 0:
            probe.pop('required_bones_mesh')
        elif index == 1:
            probe['required_bones_mesh'] = '/Fixture/Other.Other'
        elif index == 2:
            probe['frames'][0]['time'] = .5
        else:
            probe['frames'][0]['bones'] = []
        try:
            validate(changed, request)
        except (AssertionError, KeyError):
            rejected += 1
    assert rejected == 4
    return {'self_test': 'PASS', 'invalid_mesh_containers_rejected': rejected}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--self-test', action='store_true')
    parser.add_argument('--input', type=Path)
    parser.add_argument('--request', type=Path)
    args = parser.parse_args()
    if args.self_test:
        result = self_test()
    else:
        assert args.input and args.request
        result = validate(json.loads(args.input.read_text()), json.loads(args.request.read_text()))
    print(json.dumps(result))
