import argparse
import json
import math
from pathlib import Path

from coordinate_basis import SOURCE_LENGTH_TO_RUNTIME_SCALE
from import_movement_constants import NUMBER, literal


def parameters(facts, native):
    if native['p.Chaos.Collision.Manifold.EnableFrictionRestore'] != 'true' or native['p.Chaos.Collision.SimpleAssignContact'] != 'true':
        raise ValueError('Unsupported source contact history mode')
    exact = float(native['p.Chaos.Collision.Manifold.FrictionExactPositionTolerance']) * SOURCE_LENGTH_TO_RUNTIME_SCALE
    near = float(native['p.Chaos.Collision.Manifold.FrictionNearPositionTolerance']) * SOURCE_LENGTH_TO_RUNTIME_SCALE
    if not all(math.isfinite(value) for value in (exact, near)) or not 0.0 <= exact <= near:
        raise ValueError('Invalid source contact history distances')
    source = '\n'.join(item['text'] for item in facts)
    ratio = literal(source, rf'#define\s+UE_KINDA_SMALL_NUMBER\s+\({NUMBER}f?\)', 'friction ratio')
    return exact, near, ratio


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--facts', type=Path, required=True)
    parser.add_argument('--native', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.output.resolve() in (args.facts.resolve(), args.native.resolve()):
        raise ValueError('Target output must differ from source')
    values = parameters(json.loads(args.facts.read_text()), json.loads(args.native.read_text()))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text('[' + ', '.join(f'{value:.9g}_f32' for value in values) + ']\n', encoding='utf-8')


if __name__ == '__main__':
    main()
