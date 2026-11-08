import argparse
import json
from pathlib import Path

from coordinate_basis import SOURCE_LENGTH_TO_RUNTIME_SCALE
from import_movement_constants import NUMBER, literal


def tolerances(facts):
    source = literal('\n'.join(item['text'] for item in facts),
                     rf'#define\s+UE_KINDA_SMALL_NUMBER\s+\({NUMBER}f?\)', 'sleep velocity tolerance')
    return source * SOURCE_LENGTH_TO_RUNTIME_SCALE, source


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--facts', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.facts.resolve() == args.output.resolve():
        raise ValueError('Target output must differ from source')
    linear, angular = tolerances(json.loads(args.facts.read_text(encoding='utf-8')))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(f'[{linear:.9g}_f32, {angular:.9g}_f32]\n', encoding='utf-8')


if __name__ == '__main__':
    main()
