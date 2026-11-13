import argparse
import json
import sys
from pathlib import Path

from coordinate_basis import SOURCE_LENGTH_TO_RUNTIME_SCALE

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools/animgraph_authoring'))
from ue_animation_asset_resolver import asset_property, float_curve_fixture, native_float_curve, native_source_asset, source_object_path, source_uasset_path
from ue_animgraph_converter import content_root_for_uasset, read_uasset_source
from ue_exported_text import field, parse_number, parse_struct


def runtime_curve(source, input_scale, value_scale):
    keys, pre, post = native_float_curve(source)
    return project_runtime_curve(keys, pre, post, input_scale, value_scale)


def project_runtime_curve(keys, pre, post, input_scale, value_scale):
    if len(keys) < 2:
        raise ValueError('source curve key times are invalid')
    if any(key['tangent_weight_mode'] != 'None' for key in keys):
        raise ValueError('weighted setting curves require tangent weight conversion')
    if input_scale < 0 and any(key['interpolation'] != 'Linear' for key in keys):
        raise ValueError('reversed setting curves require linear interpolation')
    extrapolations = {'RCCE_Constant': 'Constant', 'RCCE_CycleWithOffset': 'CycleWithOffset'}
    pre = extrapolations[pre]
    post = extrapolations[post]
    if input_scale < 0:
        keys.reverse()
        pre, post = post, pre
    tangent_modes = {'Auto': 0, 'User': 1, 'Break': 2, 'None': 3, 'SmartAuto': 4}
    slope_scale = value_scale / input_scale
    return {
        'times': [key['t'] * input_scale for key in keys],
        'values': [key['v'] * value_scale for key in keys],
        'interps': [key['interpolation'] for key in keys],
        'tangent_modes': [tangent_modes[key['tangent_mode']] for key in keys],
        'weight_modes': [0] * len(keys),
        'arrive_tangents': [key['arrive_tangent' if input_scale > 0 else 'leave_tangent'] * slope_scale for key in keys],
        'leave_tangents': [key['leave_tangent' if input_scale > 0 else 'arrive_tangent'] * slope_scale for key in keys],
        'arrive_weights': [0.0] * len(keys),
        'leave_weights': [0.0] * len(keys),
        'pre_infinity': pre,
        'post_infinity': post,
    }


def velocity_blend_half_life(grounded):
    return parse_number(field(grounded, 'VelocityBlendInterpolationHalfLife'))


def self_test():
    source = float_curve_fixture([{'Time':-60.0, 'Value':-30.0}, {'Time':60.0, 'Value':30.0}],
                                 pre='RCCE_CycleWithOffset', post='RCCE_CycleWithOffset')
    raw = json.dumps(source)
    yaw = runtime_curve(source, 1.0, 1.0)
    assert yaw['times'] == [-60.0, 60.0] and yaw['values'] == [-30.0, 30.0]
    assert yaw['pre_infinity'] == 'CycleWithOffset' and json.dumps(source) == raw
    source = float_curve_fixture([{'InterpMode':'RCIM_Cubic','LeaveTangent':0.5},
                                  {'Time':100.0,'Value':1.0,'ArriveTangent':0.25}])
    stride = runtime_curve(source, SOURCE_LENGTH_TO_RUNTIME_SCALE, 1.0)
    assert stride['times'] == [0.0, 1.0]
    assert stride['leave_tangents'][0] == 50.0 and stride['arrive_tangents'][1] == 25.0
    try:
        runtime_curve(source, -1.0, -1.0)
    except ValueError:
        pass
    else:
        raise AssertionError('reversed cubic curve was accepted')
    for value in ('0.0', '0.1', '0.4', '-0.2'):
        grounded = parse_struct('(VelocityBlendInterpolationHalfLife=' + value + ')')
        assert velocity_blend_half_life(grounded) == float(value)
    print('Standing curve conversion self-test PASS', flush=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--uasset', type=Path)
    parser.add_argument('--content-root', type=Path)
    parser.add_argument('--mount', default='/ALS')
    parser.add_argument('--source-reader', type=Path, default=ROOT / 'tools/run_ue_source_reader.ps1')
    parser.add_argument('--output', type=Path, default=ROOT / 'assets/ALS/Data/AnimationInstance/Grounded/als_standing_curves.json')
    parser.add_argument('--report', type=Path, default=ROOT / 'target/als_standing_curves.report.json')
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return
    report = {'format': 'als_standing_curves_v1', 'success': False}
    try:
        if args.uasset is None:
            raise ValueError('animation instance settings uasset is required')
        content_root = content_root_for_uasset(args.uasset, args.content_root)
        print('Read animation instance settings', flush=True)
        settings, _ = read_uasset_source(args.uasset, content_root, args.mount, args.source_reader, 300, 150)
        if settings.get('asset_class') != '/Script/ALS.AlsAnimationInstanceSettings':
            raise ValueError('source is not ALS animation instance settings')
        grounded = parse_struct(asset_property(settings, 'Grounded'))
        standing = parse_struct(asset_property(settings, 'Standing'))
        if field(grounded, 'RotationYawOffsetForwardCurve') != field(grounded, 'RotationYawOffsetBackwardCurve'):
            raise ValueError('forward and backward curves differ')
        output = {}
        references = {}
        for name, fields, key, scale, value_scale in [
            ('stride_walk', standing, 'StrideBlendAmountWalkCurve', SOURCE_LENGTH_TO_RUNTIME_SCALE, 1.0),
            ('stride_run', standing, 'StrideBlendAmountRunCurve', SOURCE_LENGTH_TO_RUNTIME_SCALE, 1.0),
            ('yaw_forward_backward', grounded, 'RotationYawOffsetForwardCurve', 1.0, 1.0),
            ('yaw_left', grounded, 'RotationYawOffsetLeftCurve', 1.0, 1.0),
            ('yaw_right', grounded, 'RotationYawOffsetRightCurve', 1.0, 1.0),
        ]:
            reference = source_object_path(field(fields, key))
            source_uasset_path(content_root, args.mount, reference)
            print('Read ' + reference, flush=True)
            output[name] = runtime_curve(native_source_asset(reference, args.source_reader), scale, value_scale)
            references[name] = reference
        output['velocity_blend_interpolation_half_life'] = velocity_blend_half_life(grounded)
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
