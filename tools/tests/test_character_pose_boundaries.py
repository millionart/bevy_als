import argparse
import copy
import json
import math
from pathlib import Path


def validate(document):
    cases = document['character_animation_probe']['cases']
    assert cases
    evaluations = 0
    for case in cases:
        initial = case['initialization']
        boundaries = initial['pose_boundaries']
        assert boundaries['stage'] == 'actual_rig_source_and_main_root_result'
        names = [bone['name'] for bone in boundaries['rig_source']['pose']['bones']]
        assert names and len(names) == len(set(names))
        assert set(names) == {bone['name'] for bone in initial['mesh_local_pose']}
        previous = 0
        entries = [(initial['evaluation_counter'], boundaries)] + [(frame['evaluation_counter'], frame['pose_boundaries']) for frame in case['frames']]
        for index, (counter, entry) in enumerate(entries):
            assert entry['stage'] == boundaries['stage']
            for key in ['rig_source', 'root_result']:
                value = entry[key]
                assert value['evaluations'] == counter + 1 and counter >= previous
                assert value['updates'] > 0 and 0 <= value['delta_seconds'] <= .25
                assert value['attribute_count'] >= 0
                assert value['required_bones_asset'] == boundaries['rig_source']['required_bones_asset']
                assert value['required_bones_asset']
                if index == 0:
                    assert value['evaluations'] == 1 and value['engine_frame'] == case['initial_engine_frame'] and value['delta_seconds'] == 0
                pose = value['pose']
                assert [bone['name'] for bone in pose['bones']] == names
                assert all(math.isfinite(v) for v in pose['curves'].values())
                for bone in pose['bones']:
                    transform = bone['local_transform']
                    assert all(math.isfinite(v) for field in transform.values() for v in field.values())
                    assert abs(sum(v*v for v in transform['rotation'].values())-1) < 1e-5
            assert entry['rig_source']['engine_frame'] == entry['root_result']['engine_frame']
            previous = counter
        evaluations += previous + 1
    return {'cases': len(cases), 'actual_main_root_evaluations': evaluations, 'first_query_observed_without_added_evaluation': True}


def self_test():
    transform = {'translation': dict.fromkeys('xyz', 0), 'rotation': dict(zip('xyzw', [0,0,0,1])), 'scale': dict.fromkeys('xyz', 1)}
    pose = {'bones': [{'name': 'root', 'local_transform': transform}], 'curves': {}}
    capture = {'evaluations': 1, 'updates': 1, 'delta_seconds': 0, 'engine_frame': 1, 'attribute_count': 0, 'required_bones_asset': '/Fixture/Mesh.Mesh', 'pose': pose}
    boundaries = {'stage': 'actual_rig_source_and_main_root_result', 'rig_source': capture, 'root_result': copy.deepcopy(capture)}
    initial = {'evaluation_counter': 0, 'mesh_local_pose': [{'name': 'root'}], 'pose_boundaries': boundaries}
    fixture = {'character_animation_probe': {'cases': [{'initial_engine_frame': 1, 'initialization': initial, 'frames': [{'evaluation_counter': 0, 'pose_boundaries': copy.deepcopy(boundaries)}]}]}}
    assert validate(fixture)['actual_main_root_evaluations'] == 1
    rejected = 0
    for index in range(5):
        data = copy.deepcopy(fixture)
        value = data['character_animation_probe']['cases'][0]['initialization']['pose_boundaries']
        if index == 0:
            value.pop('root_result')
        elif index == 1:
            value['stage'] = 'hierarchy_after_solve'
        elif index == 2:
            value['rig_source']['evaluations'] = 2
        elif index == 3:
            value['root_result']['engine_frame'] = 2
        else:
            value['root_result']['pose']['bones'] = []
        try:
            validate(data)
        except (AssertionError, KeyError):
            rejected += 1
    assert rejected == 5
    return {'self_test': 'PASS', 'invalid_pose_boundaries_rejected': rejected}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument('--input', type=Path)
    group.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    print(json.dumps(self_test() if args.self_test else validate(json.loads(args.input.read_text()))))
