import copy
import gzip
import json
import math
import sys
from pathlib import Path


def vector(value):
    return [value[axis] for axis in 'xyz']


def yaw_rotate(point, rotation):
    x, y, z = point
    qx, qy, qz, qw = rotation
    if qx != 0 or qy != 0:
        raise ValueError('Fixture requires a yaw-only rotation')
    cosine = 1 - 2 * qz * qz
    sine = 2 * qz * qw
    return [cosine * x - sine * y, sine * x + cosine * y, z]


def read_capture(path):
    return json.loads(gzip.decompress(path.read_bytes()))['physics_contact_probe']['samples']


def triangle_vertices(sample, face_index):
    mesh = sample['other_geometry']
    return [vector(mesh['trimesh_core_vertices_cm'][vertex]) for vertex in mesh['trimesh_core_triangles'][face_index]]


def make_request(root):
    samples = read_capture(root.parent / 'd100b4b2a2' / 'triangles.retry.native.json.gz')
    stairs = next(sample for sample in samples if sample['input']['label'] == 'SM_Als_Stairs_01_production')
    a, _, c = triangle_vertices(stairs, 252)
    if a[1:] != c[1:] or a[0] <= c[0]:
        raise ValueError('Stair edge must point along positive source X')
    first = vector(stairs['geometry']['endpoint_0_cm'])
    second = vector(stairs['geometry']['endpoint_1_cm'])
    angle = -math.atan2(second[1] - first[1], second[0] - first[0])
    rotation = [0, 0, math.sin(angle * 0.5), math.cos(angle * 0.5)]
    first_rotated = yaw_rotate(first, rotation)
    inputs = []
    for label, x in [('both_inside', c[0] + 100), ('one_outside', a[0] - 3), ('both_before', c[0] - 10)]:
        request = copy.deepcopy(stairs['input'])
        request['label'] = 'stair_parallel_' + label
        request['body_rotation_xyzw'] = rotation
        request['body_translation_cm'] = [value - offset for value, offset in
                                          zip([x, c[1] - 4, c[2] + 8.9], first_rotated)]
        request['triangle_indices'] = [252]
        inputs.append(request)
    (root / 'parallel.request.json').write_text(json.dumps({'samples': inputs}, indent=2) + '\n')


def make_fixture(root):
    samples = []
    for sample in read_capture(root / 'parallel.native.json.gz'):
        request = sample['input']
        geometry = sample['geometry']
        mesh = sample['other_geometry']
        for shape in (geometry, mesh):
            local = shape['shape_to_body']
            if vector(local['translation']) != [0, 0, 0] or vector(local['scale']) != [1, 1, 1]:
                raise ValueError('Fixture requires identity shape transforms')
            if [local['rotation'][axis] for axis in 'xyzw'] != [0, 0, 0, 1]:
                raise ValueError('Fixture requires identity local rotations')
        if request['mesh_rotation_xyzw'] != [0, 0, 0, 1] or vector(mesh['trimesh_core_scale']) != [1, 1, 1]:
            raise ValueError('Fixture requires an unrotated unit mesh')
        x, y, z, w = request['body_rotation_xyzw']
        inverse = [-x, -y, -z, w]
        shift = yaw_rotate([m - b for m, b in zip(request['mesh_translation_cm'], request['body_translation_cm'])], inverse)
        triangles = []
        for face in sample['triangle_contacts']:
            index = face['triangle_index']
            triangles.append({
                'index': index,
                'vertices_cm': [[a + b for a, b in zip(yaw_rotate(vertex, inverse), shift)]
                                for vertex in triangle_vertices(sample, index)],
                'points': [{
                    'first_cm': vector(point['shape_point_0_cm']),
                    'second_cm': vector(point['shape_point_1_cm']),
                    'normal': yaw_rotate(vector(point['world_normal']), inverse),
                    'phi_cm': point['phi_cm'],
                    'contact_type': point['contact_type'],
                } for point in face['points']],
            })
        samples.append({
            'label': request['label'],
            'endpoints_cm': [vector(geometry['endpoint_0_cm']), vector(geometry['endpoint_1_cm'])],
            'radius_cm': geometry['radius_cm'],
            'cull_cm': request['cull_distance_cm'],
            'triangles': triangles,
        })
    (root / 'parallel-cases.json').write_text(json.dumps(samples, indent=2) + '\n')


{'request': make_request, 'fixture': make_fixture}[sys.argv[1]](Path(__file__).parent)
