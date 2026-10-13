import argparse
import json
import math
import re
import sys
from copy import deepcopy
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'animgraph_authoring'))
from ue_animation_asset_resolver import object_property
from ue_exported_text import parse_struct


def cvar(text, name):
    match = re.search(r'TEXT\("' + re.escape(name) + r'"\),\s*(-?[\d.]+)f?\s*,', text)
    if not match:
        raise ValueError('Source console default is missing: ' + name)
    return float(match[1])


def compile_lighting(world, camera, eye_source, view_source, project_settings):
    props = {prop['name']: prop['exported_text'] for prop in camera['blueprint_cdo_properties']}
    if float(props['PostProcessWeight']) != 0.0:
        raise ValueError('Active camera post processing requires its settings closure')
    if cvar(view_source, 'r.DefaultFeature.AutoExposure') != 1 or cvar(view_source, 'r.DefaultFeature.AutoExposure.Method') != 0:
        raise ValueError('Only source histogram exposure is supported')
    if 'r.DefaultFeature.AutoExposure.ExtendDefaultLuminanceRange=True' not in project_settings:
        raise ValueError('Source extended luminance range is required')
    volumes = [obj for obj in world['asset_objects'] if obj['class_path'] == '/Script/Engine.PostProcessVolume']
    skies = [obj for obj in world['asset_objects'] if obj['class_path'] == '/Script/Engine.SkyLightComponent']
    if len(volumes) != 1 or len(skies) != 1:
        raise ValueError('Source lighting owners are missing or ambiguous')
    settings = parse_struct(object_property(volumes[0], 'Settings'))
    if any(value == 'True' for name, value in settings.items() if name.startswith('bOverride_') and name != 'bOverride_MotionBlurAmount'):
        raise ValueError('Additional source post process overrides require mapping')
    if settings.get('AutoExposureBiasCurve', 'None') != 'None' or settings.get('AutoExposureMeterMask', 'None') != 'None':
        raise ValueError('Source exposure bias curve requires its asset closure')
    lens_max = 0.78 / max(cvar(eye_source, 'r.EyeAdaptation.LensAttenuation'), 0.01)
    histogram = [float(settings['HistogramLogMin']), float(settings['HistogramLogMax'])]
    exposure_bounds = [float(settings['AutoExposureMinBrightness']), float(settings['AutoExposureMaxBrightness'])]
    if histogram != exposure_bounds or histogram[0] >= histogram[1]:
        raise ValueError('Separate source histogram and exposure clamps require mapping')
    contrast = parse_struct(settings['ColorContrast'])
    if len({float(contrast[channel]) for channel in 'XYZW'}) != 1:
        raise ValueError('Per-channel source contrast requires mapping')
    grey = re.search(r'const float kMiddleGrey = ([\d.]+)f;', eye_source)
    if not grey:
        raise ValueError('Source metering gray is missing')
    gray = float(grey[1])
    if cvar(view_source, 'r.DefaultFeature.Bloom') != 1 or settings.get('BloomMethod', 'BM_SOG') != 'BM_SOG':
        raise ValueError('Source Gaussian bloom is required')
    if float(settings['BloomThreshold']) != -1.0:
        raise ValueError('Source bloom threshold requires exposure-space mapping')
    tints = [parse_struct(settings[f'Bloom{level}Tint']) for level in range(1, 7)]
    if any(len({float(tint[channel]) for channel in 'RGB'}) != 1 for tint in tints):
        raise ValueError('Colored source bloom requires per-channel scattering')
    gain = float(settings['BloomIntensity']) * float(settings['BloomGaussianIntensity']) * sum(float(tint['R']) for tint in tints) / 6
    native_mips = int(math.log2(512)) - 1
    lower, upper = 0.0, 1.0
    for _ in range(64):
        intensity = (lower + upper) * 0.5
        if sum(intensity ** level for level in range(1, native_mips + 1)) < gain:
            lower = intensity
        else:
            upper = intensity
    sky = skies[0]
    if object_property(sky, 'AttachParent') != 'None' or object_property(sky, 'SourceType') != 'SLS_CapturedScene':
        raise ValueError('Source sky capture space or mode requires mapping')
    location = parse_struct(object_property(sky, 'RelativeLocation'))
    return {
        'format': 'playground_lighting_v1',
        'source_world': world['asset_path'],
        'exposure': {
            'histogram_log_luminance_range': [value + math.log2(lens_max) for value in histogram],
            'filter': [min(max(float(settings['AutoExposureLowPercent']), 1.0), min(max(float(settings['AutoExposureHighPercent']), 1.0), 99.0)) * 0.01,
                       min(max(float(settings['AutoExposureHighPercent']), 1.0), 99.0) * 0.01],
            'speed_brighten': float(settings['AutoExposureSpeedUp']),
            'speed_darken': float(settings['AutoExposureSpeedDown']),
            'exponential_transition_distance': cvar(eye_source, 'r.EyeAdaptation.ExponentialTransitionDistance'),
            'compensation_ev': float(settings['AutoExposureBias']) + math.log2(gray),
            'unit_exposure_ev100': -math.log2(1.2),
            'contrast': float(contrast['X']),
        },
        'bloom': {
            'intensity': (lower + upper) * 0.5,
            'integrated_scatter_gain': gain,
            'threshold': 0.0,
            'max_mip_dimension': 512,
        },
        'sky_light': {
            'intensity': float(object_property(sky, 'Intensity')),
            'capture_resolution': int(object_property(sky, 'CubemapResolution')),
            'capture_origin': [float(location['Y']) * 0.01, float(location['Z']) * 0.01, -float(location['X']) * 0.01],
            'sky_distance_threshold': float(object_property(sky, 'SkyDistanceThreshold')) * 0.01,
            'lower_hemisphere_black': object_property(sky, 'bLowerHemisphereIsBlack') == 'True',
            'real_time_capture': object_property(sky, 'bRealTimeCapture') == 'True',
        },
    }


