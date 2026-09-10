import argparse
import json
from pathlib import Path
from unittest.mock import patch

from gen_standing_curves import ROOT, runtime_curve
from coordinate_basis import SOURCE_LENGTH_TO_RUNTIME_SCALE
from ue_animation_asset_resolver import asset_property, native_source_asset, source_object_path
from ue_animgraph_converter import content_root_for_uasset, read_uasset_source
from ue_exported_text import field, parse_number, parse_struct


def generate(settings, source_reader):
    if settings.get('asset_class') != '/Script/ALS.AlsAnimationInstanceSettings':
        raise ValueError('source is not ALS animation instance settings')
    general = parse_struct(asset_property(settings, 'General'))
    air = parse_struct(asset_property(settings, 'InAir'))
    output = {'lean_interpolation_half_life': parse_number(field(general, 'LeanInterpolationHalfLife'))}
    references = {}
    for name, key, scale in [
        ('lean', 'LeanAmountCurve', SOURCE_LENGTH_TO_RUNTIME_SCALE),
        ('ground_prediction', 'GroundPredictionAmountCurve', 1.0),
    ]:
        reference = source_object_path(field(air, key))
        print('Read ' + reference, flush=True)
        output[name] = runtime_curve(native_source_asset(reference, source_reader), scale, 1.0)
        references[name] = reference
    return output, references


def self_test():
    settings = {'asset_class': '/Script/ALS.AlsAnimationInstanceSettings', 'asset_properties': [
        {'name': 'General', 'exported_text': '(LeanInterpolationHalfLife=0.2)'},
        {'name': 'InAir', 'exported_text': '(LeanAmountCurve="/Script/Engine.CurveFloat\'/Test/Lean.Lean\'",GroundPredictionAmountCurve="/Script/Engine.CurveFloat\'/Test/Prediction.Prediction\'")'},
    ]}
    curve = {'format': 'ue_asset_source_v1', 'asset_class': '/Script/Engine.CurveFloat', 'asset_properties': [
        {'name': 'FloatCurve', 'exported_text': '(Keys=((InterpMode=RCIM_Cubic,Time=-100,LeaveTangent=0.01),(Value=1,ArriveTangent=0.02)),DefaultValue=0)'},
    ]}
    raw = json.dumps([settings, curve])
    with patch(__name__ + '.native_source_asset', return_value=curve) as reader:
        output, references = generate(settings, Path('reader.ps1'))
    assert reader.call_count == 2 and len(references) == 2
    assert output['lean_interpolation_half_life'] == 0.2
    assert output['lean']['times'] == [-1.0, 0.0]
    assert output['lean']['leave_tangents'][0] == 1.0
    assert output['lean']['arrive_tangents'][1] == 2.0
    assert output['ground_prediction']['times'] == [-100.0, 0.0]
    assert json.dumps([settings, curve]) == raw
    print('In-air setting conversion self-test PASS', flush=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--uasset', type=Path)
    parser.add_argument('--source-reader', type=Path, default=ROOT / 'tools/run_ue_source_reader.ps1')
    parser.add_argument('--output', type=Path, default=ROOT / 'assets/character/als_in_air_settings.json')
    parser.add_argument('--report', type=Path, default=ROOT / 'target/als_in_air_settings.report.json')
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return
    report = {'format': 'als_in_air_settings_v1', 'success': False}
    try:
        if args.uasset is None:
            raise ValueError('animation instance settings uasset is required')
        print('Read animation instance settings', flush=True)
        settings, _ = read_uasset_source(args.uasset, content_root_for_uasset(args.uasset, None), '/ALS', args.source_reader, 300, 150)
        output, references = generate(settings, args.source_reader)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(output, indent=2) + '\n', encoding='utf-8')
        report.update(success=True, source_settings=settings['asset_path'], source_curves=references)
    except (ValueError, OSError, KeyError, RuntimeError) as error:
        report['error'] = str(error)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(report), flush=True)
    if not report['success']:
        raise SystemExit(1)


if __name__ == '__main__':
    main()
