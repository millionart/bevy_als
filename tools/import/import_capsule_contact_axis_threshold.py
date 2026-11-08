import argparse
import json
from pathlib import Path

from import_movement_constants import NUMBER, literal


def threshold(facts):
    source = '\n'.join(item['text'] for item in facts)
    return literal(source, rf'CapsuleEndCapAxisNormalThreshold\s*=\s*FReal\({NUMBER}\)', 'capsule contact axis')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--facts', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.facts.resolve() == args.output.resolve():
        raise ValueError('Target output must differ from source')
    value = threshold(json.loads(args.facts.read_text()))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(f'{value:.9g}_f32\n', encoding='utf-8')


if __name__ == '__main__':
    main()
