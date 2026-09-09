import argparse
import json
import math
import sys
from pathlib import Path

from coordinate_basis import SOURCE_LENGTH_TO_RUNTIME_SCALE
from gen_als_ragdoll_settings import literal, method

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools/animgraph_authoring'))
from ue_animation_asset_resolver import asset_property
from ue_animgraph_converter import content_root_for_uasset, read_uasset_source


def runtime_thresholds(settings, cpp):
    if settings.get('asset_class') != '/Script/ALS.AlsCharacterSettings':
        raise ValueError('source is not ALS character settings')
    values = (
        literal(method(cpp, 'RefreshLocomotion'), 'HasSpeedThreshold'),
        float(asset_property(settings, 'MovingSpeedThreshold')),
    )
    if any(not math.isfinite(value) or value <= 0 for value in values):
        raise ValueError('invalid locomotion speed threshold')
    return tuple(value * SOURCE_LENGTH_TO_RUNTIME_SCALE for value in values)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--uasset', type=Path)
    parser.add_argument('--content-root', type=Path)
    parser.add_argument('--source-cpp', type=Path)
    parser.add_argument('--source-reader', type=Path, default=ROOT / 'tools/run_ue_source_reader.ps1')
    parser.add_argument('--output', type=Path, default=ROOT / 'crates/als-core/data/locomotion_thresholds.rs')
    parser.add_argument('--report', type=Path, default=ROOT / 'target/locomotion_thresholds.report.json')
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    fixture = {'asset_class': '/Script/ALS.AlsCharacterSettings', 'asset_properties': [
        {'name': 'MovingSpeedThreshold', 'exported_text': '50.0'}]}
    cpp = 'void AAlsCharacter::RefreshLocomotion(float dt)\n{\nstatic constexpr auto HasSpeedThreshold{1.0f};\n}'
    assert runtime_thresholds(fixture, cpp) == (0.01, 0.5)
    fixture['asset_properties'][0]['exported_text'] = 'nan'
    try:
        runtime_thresholds(fixture, cpp)
    except ValueError:
        pass
    else:
        raise AssertionError('non-finite threshold accepted')
    if args.self_test:
        print('Locomotion threshold conversion self-test PASS', flush=True)
        return
    report = {'success': False}
    try:
        if args.uasset is None:
            raise ValueError('character settings uasset is required')
        content = content_root_for_uasset(args.uasset, args.content_root)
        print('Read character locomotion settings', flush=True)
        settings, _ = read_uasset_source(args.uasset, content, '/ALS', args.source_reader, 180, 90)
        cpp_path = args.source_cpp or content.parent / 'Source/ALS/Private/AlsCharacter.cpp'
        values = runtime_thresholds(settings, cpp_path.read_text(encoding='utf-8-sig'))
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(f'({values[0]!r}, {values[1]!r})\n', encoding='utf-8')
        report.update(success=True, source_settings=settings['asset_path'], runtime_thresholds=values)
    except (ValueError, OSError, KeyError, RuntimeError) as error:
        report['error'] = str(error)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(report), flush=True)
    if not report['success']:
        raise SystemExit(1)


if __name__ == '__main__':
    main()
