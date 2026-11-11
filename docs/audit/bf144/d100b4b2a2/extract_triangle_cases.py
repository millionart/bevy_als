import gzip
import json
from pathlib import Path


def vector(value):
    return [value[axis] for axis in 'xyz']


root = Path(__file__).parent
native = json.loads(gzip.decompress((root / 'triangles.retry.native.json.gz').read_bytes()))
samples = []
for sample in native['physics_contact_probe']['samples']:
    request = sample['input']
    capsule = sample['geometry']
    mesh = sample['other_geometry']
    for shape in (capsule, mesh):
        local = shape['shape_to_body']
        if vector(local['translation']) != [0, 0, 0] or vector(local['scale']) != [1, 1, 1]:
            raise ValueError('Fixture requires identity shape transforms')
        if [local['rotation'][axis] for axis in 'xyzw'] != [0, 0, 0, 1]:
            raise ValueError('Fixture requires identity shape rotations')
    if request['body_rotation_xyzw'] != [0, 0, 0, 1] or request['mesh_rotation_xyzw'] != [0, 0, 0, 1]:
        raise ValueError('Fixture requires identity world rotations')
    if vector(mesh['trimesh_core_scale']) != [1, 1, 1]:
        raise ValueError('Fixture requires unit mesh scale')

    translation = [mesh_cm - body_cm for mesh_cm, body_cm in
                   zip(request['mesh_translation_cm'], request['body_translation_cm'])]
    triangles = []
    for face in sample['triangle_contacts']:
        index = face['triangle_index']
        vertices = [vector(mesh['trimesh_core_vertices_cm'][vertex]) for vertex in mesh['trimesh_core_triangles'][index]]
        triangles.append({
            'index': index,
            'vertices_cm': [[component + shift for component, shift in zip(vertex, translation)] for vertex in vertices],
            'points': [{
                'first_cm': vector(point['shape_point_0_cm']),
                'second_cm': vector(point['shape_point_1_cm']),
                'normal': vector(point['world_normal']),
                'phi_cm': point['phi_cm'],
                'contact_type': point['contact_type'],
            } for point in face['points']],
        })
    samples.append({
        'label': request['label'],
        'endpoints_cm': [vector(capsule['endpoint_0_cm']), vector(capsule['endpoint_1_cm'])],
        'radius_cm': capsule['radius_cm'],
        'cull_cm': request['cull_distance_cm'],
        'triangles': triangles,
    })

(root / 'triangle-cases.json').write_text(json.dumps(samples, indent=2) + '\n')