def self_test(world, camera, eye_source, view_source, project_settings):
    args = (world, camera, eye_source, view_source, project_settings)
    result = compile_lighting(*args)
    exposure = result['exposure']
    assert abs(2 ** -exposure['unit_exposure_ev100'] / 1.2 - 1.0) < 1e-12
    for luminance in [0.01, 0.1, 1.0, 10.0]:
        assert abs(luminance * 2 ** (exposure['compensation_ev'] - math.log2(luminance)) - 0.36) < 1e-12
    changed = deepcopy(camera)
    next(prop for prop in changed['blueprint_cdo_properties'] if prop['name'] == 'PostProcessWeight')['exported_text'] = '1.0'
    try:
        compile_lighting(world, changed, eye_source, view_source, project_settings)
    except ValueError:
        pass
    else:
        raise AssertionError('Active source camera override was ignored')
    bloom = result['bloom']
    assert abs(sum(bloom['intensity'] ** level for level in range(1, 9)) - bloom['integrated_scatter_gain']) < 1e-12
    assert bloom['threshold'] == 0.0 and bloom['integrated_scatter_gain'] > 0
    assert result['sky_light']['capture_origin'] == [0.0, 0.0, -0.0]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--world', type=Path, required=True)
    parser.add_argument('--camera', type=Path, required=True)
    parser.add_argument('--engine', type=Path, required=True)
    parser.add_argument('--project-settings', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--scene', type=Path)
    parser.add_argument('--self-test', action='store_true')
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    world = json.loads(args.world.read_text(encoding='utf-8-sig'))
    camera = json.loads(args.camera.read_text(encoding='utf-8-sig'))
    eye = (args.engine / 'Source/Runtime/Renderer/Private/PostProcess/PostProcessEyeAdaptation.cpp').read_text(encoding='utf-8-sig')
    view = (args.engine / 'Source/Runtime/Engine/Private/SceneView.cpp').read_text(encoding='utf-8-sig')
    project = args.project_settings.read_text(encoding='utf-8-sig')
    header = (args.engine / 'Source/Runtime/Engine/Classes/Engine/Scene.h').read_text(encoding='utf-8-sig')
    if 'AutoExposureBiasCurve = nullptr;' not in header or 'AutoExposureMeterMask = nullptr;' not in header:
        raise ValueError('Source omitted reference defaults require re-extraction')
    scene = (args.engine / 'Source/Runtime/Engine/Private/Scene.cpp').read_text(encoding='utf-8-sig')
    bloom = (args.engine / 'Source/Runtime/Renderer/Private/PostProcess/PostProcessBloomSetup.cpp').read_text(encoding='utf-8-sig')
    console = (args.engine / 'Source/Runtime/Core/Private/HAL/ConsoleManager.cpp').read_text(encoding='utf-8-sig')
    if 'BloomMethod = BM_SOG;' not in scene or cvar(console, 'r.BloomQuality') != 5 or 'const float TintScale = (1.0f / BloomQualityCountMax) * BloomIntensity;' not in bloom:
        raise ValueError('Source bloom defaults or normalization require mapping')
    if args.self_test:
        self_test(world, camera, eye, view, project)
    text = json.dumps(compile_lighting(world, camera, eye, view, project), indent=2) + '\n'
    if args.check:
        if args.output.read_text(encoding='utf-8') != text:
            raise ValueError('Installed lighting data differs from source')
    else:
        args.output.write_text(text, encoding='utf-8')
    if args.scene:
        original = args.scene.read_text(encoding='utf-8')
        updated = re.sub(r',\s*camera_(?:exposure_ev100|contrast):\s*(?:Some\([^)]*\)|None)', '', original)
        if args.check:
            if updated != original:
                raise ValueError('Retired fixed exposure fields remain in the scene')
        else:
            args.scene.write_text(updated, encoding='utf-8')
    print(json.dumps({'success': True, 'self_test': args.self_test, 'check': args.check}), flush=True)


if __name__ == '__main__':
    main()
