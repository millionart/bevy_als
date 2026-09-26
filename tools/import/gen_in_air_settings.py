import argparse
import json
import re
from pathlib import Path
from unittest.mock import patch

from gen_standing_curves import ROOT, runtime_curve
from coordinate_basis import SOURCE_LENGTH_TO_RUNTIME_SCALE
from ue_animation_asset_resolver import asset_property, native_source_asset, source_object_path
from ue_animgraph_converter import content_root_for_uasset, read_uasset_source
from ue_exported_text import field, parse_number, parse_struct


def generate(settings, source_reader, animation_source):
    if settings.get('asset_class') != '/Script/ALS.AlsAnimationInstanceSettings':
        raise ValueError('source is not ALS animation instance settings')
    general = parse_struct(asset_property(settings, 'General'))
    air = parse_struct(asset_property(settings, 'InAir'))
    output = {'lean_interpolation_half_life': parse_number(field(general, 'LeanInterpolationHalfLife'))}
    responses = parse_struct(field(air, 'GroundPredictionSweepResponses'))
    if field(air, 'GroundPredictionSweepChannel') != 'ECC_Visibility' or responses != {
        'WorldStatic': 'ECR_Block', 'WorldDynamic': 'ECR_Block', 'Destructible': 'ECR_Block',
    }:
        raise ValueError('source ground prediction collision responses are unsupported')
    output['prediction_trace_layer'] = 'Visibility'
    for name, function, constant, scale in [
        ('jump_reference_speed', 'RefreshInAir', 'ReferenceSpeed', SOURCE_LENGTH_TO_RUNTIME_SCALE),
        ('jump_min_play_rate', 'RefreshInAir', 'MinPlayRate', 1.0),
        ('jump_max_play_rate', 'RefreshInAir', 'MaxPlayRate', 1.0),
        ('lean_reference_speed', 'RefreshInAirLean', 'ReferenceSpeed', SOURCE_LENGTH_TO_RUNTIME_SCALE),
        ('prediction_velocity_threshold', 'RefreshGroundPrediction', 'VerticalVelocityThreshold', SOURCE_LENGTH_TO_RUNTIME_SCALE),
        ('prediction_min_velocity', 'RefreshGroundPrediction', 'MinVerticalVelocity', SOURCE_LENGTH_TO_RUNTIME_SCALE),
        ('prediction_max_velocity', 'RefreshGroundPrediction', 'MaxVerticalVelocity', SOURCE_LENGTH_TO_RUNTIME_SCALE),
        ('prediction_min_distance', 'RefreshGroundPrediction', 'MinSweepDistance', SOURCE_LENGTH_TO_RUNTIME_SCALE),
        ('prediction_max_distance', 'RefreshGroundPrediction', 'MaxSweepDistance', SOURCE_LENGTH_TO_RUNTIME_SCALE),
    ]:
        scope = animation_source.split('void UAlsAnimationInstance::' + function + '()', 1)
        matches = re.findall(r'\b' + constant + r'\{(-?[0-9.]+)f\}', scope[1].split('\nvoid ', 1)[0]) if len(scope) == 2 else []
        if len(matches) != 1:
            raise ValueError('source in-air constant is missing or ambiguous: ' + name)
        output[name] = float(matches[0]) * scale
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
        {'name': 'InAir', 'exported_text': '(LeanAmountCurve="/Script/Engine.CurveFloat\'/Test/Lean.Lean\'",GroundPredictionAmountCurve="/Script/Engine.CurveFloat\'/Test/Prediction.Prediction\'",GroundPredictionSweepChannel=ECC_Visibility,GroundPredictionSweepResponses=(WorldStatic=ECR_Block,WorldDynamic=ECR_Block,Destructible=ECR_Block))'},
    ]}
    curve = {'format': 'ue_asset_source_v1', 'asset_class': '/Script/Engine.CurveFloat', 'asset_properties': [
        {'name': 'FloatCurve', 'exported_text': '(Keys=((InterpMode=RCIM_Cubic,Time=-100,LeaveTangent=0.01),(Value=1,ArriveTangent=0.02)),DefaultValue=0)'},
    ]}
    raw = json.dumps([settings, curve])
    source = 'void UAlsAnimationInstance::RefreshInAir()\n{ ReferenceSpeed{600.0f}; MinPlayRate{1.2f}; MaxPlayRate{1.5f}; }\nvoid UAlsAnimationInstance::RefreshInAirLean()\n{ ReferenceSpeed{350.0f}; }\nvoid UAlsAnimationInstance::RefreshGroundPrediction()\n{ VerticalVelocityThreshold{-200.0f}; MinVerticalVelocity{-4000.0f}; MaxVerticalVelocity{-200.0f}; MinSweepDistance{150.0f}; MaxSweepDistance{2000.0f}; }'
    with patch(__name__ + '.native_source_asset', return_value=curve) as reader:
        output, references = generate(settings, Path('reader.ps1'), source)
    assert reader.call_count == 2 and len(references) == 2
    assert output['prediction_trace_layer'] == 'Visibility'
    assert output['lean_interpolation_half_life'] == 0.2
    assert output['jump_reference_speed'] == 6.0 and output['lean_reference_speed'] == 3.5
    assert [output['prediction_min_distance'], output['prediction_max_distance']] == [1.5, 20.0]
    assert [output['prediction_min_velocity'], output['prediction_max_velocity']] == [-40.0, -2.0]
    assert output['lean']['times'] == [-1.0, 0.0]
    assert output['lean']['leave_tangents'][0] == 1.0
    assert output['lean']['arrive_tangents'][1] == 2.0
    assert output['ground_prediction']['times'] == [-100.0, 0.0]
    assert json.dumps([settings, curve]) == raw
    print('In-air setting conversion self-test PASS', flush=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--uasset', type=Path)
    parser.add_argument('--animation-source', type=Path)
    parser.add_argument('--source-reader', type=Path, default=ROOT / 'tools/run_ue_source_reader.ps1')
    parser.add_argument('--output', type=Path, default=ROOT / 'assets/ALS/Data/AnimationInstance/Air/als_in_air_settings.json')
    parser.add_argument('--report', type=Path, default=ROOT / 'target/als_in_air_settings.report.json')
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return
    report = {'format': 'als_in_air_settings_v1', 'success': False}
    try:
        if args.uasset is None or args.animation_source is None:
            raise ValueError('animation instance settings uasset and C++ source are required')
        print('Read animation instance settings', flush=True)
        settings, _ = read_uasset_source(args.uasset, content_root_for_uasset(args.uasset, None), '/ALS', args.source_reader, 300, 150)
        output, references = generate(settings, args.source_reader, args.animation_source.read_text(encoding='utf-8'))
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
