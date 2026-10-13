import argparse
import hashlib
import json
import re
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'animgraph_authoring'))
from ue_animation_asset_resolver import object_property
from ue_exported_text import parse_struct
from import_playground_lighting import cvar


def array_constant(text, name, shape):
    clean = re.sub(r'//[^\n]*', '', text)
    match = re.search(r'\b' + re.escape(name) + r'\s*=\s*\{([^}]+)\}', clean)
    if not match:
        raise ValueError('Source color constant is missing: ' + name)
    values = [float(value) for value in re.findall(r'[-+]?\d*\.?\d+(?:[eE][-+]?\d+)?', match[1])]
    return np.array(values).reshape(shape)


def load_source(world, engine):
    volume = next(obj for obj in world['asset_objects'] if obj['class_path'] == '/Script/Engine.PostProcessVolume')
    settings = parse_struct(object_property(volume, 'Settings'))
    if any(value == 'True' for name, value in settings.items() if name.startswith('bOverride_') and name != 'bOverride_MotionBlurAmount'):
        raise ValueError('Additional source post processing requires its active override closure')
    paths = ['Shaders/Private/ACES/ACESCommon.ush', 'Shaders/Private/TonemapCommon.ush',
             'Shaders/Private/PostProcessCombineLUTs.usf', 'Source/Runtime/Engine/Private/Scene.cpp',
             'Source/Runtime/Core/Public/ColorManagement/ColorManagementDefines.h']
    texts = [(engine / name).read_text(encoding='utf-8-sig') for name in paths]
    if 'TonemappingMethod = ETonemappingMethod::Filmic;' not in texts[3] or 'DEFAULT_CHROMATIC_ADAPTATION_METHOD = EChromaticAdaptationMethod::Bradford' not in texts[4]:
        raise ValueError('Source display method or adaptation requires mapping')
    for name, neutral in [('ColorSaturation', 1), ('ColorContrast', 1), ('ColorGamma', 1), ('ColorGain', 1), ('ColorOffset', 0)]:
        if any(float(value) != neutral for value in parse_struct(settings[name]).values()):
            raise ValueError('Source color grading requires mapping: ' + name)
    def scalar(name):
        if name in settings:
            return float(settings[name])
        match = re.search(r'\b' + re.escape(name) + r'\s*=\s*(-?[\d.]+)f?\s*;', texts[3])
        if not match:
            raise ValueError('Source omitted scalar default is missing: ' + name)
        return float(match[1])
    if scalar('WhiteTemp') != 6500 or scalar('WhiteTint') != 0:
        raise ValueError('Source white balance requires mapping')
    matrices = {name: array_constant(texts[0], name, (3, 3)) for name in
                ['sRGB_2_XYZ_MAT', 'XYZ_2_sRGB_MAT', 'XYZ_2_AP0_MAT', 'AP0_2_XYZ_MAT',
                 'XYZ_2_AP1_MAT', 'AP1_2_XYZ_MAT', 'AP0_2_AP1_MAT', 'D65_2_D60_CAT', 'D60_2_D65_CAT']}
    matrices['luma'] = array_constant(texts[0], 'AP1_RGB2Y', (3,))
    for name in ['BlueCorrect', 'BlueCorrectInv', 'Wide_2_XYZ_MAT']:
        matrices[name] = array_constant(texts[2], name, (3, 3))
    parameters = {name: scalar(name) for name in ['FilmSlope', 'FilmToe', 'FilmShoulder',
                  'FilmBlackClip', 'FilmWhiteClip', 'BlueCorrection', 'ExpandGamut', 'ToneCurveAmount']}
    return matrices, parameters, {name: hashlib.sha256(text.encode()).hexdigest() for name, text in zip(paths, texts)}


