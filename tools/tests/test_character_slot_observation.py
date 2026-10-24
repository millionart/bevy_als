import argparse
import copy
import json
import math
from pathlib import Path


def validate(document):
    packets = 0
    slots = 0
    for case in document['character_animation_probe']['cases']:
        snapshots = [case['initialization']['instances']] + [row['linked_instances'] for row in case['frames']]
        for instances in snapshots:
            assert instances
            for instance in instances:
                assert isinstance(instance['slots'], list)
                properties = set()
                for slot in instance['slots']:
                    assert isinstance(slot['property'], str) and slot['property']
                    assert slot['property'] not in properties
                    properties.add(slot['property'])
                    assert isinstance(slot['slot_name'], str) and slot['slot_name']
                    assert isinstance(slot['always_update_source_pose'], bool)
                    for name in ['source_weight', 'slot_node_weight', 'total_node_weight']:
                        value = slot[name]
                        assert type(value) in [int, float] and math.isfinite(value)
                    slots += 1
                packets += 1
    assert packets > 0 and slots > 0
    return {'complete_instance_slot_packets': packets, 'finite_slot_weight_records': slots}


def self_test():
    slot = {'property': 'Slot_0', 'slot_name': 'Body', 'always_update_source_pose': False, 'source_weight': 1, 'slot_node_weight': 0, 'total_node_weight': 0}
    instances = [{'slots': [slot]}, {'slots': []}]
    fixture = {'character_animation_probe': {'cases': [{'initialization': {'instances': instances}, 'frames': [{'linked_instances': instances}]}]}}
    assert validate(fixture)['finite_slot_weight_records'] == 2
    rejected = 0
    for name in ['slots', 'source_weight', 'slot_node_weight', 'total_node_weight', 'always_update_source_pose', 'nonfinite']:
        data = copy.deepcopy(fixture)
        instance = data['character_animation_probe']['cases'][0]['frames'][0]['linked_instances'][0]
        if name == 'slots':
            instance.pop(name)
        elif name == 'nonfinite':
            instance['slots'][0]['source_weight'] = float('nan')
        else:
            instance['slots'][0].pop(name)
        try:
            validate(data)
        except (AssertionError, KeyError):
            rejected += 1
    assert rejected == 6
    return {'self_test': 'PASS', 'missing_or_invalid_slot_records_rejected': rejected}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--input', type=Path)
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    print(json.dumps(self_test() if args.self_test else validate(json.loads(args.input.read_text()))))
