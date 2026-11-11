import argparse
import gzip
import json
import math
from pathlib import Path

from import_movement_constants import NUMBER, literal


def extract(facts, physics):
    text = '\n'.join(item['text'] for item in facts)
    settings = physics['constraint_console_variables']
    expected = {
        'p.Chaos.Collision.ConvexTriMeshMode': '1',
        'p.Chaos.Collision.UseCapsuleTriMesh2': 'true',
        'p.Chaos.Collision.EnableMeshManifoldOptimizedLoop': 'true',
        'p.Chaos.Collision.EnableMeshManifoldOptimizedLoopTriMesh': 'true',
        'p.Chaos.Collision.ConvexTriMeshSortByPhi': 'false',
        'p.Chaos.Collision.ConvexTriMeshSortByDistance': 'false',
        'p.Chaos.MeshContactGenerator.FixContactNormal.FixEnabled': 'true',
        'p.Chaos.MeshContactGenerator.TreatBoundaryEdgeAsFace': 'true',
        'p.Chaos.MeshContactGenerator.TreatBoundaryVertexAsFace': 'true',
        'p.Chaos.Solver.bChaosForceMACD': 'false',
    }
    for name, value in expected.items():
        if settings.get(name) != value:
            raise ValueError(f'Unexpected source mesh contact setting: {name}')
    bodies = physics['bodies']
    if not bodies or any([prop['exported_text'] for prop in body['default_instance_properties']
                          if prop['name'] == 'bUseMACD'] != ['False'] for body in bodies):
        raise ValueError('Source physics bodies must use ordinary mesh contacts')
    barycentric = literal(text, rf'BarycentricTolerance\s*=\s*FReal\({NUMBER}\)', 'barycentric tolerance')
    backface = literal(text, rf'BackFaceCullTolerance\s*=\s*FReal\({NUMBER}\)', 'backface tolerance')
    capacity = literal(text, rf'MaxContactsBufferSize\s*=\s*{NUMBER}\s*;', 'contact capacity')
    face_dot = float(settings['p.Chaos.Collision.MeshContactNormalThreshold'])
    reject = float(settings['p.Chaos.Collision.MeshContactNormalRejectionThreshold'])
    if capacity != int(capacity) or not all(math.isfinite(value) and 0 < value <= 1 for value in (face_dot, reject)):
        raise ValueError('Source mesh contact scalars are invalid')
    return f'({barycentric!r}_f64, {face_dot!r}_f32, {reject!r}_f32, {backface!r}_f64, {int(capacity)}_usize)\n'


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--facts', type=Path, required=True)
    parser.add_argument('--native', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.output.resolve() in (args.facts.resolve(), args.native.resolve()):
        raise ValueError('Target output must differ from source')
    facts = json.loads(args.facts.read_text())
    physics = json.loads(gzip.decompress(args.native.read_bytes()))['physics_asset']
    args.output.write_text(extract(facts, physics))


if __name__ == '__main__':
    main()
