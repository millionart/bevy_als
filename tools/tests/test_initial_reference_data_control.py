import argparse
import json
from pathlib import Path


def validate(rows, source_reference):
    assert rows and source_reference
    results = []
    for row in rows:
        original = {bone['name']: bone for bone in row['reference']['bones']}
        restored = {bone['name']: bone for bone in row['source_reference']['bones']}
        assert original.keys() == restored.keys()
        for name, wanted in source_reference.items():
            assert restored[name]['present']
            assert all(restored[name][key] == wanted[key] for key in ['translation', 'rotation', 'scale'])
        assert all(original[name] == restored[name] for name in original.keys() - source_reference.keys())
        assert row['reference']['timestamp'] == row['source_reference']['timestamp']
        assert row['reference']['curves'] == row['source_reference']['curves']
        before = {bone['name']: bone for bone in row['original_delta']['bones']}
        after = {bone['name']: bone for bone in row['control_delta']['bones']}
        assert before.keys() == after.keys()
        changed = [name for name in source_reference if before[name] != after[name]]
        assert changed
        assert row['original_delta']['timestamp'] == row['control_delta']['timestamp']
        assert row['original_delta']['curves'] == row['control_delta']['curves']
        results.append({'kind': row['kind'], 'changed_bones': len(changed)})
    return results


def self_test():
    source = {'root': {'translation': [1, 0, 0], 'rotation': [0, 0, 0, 1], 'scale': [1, 1, 1]}}
    bone = {'name': 'root', 'present': True, **source['root']}
    pose = {'bones': [bone], 'timestamp': 0, 'curves': None}
    changed = json.loads(json.dumps(pose))
    changed['bones'][0]['translation'] = [-1, 0, 0]
    fixture = [{'kind': 'local', 'reference': pose, 'source_reference': pose, 'original_delta': pose, 'control_delta': changed}]
    assert validate(fixture, source)[0]['changed_bones'] == 1
    rejected = 0
    for index in range(4):
        rows = json.loads(json.dumps(fixture))
        if index == 0:
            rows[0]['source_reference']['bones'][0]['translation'] = [0, 0, 0]
        elif index == 1:
            rows[0]['control_delta'] = rows[0]['original_delta']
        elif index == 2:
            rows[0]['source_reference']['timestamp'] = .5
        else:
            rows[0]['control_delta']['curves'] = {'unexpected': 1}
        try:
            validate(rows, source)
        except AssertionError:
            rejected += 1
    assert rejected == 4
    return {'self_test': 'PASS', 'invalid_reference_controls_rejected': rejected}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--self-test', action='store_true', required=True)
    parser.parse_args()
    print(json.dumps(self_test()))
