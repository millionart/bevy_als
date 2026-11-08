import argparse
import hashlib
import json
from pathlib import Path


def matrix(rotation, principal):
    x, y, z, w = rotation
    basis = [
        [1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
        [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
        [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)],
    ]
    return [[sum(basis[i][k] * principal[k] * basis[j][k] for k in range(3))
             for j in range(3)] for i in range(3)]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--native', type=Path, required=True)
    parser.add_argument('--profile', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    native_bytes, profile_bytes = args.native.read_bytes(), args.profile.read_bytes()
    native, profile = json.loads(native_bytes), json.loads(profile_bytes)
    rows = []
    for sample in native['samples']:
        scale = sample['scale'][0]
        for label in ('pelvis', 'spine_03', 'head', 'hand_l', 'hand_r'):
            body = next(item for item in sample['bodies'] if item['bone_name'] == label)
            expected = matrix(body['rotation_xyzw'], body['principal_kg_m2'])
            mass = next(item for item in profile['bodies'] if item['bone_name'] == label)['target']['mass_properties']
            scaled_mass = max(mass['unclamped_mass_kg'] * scale ** mass['scale_exponent'], 0.001)
            factor = scale * scale * scaled_mass / mass['mass_kg']
            target = [[value * factor for value in row] for row in mass['inertia_tensor_kg_m2']]
            error = max(abs(target[i][j] - expected[i][j]) for i in range(3) for j in range(3))
            largest = max(abs(value) for row in expected for value in row)
            rows.append({'scale': scale, 'bone_name': label, 'max_input_error_kg_m2': error,
                         'relative_input_error': error / largest, 'native_xz_kg_m2': expected[0][2]})
    result = {'native_sha256': hashlib.sha256(native_bytes).hexdigest(),
              'profile_sha256': hashlib.sha256(profile_bytes).hexdigest(), 'samples': rows}
    args.output.write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')


if __name__ == '__main__':
    main()
