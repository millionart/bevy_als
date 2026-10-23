import argparse
import json
import struct
from pathlib import Path


def validate(warm, native):
    first = warm[0]
    records = first['rig_link_records']
    assert len(records) == 1
    record = records[0]
    assert record['inner'] == record['rig_input']
    assert record['external_values_after_execution']['RigInput'] == first['rig_fields_after']
    source = native['rig_node_history_probe']['cases'][0]['frames'][0]
    before = record['rig_input']
    after = record['rig_output']
    assert source['before']['curves'] == source['after']['curves'] == before['curves'] == after['curves']
    f32 = lambda value: struct.unpack('f', struct.pack('f', value))[0]
    target = {bone['name']: bone for bone in after['bones']}
    assert len(target) == len(after['bones'])
    names = [bone['name'] for bone in source['after']['bones']]
    assert names and len(set(names)) == len(names) and set(names) <= target.keys()
    for bone in source['after']['bones']:
        transform = bone['local_transform']
        translation, rotation, scale = (transform[key] for key in ['translation', 'rotation', 'scale'])
        assert target[bone['name']]['translation'] == [f32(translation[key] * .01) for key in ['x', 'z', 'y']]
        assert target[bone['name']]['rotation'] == [f32(-rotation['x']), f32(-rotation['z']), f32(-rotation['y']), f32(rotation['w'])]
        assert target[bone['name']]['scale'] == [f32(scale[key]) for key in ['x', 'z', 'y']]
    return {'same_input_native_bones_exact': len(names), 'actual_rig_cache_required': True}


def self_test():
    bone = {'name': 'root', 'translation': [0, 0, 0], 'rotation': [0, 0, 0, 1], 'scale': [1, 1, 1]}
    pose = {'bones': [bone], 'curves': {'PoseGrounded': 1}}
    warm = [{'rig_fields_after': [], 'rig_link_records': [{'inner': pose, 'rig_input': pose, 'rig_output': pose, 'external_values_after_execution': {'RigInput': []}}]}]
    transform = {'translation': dict(x=0, y=0, z=0), 'rotation': dict(x=0, y=0, z=0, w=1), 'scale': dict(x=1, y=1, z=1)}
    native = {'rig_node_history_probe': {'cases': [{'frames': [{'before': {'curves': pose['curves']}, 'after': {'curves': pose['curves'], 'bones': [{'name': 'root', 'local_transform': transform}]}}]}]}}
    assert validate(warm, native)['same_input_native_bones_exact'] == 1
    rejected = 0
    for index in range(4):
        changed = json.loads(json.dumps(warm))
        record = changed[0]['rig_link_records'][0]
        if index == 0:
            changed[0].pop('rig_link_records')
        elif index == 1:
            record['rig_output']['bones'][0]['rotation'][3] = .99999994
        elif index == 2:
            record['rig_output']['curves'] = {}
        else:
            record['external_values_after_execution']['RigInput'] = [False]
        try:
            validate(changed, native)
        except (AssertionError, KeyError):
            rejected += 1
    assert rejected == 4
    return {'self_test': 'PASS', 'invalid_boundaries_rejected': rejected}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--warm', type=Path)
    parser.add_argument('--native', type=Path)
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    if args.self_test:
        result = self_test()
    else:
        assert args.warm and args.native
        result = validate(json.loads(args.warm.read_text()), json.loads(args.native.read_text()))
    print(json.dumps(result))
