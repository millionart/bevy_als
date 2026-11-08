import argparse
import json
import math
import re
from pathlib import Path

from coordinate_basis import SOURCE_LENGTH_TO_RUNTIME_SCALE
from import_mesh_pawn_response import field


def pushout(settings, native):
    text = field(settings['asset_properties'], 'SolverOptions')['exported_text']
    matches = re.findall(r'\bCollisionMaxPushOutVelocity\s*=\s*(-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?)', text)
    if len(matches) != 1:
        raise ValueError('Expected one source pushout speed')
    default = float(matches[0])
    override = float(native['max_pushout_override'])
    if not math.isfinite(default) or not math.isfinite(override):
        raise ValueError('Invalid source pushout speed')
    value = override if override >= 0.0 else default
    return value * SOURCE_LENGTH_TO_RUNTIME_SCALE if value > 0.0 else math.inf


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--settings', type=Path, required=True)
    parser.add_argument('--native', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.output.resolve() in (args.settings.resolve(), args.native.resolve()):
        raise ValueError('Target output must differ from source')
    value = pushout(json.loads(args.settings.read_text(encoding='utf-8-sig')), json.loads(args.native.read_text()))
    result = 'f32::INFINITY' if math.isinf(value) else f'{value:.9g}_f32'
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(result + '\n', encoding='utf-8')


if __name__ == '__main__':
    main()
