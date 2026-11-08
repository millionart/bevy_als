import argparse
import json
from pathlib import Path

from import_mesh_pawn_response import field


def source_substeps(settings):
    properties = settings['asset_properties']
    for name in ('bSubstepping', 'bTickPhysicsAsync'):
        if field(properties, name)['exported_text'] != 'False':
            raise ValueError(f'Unsupported physics clock mode: {name}')
    return 1


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--settings', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.settings.resolve() == args.output.resolve():
        raise ValueError('Target output must differ from source')
    value = source_substeps(json.loads(args.settings.read_text(encoding='utf-8-sig')))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(f'{value}_u32\n', encoding='utf-8')


if __name__ == '__main__':
    main()
