import argparse
import json
import re
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
    if not re.search(r'ContactGeneratorSettings\.bUseTwoPassLoop\s*=\s*false\s*;', text):
        raise ValueError('Source quadratic shape must use one pass')
    threshold = literal(text, rf'ContactGeneratorSettings\.FaceNormalDotThreshold\s*=\s*{NUMBER}\s*;',
                        'quadratic face normal threshold')
    if threshold > 1:
        raise ValueError('Source quadratic face normal threshold is invalid')
    args.output.write_text(f'{threshold!r}_f64\n')


if __name__ == '__main__':
    main()
