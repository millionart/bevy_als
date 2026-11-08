import argparse
import gzip
import json
from pathlib import Path

from coordinate_basis import source_direction_to_runtime, source_rotation_to_runtime


def xyz(value):
    return [value[axis] for axis in ('x', 'y', 'z')]


def convert(source):
    samples = []
    for sample in source['physics_body_probe']['samples']:
        bodies = []
        for body in sample['bodies']:
            rotation = body['mass_space_local']['rotation']
            principal = source_direction_to_runtime(xyz(body['principal_inertia_kg_cm2']))
            bodies.append({
                'bone_name': body['bone_name'],
                'principal_kg_m2': [value * 0.0001 for value in principal],
                'rotation_xyzw': source_rotation_to_runtime([rotation[axis] for axis in ('x', 'y', 'z', 'w')]),
            })
        samples.append({'scale': source_direction_to_runtime(xyz(sample['scale'])), 'bodies': bodies})
    return {'source_asset': source['asset_path'], 'samples': samples}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.source.resolve() == args.output.resolve():
        raise ValueError('Target output must differ from source')
    opener = gzip.open if args.source.suffix == '.gz' else open
    with opener(args.source, 'rt', encoding='utf-8-sig') as source:
        result = convert(json.load(source))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')


if __name__ == '__main__':
    main()
