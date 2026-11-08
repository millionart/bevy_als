import argparse
import gzip
import json
import math
from pathlib import Path


def source_limit(source):
    value = float(source['physics_asset']['constraint_console_variables']['p.Chaos.Joint.DegenerateRotationLimit'])
    if not math.isfinite(value):
        raise ValueError('Joint degeneracy limit must be finite')
    return value


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.source.resolve() == args.output.resolve():
        raise ValueError('Target output must differ from source')
    opener = gzip.open if args.source.suffix == '.gz' else open
    with opener(args.source, 'rt', encoding='utf-8-sig') as source:
        value = source_limit(json.load(source))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(f'{value:.9g}_f32\n', encoding='utf-8')


if __name__ == '__main__':
    main()
