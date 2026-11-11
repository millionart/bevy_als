import gzip
import json
from pathlib import Path


def vector(value):
    return [value[axis] for axis in 'xyz']


root = Path(__file__).parent
inputs = {
    'd104b3b/restore.native.json.gz': ['SM_Als_Cart_foot_shift_0.2', 'SM_Als_Stairs_02_foot_shift_0.2'],
    'd100b/mesh.native.json.gz': ['SM_Als_Rock_shape_0_head_scale_1_gap_-0.1',
                                'SM_Als_Rock_shape_0_foot_l_scale_1_gap_-0.1',
                                'SM_Als_Rock_shape_0_head_scale_1_gap_30'],
    'd100b4b2a2/triangles.retry.native.json.gz': ['SM_Als_Stairs_01_production',
                                              'SM_Als_Crate_Broken_front', 'SM_Als_Crate_Broken_back'],
    'd100b4b2a3/parallel.native.json.gz': ['stair_parallel_both_inside'],
}
cases = []
for path, labels in inputs.items():
    report = json.loads(gzip.decompress((root.parent / path).read_bytes()))
    samples = {sample['input']['label']: sample for sample in report['physics_contact_probe']['samples']}
    for label in labels:
        sample = samples[label]
        mesh = sample['other_geometry']
        shape = sample['geometry']
        if mesh['kind'] != 'trimesh' or shape['kind'] not in ('box', 'capsule'):
            raise ValueError('Fixture requires a box or capsule against a triangle mesh')
        for geometry in (mesh, shape):
            local = geometry['shape_to_body']
            if vector(local['translation']) != [0, 0, 0] or vector(local['scale']) != [1, 1, 1]:
                raise ValueError('Fixture requires identity local shape transforms')
            if [local['rotation'][axis] for axis in 'xyzw'] != [0, 0, 0, 1]:
                raise ValueError('Fixture requires identity local shape rotation')
        pose = {key: sample['input'][key] for key in (
            'label', 'mesh_asset_path', 'cull_distance_cm', 'body_translation_cm', 'body_rotation_xyzw',
            'mesh_translation_cm', 'mesh_rotation_xyzw',
        )}
        cases.append({'input': pose, 'geometry': shape, 'scale': vector(mesh['trimesh_core_scale']),
                      'points': sample['points']})

(root / 'mesh-contact-cases.json').write_text(json.dumps(cases, indent=2) + '\n')
