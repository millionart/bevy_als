import argparse
import copy
import json
import math
from pathlib import Path

PARENT_FIELDS = {'StandingState': {'StrideBlendAmount', 'WalkRunBlendAmount', 'PlayRate', 'SprintBlockAmount', 'SprintTime', 'SprintAccelerationAmount', 'bPivotActive'}, 'RotateInPlaceState': {'bUpdatedThisFrame', 'bRotatingLeft', 'bRotatingRight', 'PlayRate'}}


def finite(value):
    assert type(value) in [int, float] and math.isfinite(value)


def validate(document):
    counts = {'instance_packets': 0, 'machine_packets': 0, 'state_weights': 0, 'transition_entries': 0, 'parent_packets': 0, 'before_tick_packets': 0}
    def instances(values):
        for instance in values:
            assert instance['weight_read_buffer_index'] in [0, 1]
            for machine in instance['machines']:
                assert type(machine['node_index']) is int and machine['node_index'] >= 0
                assert type(machine['machine_class_index']) is int and machine['machine_class_index'] >= 0
                finite(machine['recorded_machine_weight'])
                assert machine['state_weights']
                for state in machine['state_weights']:
                    assert type(state['index']) is int and isinstance(state['name'], str)
                    finite(state['recorded_weight'])
                    finite(state['current_weight'])
                    counts['state_weights'] += 1
                assert isinstance(machine['active_transitions'], list)
                for transition in machine['active_transitions']:
                    for key in ['previous_state', 'next_state', 'logic_type', 'blend_option', 'cached_pose_bone_count']:
                        assert type(transition[key]) is int
                    for key in ['elapsed', 'alpha', 'crossfade_duration']:
                        finite(transition[key])
                    assert isinstance(transition['active'], bool)
                    assert isinstance(transition['blend_profile'], str)
                    assert all(type(value) is int for value in transition['source_transition_indices'])
                    counts['transition_entries'] += 1
                counts['machine_packets'] += 1
            counts['instance_packets'] += 1
    def parent(values):
        groups = {value['name']: value for value in values}
        for name in ['Gait', 'GroundedEntryMode']:
            assert isinstance(groups[name]['value'], str)
        for name, expected in PARENT_FIELDS.items():
            fields = {value['name']: value for value in groups[name]['fields']}
            assert set(fields) == expected
            for key in expected:
                if key.startswith('b'):
                    assert fields[key]['exported_text'] in ['True', 'False']
                else:
                    finite(fields[key]['numeric_value'])
        counts['parent_packets'] += 1
    for case in document['character_animation_probe']['cases']:
        parent(case['initialization']['source_state'])
        instances(case['initialization']['instances'])
        for frame in case['frames']:
            parent(frame['source_state_after_tick'])
            instances(frame['instances_before_tick'])
            instances(frame['linked_instances'])
            counts['before_tick_packets'] += 1
    assert counts['machine_packets'] > 0 and counts['before_tick_packets'] > 0
    return counts


def self_test():
    groups = [{'name': name, 'value': ''} for name in ['Gait', 'GroundedEntryMode']]
    for name, expected in PARENT_FIELDS.items():
        groups.append({'name': name, 'fields': [{'name': key, **({'exported_text': 'False'} if key.startswith('b') else {'numeric_value': 0})} for key in expected]})
    machine = {'node_index': 5, 'machine_class_index': 0, 'recorded_machine_weight': 1, 'state_weights': [{'index': 0, 'name': 'Idle', 'recorded_weight': 1, 'current_weight': 1}], 'active_transitions': []}
    items = [{'weight_read_buffer_index': 0, 'machines': [machine]}]
    fixture = {'character_animation_probe': {'cases': [{'initialization': {'source_state': groups, 'instances': items}, 'frames': [{'source_state_after_tick': groups, 'instances_before_tick': items, 'linked_instances': items}]}]}}
    assert validate(fixture)['before_tick_packets'] == 1
    rejected = 0
    for key in ['instances_before_tick', 'weight_read_buffer_index', 'node_index', 'state_weights', 'active_transitions', 'nonfinite', 'parent_group']:
        data = copy.deepcopy(fixture)
        frame = data['character_animation_probe']['cases'][0]['frames'][0]
        if key == 'instances_before_tick':
            frame.pop(key)
        elif key == 'parent_group':
            frame['source_state_after_tick'].pop()
        elif key == 'weight_read_buffer_index':
            frame['linked_instances'][0].pop(key)
        elif key == 'nonfinite':
            frame['linked_instances'][0]['machines'][0]['recorded_machine_weight'] = float('nan')
        else:
            frame['linked_instances'][0]['machines'][0].pop(key)
        try:
            validate(data)
        except (AssertionError, KeyError):
            rejected += 1
    assert rejected == 7
    return {'self_test': 'PASS', 'missing_or_invalid_fsm_phase_inputs_rejected': rejected}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--input', type=Path)
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    print(json.dumps(self_test() if args.self_test else validate(json.loads(args.input.read_text()))))
