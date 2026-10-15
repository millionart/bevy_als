import argparse
import json
import math
import struct
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


def validate_mesh_visits(path):
    samples = json.loads(path.read_text(encoding='utf-8-sig'))['geometry_query_probe']['samples']
    f32 = lambda value: struct.unpack('<f', struct.pack('<f', value))[0]
    count = 0
    for sample in samples:
        name = sample['input']['name']
        assert sample['input']['mesh_raycast']['capture_ray_visits'], name
        shape = next(s for s in sample['mesh_raycast']['shapes'] if s['complex'])
        assert 'ray_visits' in shape, f'Missing native mesh ray visits: {name}'
        ray = shape['ray_visits']
        assert ray['agrees_with_core'] and ray['length_cm'] > 0, name
        length = f32(ray['length_cm'])
        selected = None
        seen = set()
        for visit in ray['visits']:
            assert visit['internal_face'] not in seen and visit['length_before_cm'] == length, name
            seen.add(visit['internal_face'])
            assert len(visit['vertices_cm']) == 3, name
            if visit['hit']:
                assert not visit['culled'] and 0 <= visit['distance_cm'] <= length, name
                u, v = visit['barycentric_uv']
                assert u >= -1e-8 and v >= -1e-8 and u + v <= 1 + 1e-8, name
                assert abs(sum(x * x for x in visit['normal'].values()) - 1) < 1e-6, name
                length = f32(visit['distance_cm'])
                selected = visit
            assert visit['length_after_cm'] == length, name
            count += 1
        hit = selected is not None and selected['distance_cm'] <= ray['length_cm']
        assert ray['hit'] == hit and ray['selected_internal_face'] == (selected['internal_face'] if selected else -1), name
        query = sample['input']['mesh_raycast']
        delta = [b - a for a, b in zip(query['start_cm'], query['end_cm'])]
        length = f32(math.sqrt(sum(x * x for x in delta)))
        scaled = 'core_inverse_scale' in shape
        distance = ray['distance_cm'] * (1 / ray['length_scale']) if hit else 0
        wrapped_hit = hit and (not scaled or distance == 0 or distance < length)
        if hit:
            assert all(abs(ray['position_cm'][k] - ray['start_cm'][k] - ray['direction'][k] * ray['distance_cm']) < 1e-6 for k in 'xyz'), name
        assert sample['mesh_raycast']['hit'] == wrapped_hit, f'Native wrapped hit differs: {name}'
        if sample['mesh_raycast']['hit']:
            assert hit and selected['external_face'] == sample['mesh_raycast']['face_index'], name
            assert abs(sample['mesh_raycast']['distance_cm'] - f32(distance)) < 1e-5, name
            if distance == 0:
                point = query['start_cm']
                size = math.sqrt(sum(x * x for x in delta))
                normal = [-x / size for x in delta]
            else:
                point = [ray['position_cm'][k] for k in 'xyz']
                normal = [ray['normal'][k] for k in 'xyz']
                if scaled:
                    point = [f32(shape['core_scale'][k] * f32(x)) for k, x in zip('xyz', point)]
                    normal = [f32(shape['core_inverse_scale'][k] * f32(x)) for k, x in zip('xyz', normal)]
                q = sample['input']['rotation_xyzw']
                def rotate(v):
                    cross = lambda a, b: [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
                    t = [2 * x for x in cross(q[:3], v)]
                    return [a + q[3] * b + c for a, b, c in zip(v, t, cross(q[:3], t))]
                point = [a + b for a, b in zip(rotate(point), query['translation_cm'])]
                normal = rotate(normal)
                size = math.sqrt(sum(x * x for x in normal))
                normal = [x / size for x in normal]
            result = sample['mesh_raycast']
            assert all(abs(result['position_cm'][k] - x) < 2e-5 for k, x in zip('xyz', point)), f'Native wrapped position differs: {name}'
            assert all(abs(result['normal'][k] - x) < 1e-6 for k, x in zip('xyz', normal)), f'Native wrapped normal differs: {name}'
    assert samples and count > 0
    print(json.dumps({'native_mesh_ray_visits_self_test': 'PASS', 'samples': len(samples), 'visits': count}), flush=True)


def rotation_request():
    half = math.sqrt(0.5)
    rotations = [[0, 0, 0, 1], [half, 0, 0, half], [0, half, 0, half]]
    samples = [{'name': 'product_xy', 'rotation_xyzw': rotations[1], 'rotation_factors': [{'rotation_xyzw': rotations[2]}]},
               {'name': 'product_yx', 'rotation_xyzw': rotations[2], 'rotation_factors': [{'rotation_xyzw': rotations[1]}]},
               {'name': 'product_inverse', 'rotation_xyzw': rotations[1], 'rotation_factors': [{'rotation_xyzw': [-half, 0, 0, half]}]}]
    for name, normal in [('slope', [0.5, 0, math.sqrt(3) / 2]), ('opposite', [0, 0, -1]), ('zero', [0, 0, 0])]:
        samples.append({'name': name, 'rotation_xyzw': rotations[0], 'rotation_between_vectors': {'from': [0, 0, 1], 'to': normal}})
    return {'samples': samples}


def validate_rotation(path):
    samples = json.loads(path.read_text(encoding='utf-8-sig'))['geometry_query_probe']['samples']
    assert [s['input'] for s in samples] == rotation_request()['samples']
    expected = [[0.5, 0.5, 0.5, 0.5], [0.5, 0.5, -0.5, 0.5], [0, 0, 0, 1],
                [0, math.sin(math.pi / 12), 0, math.cos(math.pi / 12)], [0, -1, 0, 0], [0, 0, 0, 1]]
    for index, (sample, value) in enumerate(zip(samples, expected)):
        key = 'rotation_product' if index < 3 else 'rotation_between_vectors'
        assert key in sample, f'Missing native rotation result: {sample["input"]["name"]}'
        assert all(abs(sample[key][axis] - v) < 1e-12 for axis, v in zip('xyzw', value)), sample['input']['name']
        if index < 3:
            assert all(math.isfinite(sample['rotation_product_degrees'][axis]) for axis in ['pitch', 'yaw', 'roll'])
    print(json.dumps({'native_rotation_self_test': 'PASS', 'samples': len(samples)}), flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--write-request', type=Path)
    parser.add_argument('--report', type=Path)
    parser.add_argument('--write-mesh-request', type=Path)
    parser.add_argument('--mesh-report', type=Path)
    parser.add_argument('--mesh-visits-report', type=Path)
    parser.add_argument('--write-rotation-request', type=Path)
    parser.add_argument('--rotation-report', type=Path)
    args = parser.parse_args()
    if not any([args.write_request, args.report, args.write_mesh_request, args.mesh_report, args.mesh_visits_report, args.write_rotation_request, args.rotation_report]):
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
    if args.mesh_visits_report:
        validate_mesh_visits(args.mesh_visits_report)
    if args.write_rotation_request:
        args.write_rotation_request.write_text(json.dumps(rotation_request(), indent=2), encoding='utf-8')
        print('Wrote native rotation request', flush=True)
    if args.rotation_report:
        validate_rotation(args.rotation_report)
