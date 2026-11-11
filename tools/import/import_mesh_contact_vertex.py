import argparse
import json
from pathlib import Path

from import_movement_constants import NUMBER, literal


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--facts', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.facts.resolve() == args.output.resolve():
        raise ValueError('Target output must differ from source')
    text = '\n'.join(item['text'] for item in json.loads(args.facts.read_text()))
    tolerance = literal(text, rf'NormalToleranceSq\s*=\s*FReal\({NUMBER}\)', 'vertex normal tolerance')
    args.output.write_text(f'{tolerance!r}_f64\n')


if __name__ == '__main__':
    main()
