import argparse
import json
import math
from pathlib import Path

from import_movement_constants import NUMBER, literal


def parameters(facts, settings):
    source = '\n'.join(item['text'] for item in facts)
    denominator = literal(source, rf'FakeDT\s*=\s*\(FReal\)1\.\s*/\s*\(FReal\){NUMBER}\s*;', 'sleep force prediction time')
    rate = float(settings['smooth_rate'])
    if not math.isfinite(rate):
        raise ValueError('Invalid source sleep smoothing rate')
    return 1.0 / denominator, max(0.0, min(1.0, rate))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--facts', type=Path, required=True)
    parser.add_argument('--settings', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.output.resolve() in (args.facts.resolve(), args.settings.resolve()):
        raise ValueError('Target output must differ from source')
    time, rate = parameters(json.loads(args.facts.read_text()), json.loads(args.settings.read_text()))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(f'[{time:.9g}_f32, {rate:.9g}_f32]\n', encoding='utf-8')


if __name__ == '__main__':
    main()
