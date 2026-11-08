import argparse
import gzip
import json
from pathlib import Path

from coordinate_basis import source_point_to_runtime
from export_body_inertia_probe import convert as convert_inertia, xyz


def convert(source):
    result = convert_inertia(source)
    result['rescale_from_unit'] = source['physics_body_probe']['rescale_from_unit']
    for original, converted in zip(source['physics_body_probe']['samples'], result['samples']):
        for body, output in zip(original['bodies'], converted['bodies']):
            output['mass_kg'] = body['mass_kg']
            output['center_m'] = source_point_to_runtime(xyz(body['mass_space_local']['translation']))
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
