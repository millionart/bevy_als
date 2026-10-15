import argparse
import json
import math
from pathlib import Path


def request():
    samples = []
    for name, start, length in [('before_end', [0, 0, 50], 100.01), ('at_end', [0, 0, 50], 100.0),
                                ('after_end', [0, 0, 50], 99.99), ('inside', [0, 0, -60], 130.0),
                                ('surface', [0, 0, -50], 130.0)]:
        samples.append({'name': name, 'rotation_xyzw': [0, 0, 0, 1], 'box_raycast': {
            'box_translation_cm': [0, 0, -75], 'box_half_extents_cm': [100, 100, 25],
            'start_cm': start, 'direction': [0, 0, -1], 'length_cm': length}})
    rotated = {**samples[0], 'name': 'rotated', 'rotation_xyzw': [math.sin(math.pi / 12), 0, 0, math.cos(math.pi / 12)]}
    samples.append(rotated)
    for kind in ['sphere_sweep', 'capsule_sweep']:
        sweep = {**samples[0]['box_raycast'], 'radius_cm': 10, 'half_height_cm': 30}
        samples.append({'name': kind, 'rotation_xyzw': [0, 0, 0, 1], kind: sweep})
    return {'samples': samples}


def validate(path):
    document = json.loads(path.read_text(encoding='utf-8-sig'))
    samples = document['geometry_query_probe']['samples']
    expected = {'before_end': 100, 'at_end': 100, 'after_end': None, 'inside': 0, 'surface': 0,
                'rotated': 125 - 25 / math.cos(math.pi / 6), 'sphere_sweep': 90, 'capsule_sweep': 70}
    seen = set()
    for sample in samples:
        name = sample['input']['name']
        kind = name if name.endswith('_sweep') else 'box_raycast'
        assert kind in sample, f'Missing native query result: {name}/{kind}'
        result = sample[kind]
        assert result['hit'] == (expected[name] is not None), name
        if result['hit']:
            assert abs(result['distance_cm'] - expected[name]) < 1e-4, name
            position = result['position_cm']
            if kind == 'box_raycast':
                query = sample['input'][kind]
                point = [a + b * result['distance_cm'] for a, b in zip(query['start_cm'], query['direction'])]
                assert all(abs(position[axis] - value) < 1e-6 for axis, value in zip('xyz', point)), name
            else:
                assert abs(position['z'] + 50) < 1e-6, name
            normal = result['normal']
            if name in ['inside', 'surface']:
                assert all(value == 0 for value in normal.values()), name
            elif name == 'rotated':
                assert abs(normal['y'] + 0.5) < 1e-6 and abs(normal['z'] - math.cos(math.pi / 6)) < 1e-6
            else:
                assert abs(normal['z'] - 1) < 1e-6, name
        assert name not in seen, name
        seen.add(name)
    assert seen == set(expected), sorted(seen)
    print(json.dumps({'native_geometry_query_self_test': 'PASS', 'samples': len(seen)}), flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--write-request', type=Path)
    parser.add_argument('--report', type=Path)
    args = parser.parse_args()
    if not args.write_request and not args.report:
        parser.error('Provide --write-request or --report')
    if args.write_request:
        args.write_request.write_text(json.dumps(request(), indent=2), encoding='utf-8')
        print('Wrote native geometry query request', flush=True)
    if args.report:
        validate(args.report)
