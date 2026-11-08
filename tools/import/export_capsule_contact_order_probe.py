import argparse
import gzip
import json
import math
from pathlib import Path

from coordinate_basis import source_direction_to_runtime, source_point_to_runtime
from gen_ragdoll import quat_rotate


def components(value):
    return [value['x'], value['y'], value['z']]


def convert(probe, profile, sample_name, bone_name, profile_path):
    sample = next(item for item in probe['physics_contact_probe']['samples'] if item['input']['label'] == sample_name)
    geometry = sample['geometry']
    if geometry['kind'] != 'capsule' or len(sample['points']) < 2:
        raise ValueError('Expected a capsule contact with at least two points')
    first = source_direction_to_runtime(components(geometry['endpoint_0_cm']))
    second = source_direction_to_runtime(components(geometry['endpoint_1_cm']))
    axis = [a - b for a, b in zip(first, second)]
    length = math.sqrt(sum(value * value for value in axis))
    if length == 0.0:
        raise ValueError('Capsule axis must be nonzero')
    axis = [value / length for value in axis]
    body = next(item for item in profile['bodies'] if item['bone_name'] == bone_name)
    shape = next(item['target'] for item in body['shapes'] if item['target']['kind'] == 'capsule')
    target = quat_rotate(shape['rotation_xyzw'], [0.0, -1.0, 0.0])
    return {'source_endpoint_0_minus_1_runtime_direction': axis,
            'target_local_negative_y_world_direction': target,
            'dot': sum(a * b for a, b in zip(axis, target)),
            'target_profile': profile_path, 'native_sample': sample_name,
            'source_points_body_m': [source_point_to_runtime(components(point['shape_point_0_cm'])) for point in sample['points']],
            'source_normal_runtime': source_direction_to_runtime(components(sample['points'][0]['world_normal']))}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--profile', type=Path, required=True)
    parser.add_argument('--sample', required=True)
    parser.add_argument('--bone', required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.output.resolve() in (args.source.resolve(), args.profile.resolve()):
        raise ValueError('Target output must differ from source')
    opener = gzip.open if args.source.suffix == '.gz' else open
    with opener(args.source, 'rt', encoding='utf-8-sig') as source:
        result = convert(json.load(source), json.loads(args.profile.read_text()), args.sample, args.bone, args.profile.as_posix())
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')


if __name__ == '__main__':
    main()
