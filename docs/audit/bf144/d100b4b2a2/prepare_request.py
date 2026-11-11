import copy
import gzip
import json
import math
from pathlib import Path

root = Path(__file__).parent
native = json.loads(gzip.decompress(Path('docs/audit/bf144/d100b/mesh.native.json.gz').read_bytes()))
geometry = json.loads(gzip.decompress(Path('docs/audit/bf144/d100b4a1/meshes.native.json.gz').read_bytes()))
samples = []
for sample in native['physics_contact_probe']['samples']:
    value = sample['input']
    if value['bone_name'] == 'head' and sample['other_geometry']['kind'] == 'trimesh' and sample['points']:
        request = copy.deepcopy(value)
        request['label'] = value['mesh_asset_path'].rsplit('/', 1)[-1] + '_production'
        request['triangle_indices'] = list(dict.fromkeys(point['face_index'] for point in sample['points']))
        request.pop('placement_gap_cm', None)
        samples.append(request)

base = copy.deepcopy(samples[0])
for asset, faces in [('SM_Als_Cart', [988, 989]), ('SM_Als_Crate_Broken', [175, 176])]:
    sample = next(sample for sample in geometry['physics_contact_probe']['samples']
                  if sample['input']['mesh_asset_path'].endswith('/' + asset))
    mesh = sample['other_geometry']
    triangle = [[mesh['trimesh_core_vertices_cm'][index][axis] for axis in 'xyz']
                for index in mesh['trimesh_core_triangles'][faces[0]]]
    a = [triangle[1][i] - triangle[0][i] for i in range(3)]
    b = [triangle[2][i] - triangle[0][i] for i in range(3)]
    normal = [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
    length = math.sqrt(sum(value * value for value in normal))
    normal = [value / length for value in normal]
    center = [sum(vertex[i] for vertex in triangle) / 3 for i in range(3)]
    for side in (1, -1):
        request = copy.deepcopy(base)
        request['label'] = asset + ('_front' if side > 0 else '_back')
        request['mesh_asset_path'] = sample['input']['mesh_asset_path']
        request['body_translation_cm'] = [center[i] + side * 8.9 * normal[i] - [5, 3, 0][i] for i in range(3)]
        request['triangle_indices'] = faces
        samples.append(request)

(root / 'triangles.request.json').write_text(json.dumps({'samples': samples}, indent=2) + '\n')
