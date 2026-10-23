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


def validate_head_inputs(document):
    result = validate(document)
    for case in document['character_animation_probe']['cases']:
        for entry in [case['initialization']] + case['frames']:
            boundaries = entry['pose_boundaries']
            assert math.isfinite(boundaries['head_alpha']) and 0 <= boundaries['head_alpha'] <= 1
            assert isinstance(boundaries['head_root_space_additive'], bool)
            assert boundaries['head_instance_class'] and boundaries['head_property']
            source = boundaries['rig_source']
            names = [bone['name'] for bone in source['pose']['bones']]
            for key in ['head_base', 'head_additive']:
                value = boundaries[key]
                assert value['evaluations'] == source['evaluations'] and value['engine_frame'] == source['engine_frame']
                assert value['required_bones_asset'] == source['required_bones_asset'] and value['attribute_count'] >= 0
                pose = value['pose']
                assert [bone['name'] for bone in pose['bones']] == names
                assert all(math.isfinite(v) for v in pose['curves'].values())
                assert all(math.isfinite(v) for bone in pose['bones'] for field in bone['local_transform'].values() for v in field.values())
    return {**result, 'actual_head_inputs_and_parameters': True}


def self_test(head_inputs=False):
    transform = {'translation': dict.fromkeys('xyz', 0), 'rotation': dict(zip('xyzw', [0,0,0,1])), 'scale': dict.fromkeys('xyz', 1)}
    pose = {'bones': [{'name': 'root', 'local_transform': transform}], 'curves': {}}
    capture = {'evaluations': 1, 'updates': 1, 'delta_seconds': 0, 'engine_frame': 1, 'attribute_count': 0, 'required_bones_asset': '/Fixture/Mesh.Mesh', 'pose': pose}
    boundaries = {'stage': 'actual_rig_source_and_main_root_result', 'rig_source': capture, 'root_result': copy.deepcopy(capture)}
    if head_inputs:
        boundaries.update(head_base=copy.deepcopy(capture), head_additive=copy.deepcopy(capture), head_alpha=1, head_root_space_additive=False, head_instance_class='/Fixture/Instance.Instance_C', head_property='ApplyMesh')
    initial = {'evaluation_counter': 0, 'mesh_local_pose': [{'name': 'root'}], 'pose_boundaries': boundaries}
    fixture = {'character_animation_probe': {'cases': [{'initial_engine_frame': 1, 'initialization': initial, 'frames': [{'evaluation_counter': 0, 'pose_boundaries': copy.deepcopy(boundaries)}]}]}}
    check = validate_head_inputs if head_inputs else validate
    assert check(fixture)['actual_main_root_evaluations'] == 1
    rejected = 0
    count = 8 if head_inputs else 5
    for index in range(count):
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
        elif index == 4:
            value['root_result']['pose']['bones'] = []
        elif index == 5:
            value.pop('head_base')
        elif index == 6:
            value['head_alpha'] = math.nan
        else:
            value['head_additive']['pose']['bones'] = []
        try:
            check(data)
        except (AssertionError, KeyError):
            rejected += 1
    assert rejected == count
    return {'self_test': 'PASS', 'invalid_head_input_boundaries_rejected' if head_inputs else 'invalid_pose_boundaries_rejected': rejected}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument('--input', type=Path)
    group.add_argument('--self-test', action='store_true')
    parser.add_argument('--head-inputs', action='store_true')
    args = parser.parse_args()
    print(json.dumps(self_test(args.head_inputs) if args.self_test else (validate_head_inputs if args.head_inputs else validate)(json.loads(args.input.read_text()))))
