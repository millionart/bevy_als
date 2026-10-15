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


def mesh_request():
    samples = []
    for mesh in ['SM_Als_Cart', 'SM_Als_Barrel']:
        for start, end in [([0, 0, 200], [0, 0, -200]), ([0, 0, -50], [0, 0, 200]), ([0, 0, 50], [0, 0, -50])]:
            for complex in [True, False]:
                samples.append({'name': f'{mesh}_{len(samples)}', 'rotation_xyzw': [0, 0, 0, 1], 'mesh_raycast': {
                    'asset_path': '/ALS/ALSExtras/Environment/Meshes/' + mesh, 'translation_cm': [0, 0, 0],
                    'scale': [1, 1, 1], 'start_cm': start, 'end_cm': end, 'trace_complex': complex}})
    return {'samples': samples}


def validate_mesh(path):
    document = json.loads(path.read_text(encoding='utf-8-sig'))
    samples = document['geometry_query_probe']['samples']
    expected = [55.6957026654, 55.6957026654, 41.1642999472, 41.1642999472, 44.7454986572, 44.7454986572,
                92.2190959759, 96.0999984741, 3.8036731510, -0.1000000313, None, 50]
    assert [s['input'] for s in samples] == mesh_request()['samples']
    for sample, height in zip(samples, expected):
        name = sample['input']['name']
        assert 'mesh_raycast' in sample, f'Missing native mesh ray result: {name}'
        result = sample['mesh_raycast']
        assert not result['double_sided_geometry'] and result['hit'] == (height is not None), name
        assert result['collision_trace_flag'] == (3 if name.startswith('SM_Als_Cart') else 1), name
        complex = [s for s in result['shapes'] if s['complex']]
        assert len(complex) == 1 and complex[0]['triangle_mesh'] and complex[0]['culls_back_face'], name
        assert all(s['query_enabled'] for s in result['shapes']), name
        assert complex[0]['core_scale'] == dict.fromkeys('xyz', 1), name
        assert complex[0]['simple'] == name.startswith('SM_Als_Cart'), name
        if height is not None:
            point = result['position_cm']
            assert abs(point['x']) < 1e-6 and abs(point['y']) < 1e-6 and abs(point['z'] - height) < 1e-5, name
            assert abs(result['distance_cm'] - abs(sample['input']['mesh_raycast']['start_cm'][2] - height)) < 2e-5, name
            assert abs(sum(v * v for v in result['normal'].values()) - 1) < 1e-6, name
    print(json.dumps({'native_mesh_raycast_self_test': 'PASS', 'samples': len(samples)}), flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--write-request', type=Path)
    parser.add_argument('--report', type=Path)
    parser.add_argument('--write-mesh-request', type=Path)
    parser.add_argument('--mesh-report', type=Path)
    args = parser.parse_args()
    if not any([args.write_request, args.report, args.write_mesh_request, args.mesh_report]):
        parser.error('Provide a request output or native report')
    if args.write_request:
        args.write_request.write_text(json.dumps(request(), indent=2), encoding='utf-8')
        print('Wrote native geometry query request', flush=True)
    if args.report:
        validate(args.report)
    if args.write_mesh_request:
        args.write_mesh_request.write_text(json.dumps(mesh_request(), indent=2), encoding='utf-8')
        print('Wrote native mesh ray request', flush=True)
    if args.mesh_report:
        validate_mesh(args.mesh_report)
