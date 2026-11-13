import argparse
import gzip
import json
from pathlib import Path


def load(path):
    if path.suffix == '.gz':
        with gzip.open(path, 'rt') as stream:
            return json.load(stream)
    return json.loads(path.read_bytes())


def fields(value):
    return {field['name']: field for field in value['fields']}


def vector(value):
    value = fields(value)
    return [value[axis]['numeric_value'] for axis in 'XZY']


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('source', type=Path)
    parser.add_argument('restored', type=Path)
    parser.add_argument('without_unit_rotation', type=Path)
    args = parser.parse_args()
    source = load(args.source)['character_animation_probe']['cases'][0]
    restored = load(args.restored)
    ablated = load(args.without_unit_rotation)
    assert len(restored) == len(ablated) == len(source['frames']) + 1 == 319
    rows = []
    for index in range(2, 8):
        frame = source['frames'][index - 1]
        source_feet = fields(next(field for field in frame['source_state_after_tick'] if field['name'] == 'FeetState'))
        for side, register in [('Left', 7), ('Right', 9)]:
            foot = fields(source_feet[side])
            expected = vector(foot['FinalLocation'])
            precise = restored[index]['raw_rig']['externals']['RigInput']['List'][register]['Vector']
            control = ablated[index]['raw_rig']['externals']['RigInput']['List'][register]['Vector']
            assert precise == expected, (index, side, precise, expected)
            assert control != expected, (index, side, control)
            rows.append({'frame': index, 'side': side, 'source_and_restored_cm': expected, 'without_unit_rotation_cm': control})
    print(json.dumps({'frames': len(restored), 'exact_foot_vectors': len(rows), 'rows': rows}, indent=2))


if __name__ == '__main__':
    main()
