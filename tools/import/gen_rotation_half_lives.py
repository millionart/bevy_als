import argparse
import json
import math
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools/animgraph_authoring'))
from ue_animation_asset_resolver import asset_property, source_curve_float, source_object_path
from ue_animgraph_converter import content_root_for_uasset, read_uasset_source
from ue_exported_text import field, parse_list, parse_quoted, parse_struct


def selected_curves(settings):
    if settings.get('asset_class') != '/Script/ALS.AlsMovementSettings':
        raise ValueError('source movement settings class is invalid')
    selected = {}
    for mode_pair in parse_list(asset_property(settings, 'RotationModes')):
        key, value = parse_list(mode_pair)
        mode = parse_quoted(field(parse_struct(key), 'TagName')).split('.')[-1]
        for stance_pair in parse_list(field(parse_struct(value), 'Stances')):
            key, value = parse_list(stance_pair)
            stance = parse_quoted(field(parse_struct(key), 'TagName')).split('.')[-1]
            selected[(mode, stance)] = source_object_path(field(parse_struct(value), 'RotationInterpolationSpeedCurve'))
    expected = {(mode, stance) for mode in ('VelocityDirection', 'ViewDirection', 'Aiming') for stance in ('Standing', 'Crouching')}
    if set(selected) != expected:
        raise ValueError('source rotation mode or stance selection is incomplete')
    responsive = selected[('VelocityDirection', 'Standing')]
    normals = {value for key, value in selected.items() if key != ('VelocityDirection', 'Standing')}
    if len(normals) != 1 or responsive in normals:
        raise ValueError('source rotation profile selection changed')
    return normals.pop(), responsive


def half_lives(curve):
    keys = curve['keys']
    if (len(keys) != 4 or [key['t'] for key in keys] != [0.0, 1.0, 2.0, 3.0]
            or any(key['interpolation'] != 'Linear' or not math.isfinite(key['v']) or key['v'] <= 0 for key in keys)):
        raise ValueError('source rotation curve needs unsupported interpolation')
    return tuple(key['v'] for key in keys[1:])


def self_test():
    normal = '/Test/Normal.Normal'
    responsive = '/Test/Responsive.Responsive'
    modes = []
    for mode in ('VelocityDirection', 'ViewDirection', 'Aiming'):
        stances = []
        for stance in ('Standing', 'Crouching'):
            curve = responsive if (mode, stance) == ('VelocityDirection', 'Standing') else normal
            stances.append(f'((TagName="Als.Stance.{stance}"), (RotationInterpolationSpeedCurve="/Script/Engine.CurveFloat\'{curve}\'"))')
        modes.append(f'((TagName="Als.RotationMode.{mode}"), (Stances=({",".join(stances)})))')
    settings = {'asset_class': '/Script/ALS.AlsMovementSettings',
                'asset_properties': [{'name': 'RotationModes', 'exported_text': '(' + ','.join(modes) + ')'}]}
    assert selected_curves(settings) == (normal, responsive)
    curve = {'keys': [{'t': float(index), 'v': value, 'interpolation': 'Linear'}
                      for index, value in enumerate((0.15, 0.15, 0.15, 0.05))]}
    assert half_lives(curve) == (0.15, 0.15, 0.05)
    curve['keys'][1]['interpolation'] = 'Cubic'
    try:
        half_lives(curve)
    except ValueError:
        pass
    else:
        raise AssertionError('nonlinear source curve was accepted')
    print('Rotation half-life conversion self-test PASS', flush=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--uasset', type=Path)
    parser.add_argument('--source-reader', type=Path, default=ROOT / 'tools/run_ue_source_reader.ps1')
    parser.add_argument('--output', type=Path, default=ROOT / 'src/als/settings/rotation_half_lives.rs')
    parser.add_argument('--report', type=Path, default=ROOT / 'target/rotation_half_lives.report.json')
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return
    report = {'success': False}
    try:
        if args.uasset is None:
            raise ValueError('movement settings uasset is required')
        content = content_root_for_uasset(args.uasset, None)
        settings, _ = read_uasset_source(args.uasset, content, '/ALS', args.source_reader, 300, 150)
        normal_ref, responsive_ref = selected_curves(settings)
        normal = half_lives(source_curve_float(content, '/ALS', normal_ref, args.source_reader))
        responsive = half_lives(source_curve_float(content, '/ALS', responsive_ref, args.source_reader))
        args.output.parent.mkdir(parents=True, exist_ok=True)
        values = lambda profile: '[' + ', '.join(f'{value!r}f32' for value in profile) + ']'
        args.output.write_text(f'({values(normal)}, {values(responsive)})\n', encoding='utf-8')
        report.update(success=True, source_settings=settings['asset_path'], source_curves=[normal_ref, responsive_ref], half_lives=[normal, responsive])
    except (ValueError, OSError, KeyError, RuntimeError) as error:
        report['error'] = str(error)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(report), flush=True)
    if not report['success']:
        raise SystemExit(1)


if __name__ == '__main__':
    main()
