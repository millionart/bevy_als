import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'animgraph_authoring'))
from ue_animation_asset_resolver import curve_float_contract, native_source_asset


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--asset', required=True)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    result = curve_float_contract(native_source_asset(args.asset))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
    assert curve_float_contract(native_source_asset(args.asset)) == json.loads(args.output.read_text(encoding='utf-8'))
    print(json.dumps({'source_curve': result['source_curve'], 'keys': len(result['keys']), 'result': 'PASS'}))


if __name__ == '__main__':
    main()
