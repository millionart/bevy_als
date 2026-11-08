import argparse
import json
import re
from pathlib import Path

from coordinate_basis import SOURCE_LENGTH_TO_RUNTIME_SCALE


def threshold(facts):
    matches = [re.search(r'#define\s+UE_SMALL_NUMBER\s+\(([^)]+)\)', item['text']) for item in facts]
    matches = [match for match in matches if match]
    if len(matches) != 1:
        raise ValueError('Expected one source small-number definition')
    source = float(matches[0].group(1).rstrip('fF'))
    return source * SOURCE_LENGTH_TO_RUNTIME_SCALE ** 2


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--facts', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.facts.resolve() == args.output.resolve():
        raise ValueError('Target output must differ from source')
    value = threshold(json.loads(args.facts.read_text(encoding='utf-8')))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(f'{value:.9g}_f32\n', encoding='utf-8')


if __name__ == '__main__':
    main()
