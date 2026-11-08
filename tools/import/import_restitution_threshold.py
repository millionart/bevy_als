import argparse
import json
import math
from pathlib import Path

from coordinate_basis import SOURCE_LENGTH_TO_RUNTIME_SCALE
from import_movement_constants import NUMBER, literal


def threshold(facts, native):
    source = '\n'.join(item['text'] for item in facts)
    default = literal(source, rf'DefaultRestitutionThreshold\s*=\s*{NUMBER}\s*;', 'restitution acceleration')
    override = float(native['collision_restitution_threshold_override'])
    if not math.isfinite(override):
        raise ValueError('Invalid restitution threshold override')
    return (override if override >= 0.0 else default) * SOURCE_LENGTH_TO_RUNTIME_SCALE


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--facts', type=Path, required=True)
    parser.add_argument('--native', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.output.resolve() in (args.facts.resolve(), args.native.resolve()):
        raise ValueError('Target output must differ from source')
    value = threshold(json.loads(args.facts.read_text()), json.loads(args.native.read_text()))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(f'{value:.9g}_f32\n', encoding='utf-8')


if __name__ == '__main__':
    main()