def film_curve(color, parameters):
    slope, toe, shoulder = [parameters[name] for name in ['FilmSlope', 'FilmToe', 'FilmShoulder']]
    black, white = parameters['FilmBlackClip'], parameters['FilmWhiteClip']
    toe_scale, shoulder_scale = 1 + black - toe, 1 + white - shoulder
    if toe > .8:
        toe_match = (1 - toe - .18) / slope + np.log10(.18)
    else:
        bt = (.18 + black) / toe_scale - 1
        toe_match = np.log10(.18) - .5 * np.log((1 + bt) / (1 - bt)) * toe_scale / slope
    straight_match = (1 - toe) / slope - toe_match
    shoulder_match = shoulder / slope - straight_match
    with np.errstate(divide='ignore', over='ignore'):
        log_color = np.log10(color)
        straight = slope * (log_color + straight_match)
        low = -black + 2 * toe_scale / (1 + np.exp((-2 * slope / toe_scale) * (log_color - toe_match)))
        high = 1 + white - 2 * shoulder_scale / (1 + np.exp((2 * slope / shoulder_scale) * (log_color - shoulder_match)))
    low = np.where(log_color < toe_match, low, straight)
    high = np.where(log_color > shoulder_match, high, straight)
    t = np.clip((log_color - toe_match) / (shoulder_match - toe_match), 0, 1)
    if shoulder_match < toe_match:
        t = 1 - t
    t = (3 - 2 * t) * t * t
    with np.errstate(invalid='ignore'):
        return np.where(t == 0, low, np.where(t == 1, high, low * (1 - t) + high * t))


def source_display(rgb, matrices, parameters):
    m = matrices
    to_ap1 = m['XYZ_2_AP1_MAT'] @ m['D65_2_D60_CAT'] @ m['sRGB_2_XYZ_MAT']
    from_ap1 = np.linalg.inv(to_ap1)
    ap1_to_ap0 = m['XYZ_2_AP0_MAT'] @ m['AP1_2_XYZ_MAT']
    ap0_to_ap1 = m['XYZ_2_AP1_MAT'] @ m['AP0_2_XYZ_MAT']
    color = np.asarray(rgb) @ to_ap1.T
    luma = color @ m['luma']
    chroma = np.divide(color, luma[..., None], out=np.ones_like(color), where=luma[..., None] != 0)
    distance = ((chroma - 1) ** 2).sum(axis=-1)
    amount = (1 - 2 ** (-4 * distance)) * (1 - 2 ** (-4 * parameters['ExpandGamut'] * luma * luma))
    legacy_from_ap1 = m['XYZ_2_sRGB_MAT'] @ m['D60_2_D65_CAT'] @ m['AP1_2_XYZ_MAT']
    expand = m['XYZ_2_AP1_MAT'] @ m['Wide_2_XYZ_MAT'] @ legacy_from_ap1
    color += (color @ expand.T - color) * amount[..., None]
    correction = ap0_to_ap1 @ m['BlueCorrect'] @ ap1_to_ap0
    inverse_correction = ap0_to_ap1 @ m['BlueCorrectInv'] @ ap1_to_ap0
    color += (color @ correction.T - color) * parameters['BlueCorrection']
    before_curve = color.copy()
    ap0 = color @ ap1_to_ap0.T
    minimum, maximum = ap0.min(axis=-1), ap0.max(axis=-1)
    saturation = (np.maximum(maximum, 1e-10) - np.maximum(minimum, 1e-10)) / np.maximum(maximum, 1e-2)
    r, g, b = np.moveaxis(ap0, -1, 0)
    yc = (r + g + b + 1.75 * np.sqrt(np.maximum(0, b * (b - g) + g * (g - r) + r * (r - b)))) / 3
    x = (saturation - .4) / .2
    shape = .5 * (1 + np.sign(x) * (1 - np.maximum(1 - np.abs(.5 * x), 0) ** 2))
    gain = .05 * shape
    with np.errstate(divide='ignore', invalid='ignore'):
        glow = np.where(yc <= 2 / 3 * .08, gain, np.where(yc >= .16, 0, gain * (.08 / yc - .5)))
    ap0 *= (1 + glow)[..., None]
    r, g, b = np.moveaxis(ap0, -1, 0)
    hue = np.degrees(np.arctan2(np.sqrt(3) * (g - b), 2 * r - g - b))
    hue = np.where((r == g) & (g == b), 0, hue)
    hue_weight = np.clip(1 - np.abs(2 * hue / 135), 0, 1)
    hue_weight = ((3 - 2 * hue_weight) * hue_weight * hue_weight) ** 2
    ap0[..., 0] += hue_weight * saturation * (.03 - ap0[..., 0]) * (1 - .82)
    working = np.maximum(0, ap0 @ m['AP0_2_AP1_MAT'].T)
    working = (working @ m['luma'])[..., None] * .04 + working * .96
    tone = film_curve(working, parameters)
    tone = np.maximum(0, (tone @ m['luma'])[..., None] * .07 + tone * .93)
    color = before_curve + (tone - before_curve) * parameters['ToneCurveAmount']
    color += (color @ inverse_correction.T - color) * parameters['BlueCorrection']
    return np.maximum(0, color @ from_ap1.T)


