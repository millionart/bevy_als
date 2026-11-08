import argparse
import json
from pathlib import Path

from coordinate_basis import SOURCE_LENGTH_TO_RUNTIME_SCALE
from import_movement_constants import NUMBER, literal


def tolerances(facts):
    source = '\n'.join(item['text'] for item in facts)
    small = literal(source, rf'#define\s+UE_SMALL_NUMBER\s+\({NUMBER}f?\)', 'contact effective mass')
    impulse = literal(source, rf'#define\s+UE_KINDA_SMALL_NUMBER\s+\({NUMBER}f?\)', 'position impulse')
    return small, impulse * SOURCE_LENGTH_TO_RUNTIME_SCALE


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--facts', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.facts.resolve() == args.output.resolve():
        raise ValueError('Target output must differ from source')
    mass, impulse = tolerances(json.loads(args.facts.read_text()))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(f'[{mass:.9g}_f32, {impulse:.9g}_f32]\n', encoding='utf-8')


if __name__ == '__main__':
    main()
