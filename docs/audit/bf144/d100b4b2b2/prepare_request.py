import copy
import gzip
import json
from pathlib import Path


def load_samples(path):
    return json.loads(gzip.decompress(path.read_bytes()))['physics_contact_probe']['samples']


def make_request(sample, label):
    if sample['geometry']['kind'] != 'box' or sample['other_geometry']['kind'] != 'trimesh':
        raise ValueError('Source pose must be a box against a triangle mesh')
    request = {key: value for key, value in sample['input'].items()
               if key not in ('next', 'placement_gap_cm', 'history')}
    request['label'] = label
    request['triangle_indices'] = list(dict.fromkeys(point['face_index'] for point in sample['points']))
    if not request['triangle_indices'] or min(request['triangle_indices']) < 0:
        raise ValueError('Source pose must expose triangle indices')
    return request


root = Path(__file__).parent
restore = load_samples(root.parent / 'd104b3b' / 'restore.native.json.gz')
mesh = load_samples(root.parent / 'd100b' / 'mesh.native.json.gz')
cart = next(sample for sample in restore if sample['input']['label'] == 'SM_Als_Cart_foot_shift_0.2')
stairs = next(sample for sample in restore if sample['input']['label'] == 'SM_Als_Stairs_02_foot_shift_0.2')
rock = next(sample for sample in mesh if sample['input']['label'] == 'SM_Als_Rock_shape_0_foot_l_scale_1_gap_-0.1')
front = make_request(cart, 'cart_box_production')
back = copy.deepcopy(front)
back['label'] = 'cart_box_backfaces'
back['body_translation_cm'][2] -= 13.0
back['triangle_indices'] = [988, 989]
samples = [front, make_request(stairs, 'stairs_box_production'), make_request(rock, 'rock_box_production'), back]
(root / 'box-triangles.request.json').write_text(json.dumps({'samples': samples}, indent=2) + '\n')
