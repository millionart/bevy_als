import argparse
import hashlib
import json
from pathlib import Path

import numpy as np

from capture_playground_sky import cube_directions


def source_directions(runtime):
    return runtime[..., [0, 2, 1]]


def sh_basis(direction):
    x, y, z = np.moveaxis(direction, -1, 0)
    return np.stack((np.full_like(x, .282095), -.488603 * y, .488603 * z, -.488603 * x,
                     1.092548 * x * y, -1.092548 * y * z, .315392 * (3 * z * z - 1),
                     -1.092548 * x * z, .546274 * (x * x - y * y)), axis=-1)


def box_mips(cube):
    result = [cube.astype(np.float64)]
    while result[-1].shape[1] > 1:
        image = result[-1]
        size = image.shape[1]
        result.append(image.reshape(6, size // 2, 2, size // 2, 2, 4)
                      .mean(axis=(2, 4)).astype('<f2').astype(np.float64))
    return result


def project_source_sh(mips):
    image = mips[max(0, len(mips) - 6)]
    size = image.shape[1]
    directions = source_directions(np.stack([cube_directions(face, size).reshape(size, size, 3) for face in range(6)]))
    basis = sh_basis(directions)
    axis = (np.arange(size) + .5) * 2 / size - 1
    u, v = np.meshgrid(axis, axis)
    weights = np.broadcast_to(4 / (1 + u * u + v * v) ** 1.5, (6, size, size))
    coefficients = []
    for index in range(9):
        values = np.concatenate((image[..., :3] * (basis[..., index] * weights)[..., None], weights[..., None]), axis=-1)
        values = box_mips(values.astype('<f2'))[-1][:, 0, 0].sum(axis=0)
        coefficients.append((4 * np.pi * values[:3] / max(values[3], .00001)).astype('<f2'))
    return np.asarray(coefficients, dtype=np.float64)


def evaluate_source_sh(coefficients, runtime_directions):
    x, y, z = np.moveaxis(source_directions(runtime_directions), -1, 0)
    c0 = 1 / (2 * np.sqrt(np.pi))
    c1 = np.sqrt(3) / (3 * np.sqrt(np.pi))
    c2 = np.sqrt(15) / (8 * np.sqrt(np.pi))
    c3 = np.sqrt(5) / (16 * np.sqrt(np.pi))
    linear = (c0 * coefficients[0] - c3 * coefficients[6]
              + c1 * (-x[..., None] * coefficients[3] - y[..., None] * coefficients[1] + z[..., None] * coefficients[2]))
    quadratic = c2 * ((x * y)[..., None] * coefficients[4] - (y * z)[..., None] * coefficients[5]
                      - (x * z)[..., None] * coefficients[7]) + 3 * c3 * (z * z)[..., None] * coefficients[6]
    return np.maximum(0, linear + quadratic + .5 * c2 * (x * x - y * y)[..., None] * coefficients[8])


def sample_cube(cube, directions):
    absolute = np.abs(directions)
    axis = np.argmax(absolute, axis=-1)
    dominant = np.take_along_axis(directions, axis[..., None], axis=-1)[..., 0]
    positive = dominant >= 0
    divisor = np.abs(dominant)
    face = axis * 2 + (~positive).astype(int)
    u = np.where(axis == 0, np.where(positive, -directions[..., 2], directions[..., 2]),
                 np.where(axis == 1, directions[..., 0], np.where(positive, directions[..., 0], -directions[..., 0]))) / divisor
    v = np.where(axis == 1, np.where(positive, directions[..., 2], -directions[..., 2]), -directions[..., 1]) / divisor
    size = cube.shape[1]
    pixels = (np.stack((u, v), axis=-1) + 1) * (.5 * size) - .5
    integer = np.floor(pixels).astype(int)
    fraction = pixels - integer
    x, y = np.moveaxis(np.clip(integer, 0, size - 1), -1, 0)
    xx, yy = np.moveaxis(np.clip(integer + 1, 0, size - 1), -1, 0)
    a, b = fraction[..., 0, None], fraction[..., 1, None]
    return ((cube[face, y, x, :3] * (1 - a) + cube[face, y, xx, :3] * a) * (1 - b)
            + (cube[face, yy, x, :3] * (1 - a) + cube[face, yy, xx, :3] * a) * b)


def sample_mips(mips, directions, levels):
    levels = np.broadcast_to(np.clip(levels, 0, len(mips) - 1), directions.shape[:-1])
    lower = np.floor(levels).astype(int)
    upper = np.minimum(lower + 1, len(mips) - 1)
    result = np.zeros(directions.shape, dtype=np.float64)
    for index, image in enumerate(mips):
        for selected, weight in [(lower == index, 1 - (levels - lower)), (upper == index, levels - lower)]:
            if selected.any():
                result[selected] += sample_cube(image, directions[selected]) * weight[selected, None]
    return result


def tangent_frame(normal):
    x, y, z = np.moveaxis(normal, -1, 0)
    sign = np.where(z >= 0, 1, -1)
    a = -1 / (sign + z)
    b = x * y * a
    tangent = np.stack((1 + sign * a * x * x, sign * b, -sign * x), axis=-1)
    bitangent = np.stack((b, sign + a * y * y, -y), axis=-1)
    return np.stack((tangent, bitangent, normal), axis=-2)


def hammersley(count):
    indices = np.arange(count, dtype=np.uint32)
    reverse = np.zeros_like(indices)
    for bit in range(32):
        reverse |= ((indices >> bit) & 1) << (31 - bit)
    return np.stack((indices / count, reverse.astype(float) / 4294967296), axis=-1)


def filter_source_specular(mips, normal, roughness):
    if roughness < .01:
        return sample_cube(mips[0], normal)
    count = 32 if roughness < .1 else 64
    points = hammersley(count)
    phi = 2 * np.pi * points[:, 0]
    if roughness > .99:
        cosine = np.sqrt(points[:, 1])
        sine = np.sqrt(1 - points[:, 1])
        local = np.stack((sine * np.cos(phi), sine * np.sin(phi), cosine), axis=-1)
        pdf = cosine / np.pi
        weight = np.ones(count)
    else:
        y = points[:, 1] * .995
        a2 = roughness ** 4
        cosine = np.sqrt((1 - y) / (1 + (a2 - 1) * y))
        sine = np.sqrt(np.maximum(0, 1 - cosine * cosine))
        half = np.stack((sine * np.cos(phi), sine * np.sin(phi), cosine), axis=-1)
        local = 2 * cosine[:, None] * half - np.array([0, 0, 1])
        weight = np.maximum(0, local[:, 2])
        pdf = a2 / (np.pi * ((cosine * a2 - cosine) * cosine + 1) ** 2) * .25
    selected = weight > 0
    solid_angle_texel = 4 * np.pi / (6 * mips[0].shape[1] ** 2) * 2
    with np.errstate(divide='ignore'):
        level = .5 * np.log2(1 / (count * pdf[selected] * solid_angle_texel))
    frame = tangent_frame(source_directions(normal))
    direction = np.einsum('sj,njk->nsk', local[selected], frame)
    direction = source_directions(direction)
    return (sample_mips(mips, direction, level) * weight[selected, None]).sum(axis=1) / weight.sum()


def rgba_half(colors):
    return np.concatenate((colors, np.ones(colors.shape[:-1] + (1,))), axis=-1).astype('<f2')


def build_environment(cube, progress=True):
    mips = box_mips(cube)
    coefficients = project_source_sh(mips)
    normals = np.stack([cube_directions(face, 32).reshape(32, 32, 3) for face in range(6)])
    diffuse = rgba_half(evaluate_source_sh(coefficients, normals))
    source_specular = []
    maximum_mip = len(mips) - 1
    for mip, image in enumerate(mips[:-2]):
        size = image.shape[1]
        roughness = 2 ** ((mip - maximum_mip + 2) / 1.2)
        faces = []
        for face in range(6):
            normals = cube_directions(face, size)
            values = [filter_source_specular(mips, normals[start:start + 512], roughness)
                      for start in range(0, len(normals), 512)]
            faces.append(rgba_half(np.concatenate(values)).reshape(size, size, 4))
            if progress:
                print(json.dumps({'phase': 'source_specular', 'mip': mip, 'face': face}), flush=True)
        source_specular.append(np.array(faces).astype(np.float64))
    radiance = []
    for mip, image in enumerate(mips[:-2]):
        size = image.shape[1]
        roughness = mip / (maximum_mip - 2)
        source_level = maximum_mip - 2 + 1.2 * np.log2(max(roughness, .001))
        normals = np.stack([cube_directions(face, size).reshape(size, size, 3) for face in range(6)])
        radiance.append(rgba_half(sample_mips(source_specular, normals, source_level)))
    return coefficients, diffuse, radiance


def source_contract(engine):
    files = {
        'diffuse': 'Source/Runtime/Renderer/Private/ReflectionEnvironmentDiffuseIrradiance.cpp',
        'capture': 'Shaders/Private/ReflectionEnvironmentShaders.usf',
        'sh': 'Shaders/Private/SHCommon.ush',
        'reflection': 'Shaders/Private/ReflectionEnvironmentShared.ush',
        'sampling': 'Shaders/Private/MonteCarlo.ush',
    }
    text = {name: (engine / path).read_text(encoding='utf-8-sig') for name, path in files.items()}
    required = {
        'diffuse': ['GDiffuseIrradianceCubemapSize = 32', 'FSHVector3::MaxSHBasis'],
        'capture': ['Roughness < 0.1 ? 32 : 64', 'E.y *= 0.995', '4 * PI * AccumulatedValue.rgb'],
        'sh': ['0.282095f', '0.488603f', '1.092548f', '0.315392f', '0.546274f'],
        'reflection': ['REFLECTION_CAPTURE_ROUGHEST_MIP 1', 'REFLECTION_CAPTURE_ROUGHNESS_MIP_SCALE 1.2'],
        'sampling': ['float CosTheta = sqrt(E.y)', 'float2 Hammersley('],
    }
    for name, expected in required.items():
        if any(value not in text[name] for value in expected):
            raise ValueError('Source environment algorithm requires re-extraction: ' + name)
    return {name: hashlib.sha256(text[name].encode()).hexdigest() for name in files}


def self_test():
    color = np.array([.2, .4, .8, 1.0])
    cube = np.broadcast_to(color, (6, 32, 32, 4)).copy()
    mips = box_mips(cube)
    coefficients = project_source_sh(mips)
    normals = np.concatenate([cube_directions(face, 8) for face in range(6)])
    assert np.allclose(evaluate_source_sh(coefficients, normals), color[:3], atol=np.finfo(np.float16).eps)
    for roughness in [0, .05, .3, 1]:
        assert np.allclose(filter_source_specular(mips, normals[:8], roughness), color[:3], atol=np.finfo(np.float16).eps)
    frame = tangent_frame(source_directions(normals))
    assert np.allclose(frame @ np.swapaxes(frame, -1, -2), np.eye(3), atol=1e-12)
    assert np.allclose(hammersley(4), [[0, 0], [.25, .5], [.5, .25], [.75, .75]])
    lobe = np.zeros((9, 3))
    lobe[0] = 1
    lobe[2] = .5
    values = evaluate_source_sh(lobe, np.array([[0, 1, 0], [0, -1, 0]]))
    expected = np.array([1 / (2 * np.sqrt(np.pi)) + .5 * np.sqrt(3) / (3 * np.sqrt(np.pi)),
                         1 / (2 * np.sqrt(np.pi)) - .5 * np.sqrt(3) / (3 * np.sqrt(np.pi))])
    assert np.allclose(values, expected[:, None], rtol=0, atol=1e-12)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--capture', type=Path, required=True)
    parser.add_argument('--lighting', type=Path, required=True)
    parser.add_argument('--engine', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--self-test', action='store_true')
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    contract = source_contract(args.engine)
    if args.self_test:
        self_test()
    settings = json.loads(args.lighting.read_text(encoding='utf-8'))['sky_light']
    size = settings['capture_resolution']
    raw = args.capture.read_bytes()
    cube = np.frombuffer(raw, dtype='<f2').reshape(6, size, size, 4).astype(float)
    if not np.isfinite(cube).all() or not np.all(cube[..., 3] == 1):
        raise ValueError('Captured source sky must contain finite opaque HDR radiance')
    coefficients, diffuse, radiance = build_environment(cube)
    files = {'playground_sky_irradiance.rgba16f': diffuse.tobytes(),
             'playground_sky_radiance.rgba16f': b''.join(image.tobytes() for image in radiance)}
    metadata = dict(format='playground_environment_v1', capture_sha256=hashlib.sha256(raw).hexdigest(),
                    diffuse_resolution=32, radiance_resolution=size, radiance_mip_count=len(radiance),
                    data_order='mip_major', source_sh_coefficients=coefficients.tolist(),
                    source_contract=contract,
                    payload_sha256={name: hashlib.sha256(data).hexdigest() for name, data in files.items()})
    files['playground_environment.runtime.json'] = (json.dumps(metadata, indent=2) + '\n').encode()
    for name, data in files.items():
        target = args.output / name
        if args.check:
            if target.read_bytes() != data:
                raise ValueError('Installed source environment differs: ' + name)
        else:
            target.write_bytes(data)
    directions = np.concatenate([cube_directions(face, 32) for face in range(6)])
    expected = evaluate_source_sh(coefficients, directions)
    actual = diffuse[..., :3].astype(float).reshape(-1, 3)
    error = float(np.abs(actual - expected).max())
    tolerance = float(np.spacing(np.float16(actual.max())))
    if error > tolerance:
        raise ValueError('Diffuse output exceeds source half-float error')
    report = dict(success=True, self_test=args.self_test, check=args.check, diffuse_source_max_error=error,
                  half_float_tolerance=tolerance, diffuse_maximum=float(actual.max()),
                  specular_maxima=[float(image[..., :3].max()) for image in radiance])
    args.report.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(report), flush=True)


if __name__ == '__main__':
    main()