def sample_lut(lut, rgb):
    coordinate = np.asarray(rgb) / (np.asarray(rgb) + 1) * (lut.shape[0] - 1)
    lower = np.floor(coordinate).astype(int)
    upper = np.minimum(lower + 1, lut.shape[0] - 1)
    fraction = coordinate - lower
    result = np.zeros_like(coordinate)
    for z in range(2):
        for y in range(2):
            for x in range(2):
                index = np.where(np.array([x, y, z]), upper, lower)
                weight = np.where(np.array([x, y, z]), fraction, 1 - fraction).prod(axis=-1)
                result += lut[index[..., 2], index[..., 1], index[..., 0], :3] * weight[..., None]
    return result


def source_bloom_kernel(width, stage, bloom):
    level_width = width
    for _ in range(stage + 1):
        level_width = (level_width + 1) // 2
    radius = level_width * bloom['sizes'][stage] * bloom['size_scale'] * .005
    sigma = np.clip(radius, .00001, bloom['sample_count_max'] - 1)
    integer = min(int(np.ceil(np.clip(radius * bloom['filter_size_scale'], .00001, bloom['sample_count_max'] - 1))), bloom['sample_count_max'] - 1)
    taps = []
    for index in range(-integer, integer + 1, 2):
        first = np.exp(bloom['kernel_exponent'] * (index / sigma) ** 2)
        second = np.exp(bloom['kernel_exponent'] * ((index + 1) / sigma) ** 2) if index != integer else 0
        total = first + second
        if total > 0:
            taps.append([index + second / total, total])
    total = sum(tap[1] for tap in taps)
    return dict(width=width, stage=stage, level_width=level_width, radius=radius,
                horizontal_width=(level_width + 1) // 2 if radius >= bloom['fast_blur_threshold'] else level_width,
                taps=[[offset, weight / total] for offset, weight in taps],
                tint=[channel * bloom['intensity'] / 6 for channel in bloom['tints'][stage]])


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--world', type=Path, required=True)
    parser.add_argument('--engine', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--fixtures', type=Path)
    parser.add_argument('--self-test', action='store_true')
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    world = json.loads(args.world.read_text(encoding='utf-8-sig'))
    matrices, parameters, contract = load_source(world, args.engine)
    axis = np.arange(48) / 47
    axis = np.divide(axis, 1 - axis, out=np.full_like(axis, 65504), where=axis < 1)
    blue, green, red = np.meshgrid(axis, axis, axis, indexing='ij')
    colors = source_display(np.stack((red, green, blue), axis=-1), matrices, parameters)
    if not np.isfinite(colors).all():
        raise ValueError('Source display data must be finite at every native LUT coordinate')
    lut = np.concatenate((colors, np.ones(colors.shape[:-1] + (1,))), axis=-1).astype('<f2')
    volume = next(obj for obj in world['asset_objects'] if obj['class_path'] == '/Script/Engine.PostProcessVolume')
    settings = parse_struct(object_property(volume, 'Settings'))
    bloom = dict(threshold=float(settings['BloomThreshold']), size_scale=float(settings['BloomSizeScale']),
                 intensity=float(settings['BloomIntensity']) * float(settings['BloomGaussianIntensity']),
                 sizes=[float(settings[f'Bloom{index}Size']) for index in range(1, 7)],
                 tints=[[float(parse_struct(settings[f'Bloom{index}Tint'])[channel]) for channel in 'RGB'] for index in range(1, 7)])
    blur_source = (args.engine / 'Source/Runtime/Renderer/Private/PostProcess/PostProcessWeightedSampleSum.cpp').read_text(encoding='utf-8-sig')
    post_source = (args.engine / 'Source/Runtime/Renderer/Private/PostProcess/PostProcessing.cpp').read_text(encoding='utf-8-sig')
    if cvar(post_source, 'r.PostProcessing.DownsampleChainQuality') != 1 or 'const float LegacyCompatibilityConstant = -16.7f;' not in blur_source:
        raise ValueError('Source Gaussian downsample or kernel requires mapping')
    bloom.update(filter_size_scale=cvar(blur_source, 'r.Filter.SizeScale'),
                 fast_blur_threshold=cvar(blur_source, 'r.FastBlurThreshold'),
                 kernel_exponent=-16.7, sample_count_max=32, downsample_quality='high')
    data = lut.tobytes()
    metadata = dict(format='playground_display_v1', lut_dimension=48, lut_domain='linear_over_linear_plus_one',
                    lut_sha256=hashlib.sha256(data).hexdigest(), filmic=parameters, bloom=bloom, source_contract=contract)
    probes = np.array([[0, 0, 0], [.18, .18, .18], [.36, .36, .36], [.0342, .073491, .18], [.2, .2, 1], [110, 110, 110]])
    exact = source_display(probes, matrices, parameters)
    sampled = sample_lut(lut.astype(float), probes)
    if args.self_test:
        assert np.allclose(exact[0], 0, atol=1e-12)
        assert np.allclose(exact[1], .18, atol=2e-5)
        assert np.max(np.abs(sampled - exact)) < 1 / 255
        assert np.all(np.diff(film_curve(np.geomspace(.001, 100, 100), parameters)) >= 0)
        changed = dict(parameters, FilmSlope=.5)
        assert not np.allclose(source_display(probes[2:5], matrices, changed), exact[2:5])
    files = {'playground_filmic.rgba16f': data,
             'playground_display.runtime.json': (json.dumps(metadata, indent=2) + '\n').encode()}
    for name, payload in files.items():
        path = args.output / name
        if args.check:
            if path.read_bytes() != payload:
                raise ValueError('Installed source display differs: ' + name)
        else:
            path.write_bytes(payload)
    if args.fixtures:
        fixtures = dict(kernels=[source_bloom_kernel(width, stage, bloom) for width in [641, 1600] for stage in range(6)],
                        uniform_inputs=[1, 110], uniform_output_gain=sum(tint[0] for tint in bloom['tints']) * bloom['intensity'] / 6)
        fixture_text = json.dumps(fixtures, indent=2) + '\n'
        if args.check:
            if args.fixtures.read_text(encoding='utf-8') != fixture_text:
                raise ValueError('Installed Gaussian fixtures differ from source')
        else:
            args.fixtures.write_text(fixture_text, encoding='utf-8')
    report = dict(success=True, self_test=args.self_test, check=args.check, probes=probes.tolist(),
                  source=exact.tolist(), native_lut=sampled.tolist(), maximum_probe_error=float(np.max(np.abs(sampled - exact))))
    args.report.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(report), flush=True)


if __name__ == '__main__':
    main()
