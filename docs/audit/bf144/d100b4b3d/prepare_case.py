import gzip
import json
from pathlib import Path


root = Path(__file__).parent
source = root.parent / 'd100b4b2a2'
native = json.loads(gzip.decompress((source / 'triangles.retry.native.json.gz').read_bytes()))
cases = json.loads((source / 'triangle-cases.json').read_text())
label = 'SM_Als_Stairs_01_production'
sample = next(row for row in native['physics_contact_probe']['samples'] if row['input']['label'] == label)
case = next(row for row in cases if row['label'] == label)
if sample['input']['body_rotation_xyzw'] != [0, 0, 0, 1]:
    raise ValueError('Fixture requires identity body rotation')
triangles = [dict(face_index=face['index'], vertices_cm=face['vertices_cm'],
                  vertex_indices=sample['other_geometry']['trimesh_core_triangles'][face['index']])
             for face in case['triangles'] if face['index'] in (270, 271)]
raw, = next(face['points'] for face in case['triangles'] if face['index'] == 271)
expected, = [point for point in sample['points'] if point['face_index'] == 271]
case = {'triangles': triangles, 'raw': raw, 'expected': {
    'first_cm': [expected['shape_point_0_cm'][axis] for axis in 'xyz'],
    'normal': [expected['world_normal'][axis] for axis in 'xyz'],
    'phi_cm': expected['phi_cm'],
    'contact_type': expected['contact_type'],
    'face_index': expected['face_index'],
}}
(root / 'edge-normal-case.json').write_text(json.dumps(case, indent=2) + '\n')
