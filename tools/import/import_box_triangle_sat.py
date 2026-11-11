import argparse
import gzip
import json
import re
from pathlib import Path

from import_movement_constants import NUMBER, literal


def sat_data(facts):
    text = '\n'.join(item['text'] for item in facts)
    table = re.search(r'TArray<TArray<int32>>\s+PlaneVertices\s*\{(.*?)\};', text, re.DOTALL)
    if table is None:
        raise ValueError('Source box topology is missing')
    faces = [[int(vertex) for vertex in face.split(',')]
             for face in re.findall(r'\{\s*([0-9,\s]+)\}', table.group(1))]
    if len(faces) != 6 or any(len(face) != 4 or len(set(face)) != 4 or not all(0 <= vertex < 8 for vertex in face)
                              for face in faces):
        raise ValueError('Source box face vertices are invalid')
    half_edges = [(start, end, plane) for plane, face in enumerate(faces)
                  for start, end in zip(face, face[1:] + face[:1])]
    planes = {(start, end): plane for start, end, plane in half_edges}
    if len(planes) != 24 or any((end, start) not in planes or start ^ end not in (1, 2, 4) for start, end in planes):
        raise ValueError('Source box half edges are invalid')
    edges = [[start, end, plane, planes[end, start]] for start, end, plane in half_edges if start < end]
    minkowski = literal(text, rf'const FReal Tolerance\s*=\s*{NUMBER}f\s*;', 'Minkowski tolerance')
    bias = literal(text, rf'const FReal TriFaceBias\s*=\s*FReal\({NUMBER}\)', 'triangle face bias')
    return f'({edges!r}, {minkowski!r}_f32, {bias!r}_f64)\n'


def vector(value):
    return [value[axis] for axis in 'xyz']


def cart_fixture(report):
    if report['physics_asset']['constraint_console_variables']['p.Chaos.Collision.ConvexTriMeshMode'] != '1':
        raise ValueError('Source report must use mesh mode 1')
    sample = next(sample for sample in report['physics_contact_probe']['samples']
                  if sample['input']['label'] == 'SM_Als_Cart_foot_shift_0.2')
    request = sample['input']
    box = sample['geometry']
    mesh = sample['other_geometry']
    if box['kind'] != 'box' or mesh['kind'] != 'trimesh' or vector(mesh['trimesh_core_scale']) != [1, 1, 1]:
        raise ValueError('Fixture requires a box and a unit triangle mesh')
    for shape in (box, mesh):
        local = shape['shape_to_body']
        if vector(local['translation']) != [0, 0, 0] or vector(local['scale']) != [1, 1, 1]:
            raise ValueError('Fixture requires identity shape transforms')
        if [local['rotation'][axis] for axis in 'xyzw'] != [0, 0, 0, 1]:
            raise ValueError('Fixture requires identity shape rotations')
    if request['body_rotation_xyzw'] != [0, 0, 0, 1] or request['mesh_rotation_xyzw'] != [0, 0, 0, 1]:
        raise ValueError('Fixture requires identity world rotations')
    shift = [mesh_cm - body_cm for mesh_cm, body_cm in
             zip(request['mesh_translation_cm'], request['body_translation_cm'])]
    faces = []
    for index in (988, 989):
        points = [p for p in sample['points'] if p['face_index'] == index]
        if not points or any(p['contact_type'] != points[0]['contact_type'] or p['world_normal'] != points[0]['world_normal']
                             for p in points):
            raise ValueError('Source face must have one contact type and normal')
        vertices = [vector(mesh['trimesh_core_vertices_cm'][vertex]) for vertex in mesh['trimesh_core_triangles'][index]]
        faces.append({'index': index,
                      'vertices_cm': [[v + t for v, t in zip(vertex, shift)] for vertex in vertices],
                      'contact_type': points[0]['contact_type'],
                      'normal': vector(points[0]['world_normal'])})
    return {'bounds_cm': [vector(box['minimum_cm']), vector(box['maximum_cm'])],
            'cull_cm': request['cull_distance_cm'], 'faces': faces}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--facts', type=Path, required=True)
    parser.add_argument('--native', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--fixture', type=Path, required=True)
    args = parser.parse_args()
    if len({path.resolve() for path in (args.facts, args.native, args.output, args.fixture)}) != 4:
        raise ValueError('Source and target files must be distinct')
    generated = sat_data(json.loads(args.facts.read_text()))
    fixture = cart_fixture(json.loads(gzip.decompress(args.native.read_bytes())))
    args.output.write_text(generated)
    args.fixture.write_text(json.dumps(fixture, indent=2) + '\n')


if __name__ == '__main__':
    main()
