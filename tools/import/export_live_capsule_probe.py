import argparse
import gzip
import json
from pathlib import Path

from export_scaled_body_mass_probe import convert as convert_mass


def convert(source):
    if not source['physics_body_probe']['rescale_from_unit']:
        raise ValueError('Expected a live body-scale probe')
    result = convert_mass(source)
    for sample in result['samples']:
        scale = sample['scale'][0]
        if sample['scale'] != [scale, scale, scale]:
            raise ValueError('Expected uniform source scale')
        capsules = []
        for body in source['physics_asset']['bodies']:
            for shape in body['shapes']:
                if shape['kind'] != 'capsule':
                    continue
                half_height = (shape['length'] * 0.5 + shape['radius']) * scale
                radius = max(min(max(shape['radius'] * scale, 0.1), half_height), 0.0001)
                half_length = max(half_height - radius, 0.0001)
                capsules.append({'label': f"{body['bone_name']}_{shape['index']}_capsule",
                                 'radius_m': radius * 0.01, 'half_length_m': half_length * 0.01})
        sample['capsules'] = capsules
    return result


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
