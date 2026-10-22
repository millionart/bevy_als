import argparse
import copy
import json
import re
import struct
from pathlib import Path


def validate(document, request):
    cases = document['rig_node_history_probe']['cases']
    assert len(cases) == len(request['cases']) > 0
    assert len({case['name'] for case in cases}) == len(cases)
    frame_count = 0
    for case, wanted in zip(cases, request['cases']):
        assert case['name'] == wanted['name']
        assert case['game_world'] and case['bound_component']
        assert case['adapter_enabled'] and case['adapter_ready']
        assert case['external_controls_stage'] == 'after_update_before_evaluate'
        frames = case['frames']
        assert len(frames) == len(wanted['frames']) > 0
        assert len({frame['rig_id'] for frame in frames}) == 1
        construction = frames[0]['construction_events']
        for index, (frame, expected) in enumerate(zip(frames, wanted['frames'])):
            assert frame['name'] == expected['name']
            delta = struct.unpack('f', struct.pack('f', expected['delta_seconds']))[0]
            assert frame['delta_seconds_after_update'] == delta
            assert frame['leaf_update_weight'] == 1
            assert frame['forward_events'] == index + 1
            assert frame['construction_events'] == construction
            assert frame['attribute_count'] == 0
            assert [bone['name'] for bone in frame['before']['bones']] == request['bone_names']
            assert [bone['name'] for bone in frame['after']['bones']] == request['bone_names']
            assert frame['work_after']['properties']
            assert frame['externals_after']
            assert type(frame['reference_query_after_forward']['blocking_hit']) is bool
            forwards = [event for event in frame['executions'] if event['entry'] == 'Forwards Solve']
            assert len(forwards) == 1 and forwards[0]['succeeded'] and forwards[0]['instructions']
            assert all(event['succeeded'] for event in frame['executions'])
            frame_count += 1
    return {'cases': len(cases), 'frames': frame_count, 'same_instance_per_case': True, 'source_node_forward_per_frame': True, 'controlled_cold_replay_only': True}



def normalize(document):
    versions = {}
    def version(value):
        if str(value) == '-1':
            return '-1'
        return versions.setdefault(str(value), 'version_' + str(len(versions)))
    def walk(value):
        if isinstance(value, list):
            return [walk(item) for item in value]
        if not isinstance(value, dict):
            return value
        result = {}
        for key, item in value.items():
            if key == 'exported_text' and value.get('name') == 'ContainerVersion':
                result[key] = version(item)
            elif key == 'exported_text':
                result[key] = re.sub(r'ContainerVersion=(-?\d+)', lambda match: 'ContainerVersion=' + version(match[1]), item)
            else:
                result[key] = walk(item)
        return result
    return walk(document)


def self_test():
    frame = {'name': '0', 'rig_id': 1, 'delta_seconds_after_update': 0, 'leaf_update_weight': 1, 'forward_events': 1, 'construction_events': 1, 'attribute_count': 0, 'before': {'bones': [{'name': 'root'}]}, 'after': {'bones': [{'name': 'root'}]}, 'work_after': {'properties': [{'name': 'state'}]}, 'externals_after': {'input': {}}, 'reference_query_after_forward': {'blocking_hit': True}, 'executions': [{'entry': 'Forwards Solve', 'succeeded': True, 'instructions': [0, 1]}]}
    second = copy.deepcopy(frame)
    second.update(name='1', forward_events=2)
    request = {'bone_names': ['root'], 'cases': [{'name': 'case', 'frames': [{'name': '0', 'delta_seconds': 0}, {'name': '1', 'delta_seconds': 0}]}]}
    fixture = {'rig_node_history_probe': {'cases': [{'name': 'case', 'game_world': True, 'bound_component': True, 'adapter_enabled': True, 'adapter_ready': True, 'external_controls_stage': 'after_update_before_evaluate', 'frames': [frame, second]}]}}
    assert validate(fixture, request)['frames'] == 2
    rejected = 0
    for index in range(8):
        case = copy.deepcopy(fixture)
        row = case['rig_node_history_probe']['cases'][0]
        if index == 0:
            row['game_world'] = False
        elif index == 1:
            row['frames'][1]['rig_id'] = 2
        elif index == 2:
            row['frames'][1]['construction_events'] = 2
        elif index == 3:
            row['frames'][1]['forward_events'] = 1
        elif index == 4:
            row['frames'][0]['executions'] = []
        elif index == 5:
            row['frames'][0]['attribute_count'] = 1
        elif index == 6:
            row['frames'][1]['delta_seconds_after_update'] = .1
        else:
            row['adapter_ready'] = False
        try:
            validate(case, request)
        except (KeyError, AssertionError):
            rejected += 1
    assert rejected == 8
    sample = {'rig_id': 1, 'properties': [{'name': 'cache', 'exported_text': '(ContainerVersion=123)'}, {'name': 'ContainerVersion', 'exported_text': '123'}, {'name': 'spring', 'numeric_value': 1.25}]}
    changed = copy.deepcopy(sample)
    changed['properties'][0]['exported_text'] = '(ContainerVersion=456)'
    changed['properties'][1]['exported_text'] = '456'
    assert sample != changed and normalize(sample) == normalize(changed)
    different_id = copy.deepcopy(sample)
    different_id['rig_id'] = 99
    assert normalize(sample) != normalize(different_id)
    changed['properties'][2]['numeric_value'] = 1.5
    assert normalize(sample) != normalize(changed)
    changed = copy.deepcopy(sample)
    changed['properties'][1]['exported_text'] = '789'
    assert normalize(sample) != normalize(changed)
    return {'self_test': 'PASS', 'invalid_cases_rejected': rejected, 'cache_aliases_preserve_other_fields_and_equality_patterns': True}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--input', type=Path)
    parser.add_argument('--request', type=Path)
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    if args.self_test:
        result = self_test()
    else:
        assert args.input and args.request
        result = validate(json.loads(args.input.read_text()), json.loads(args.request.read_text()))
    print(json.dumps(result))
