import argparse
import copy
import json
from pathlib import Path

VIEW_FIELDS = {'RotationWorldSpace', 'YawAngle', 'YawSpeed', 'PitchAngle', 'PitchAmount', 'HeadBlendAmount'}
HEAD_FIELDS = {'bInitializationRequired', 'bSwitchingLookSides', 'PitchAngle', 'YawAngle', 'YawVelocity', 'YawAmount'}


def validate(document):
    count = 0
    for case in document['character_animation_probe']['cases']:
        snapshots = [case['initialization']['source_state']] + [row['source_state_after_tick'] for row in case['frames']]
        for snapshot in snapshots:
            groups = {item['name']: item for item in snapshot}
            for name, required in [('ViewState', VIEW_FIELDS), ('HeadState', HEAD_FIELDS)]:
                fields = {item['name']: item for item in groups[name]['fields']}
                assert set(fields) == required
                for key in required:
                    if key.startswith('b'):
                        assert fields[key]['exported_text'] in ['True', 'False']
                    elif key == 'RotationWorldSpace':
                        rotation = {item['name']: item for item in fields[key]['fields']}
                        assert set(rotation) == {'Pitch', 'Yaw', 'Roll'}
                        assert all(isinstance(item['numeric_value'], (int, float)) for item in rotation.values())
                    else:
                        assert isinstance(fields[key]['numeric_value'], (int, float))
            count += 1
    assert count > 0
    return {'complete_view_head_state_packets': count}


def self_test():
    groups = []
    for name, required in [('ViewState', VIEW_FIELDS), ('HeadState', HEAD_FIELDS)]:
        fields = []
        for key in sorted(required):
            value = {'name': key}
            if key.startswith('b'):
                value['exported_text'] = 'False'
            elif key == 'RotationWorldSpace':
                value['fields'] = [{'name': axis, 'numeric_value': 0} for axis in ['Pitch', 'Yaw', 'Roll']]
            else:
                value['numeric_value'] = 0
            fields.append(value)
        groups.append({'name': name, 'fields': fields})
    fixture = {'character_animation_probe': {'cases': [{'initialization': {'source_state': groups}, 'frames': [{'source_state_after_tick': groups}]}]}}
    assert validate(fixture)['complete_view_head_state_packets'] == 2
    rejected = 0
    for index in range(4):
        item = copy.deepcopy(fixture)
        snapshot = item['character_animation_probe']['cases'][0]['frames'][0]['source_state_after_tick']
        if index < 2:
            snapshot.pop(index)
        elif index == 2:
            snapshot[0]['fields'].pop()
        else:
            next(field for field in snapshot[1]['fields'] if field['name'] == 'PitchAngle').pop('numeric_value')
        try:
            validate(item)
        except (AssertionError, KeyError):
            rejected += 1
    assert rejected == 4
    return {'self_test': 'PASS', 'missing_or_incomplete_groups_rejected': rejected}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--input', type=Path)
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    print(json.dumps(self_test() if args.self_test else validate(json.loads(args.input.read_text()))))
