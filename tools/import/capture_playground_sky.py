import argparse
import hashlib
import json
import re
from pathlib import Path

import numpy as np

from audit_playground_sky import SourceMaterial, refresh
from coordinate_basis import runtime_point_to_source, runtime_direction_to_source
from import_moving_actors import point


def cube_directions(face, resolution):
    axis = (np.arange(resolution) + 0.5) * 2 / resolution - 1
    u, v = np.meshgrid(axis, axis)
    one = np.ones_like(u)
    faces = [(one, -v, -u), (-one, -v, u), (u, one, v),
             (u, -one, -v), (u, -v, one), (-u, -v, -one)]
    directions = np.stack(faces[face], axis=-1).reshape(-1, 3)
    return directions / np.linalg.norm(directions, axis=-1, keepdims=True)


def mesh_hits(origin, directions, positions, indices, near_plane):
    triangles = positions[indices]
    first = triangles[:, 1] - triangles[:, 0]
    second = triangles[:, 2] - triangles[:, 0]
    offset = origin - triangles[:, 0]
    cross = np.cross(directions[:, None], second[None])
    determinant = np.einsum('tj,ntj->nt', first, cross)
    inverse = np.divide(1.0, determinant, out=np.zeros_like(determinant, dtype=float), where=np.abs(determinant) > 1e-10)
    u = np.einsum('tj,ntj->nt', offset, cross) * inverse
    q = np.cross(offset, first)
    v = np.einsum('nj,tj->nt', directions, q) * inverse
    distance = np.einsum('tj,tj->t', second, q)[None] * inverse
    valid = (np.abs(determinant) > 1e-10) & (u >= -1e-7) & (v >= -1e-7) & (u + v <= 1 + 1e-7)
    valid &= distance * np.abs(directions).max(axis=-1, keepdims=True) >= near_plane
    distance = np.where(valid, distance, np.inf)
    selected = np.argmin(distance, axis=-1)
    rows = np.arange(len(directions))
    if not np.isfinite(distance[rows, selected]).all():
        raise ValueError('Source sky geometry does not cover every capture ray beyond the near plane')
    weights = np.stack((1 - u[rows, selected] - v[rows, selected], u[rows, selected], v[rows, selected]), axis=-1)
    return selected, weights


def capture(material, definition, geometry, sky, resolution, capture_time, progress=True):
    frame = definition['transform']
    positions = np.array([point(frame, value) for value in geometry['positions']])
    indices = np.asarray(geometry['indices']).reshape(-1, 3)
    uv = [np.asarray(geometry[name])[indices] for name in ['uv0', 'uv1']]
    origin = np.asarray(sky['capture_origin'])
    object_origin = np.asarray(runtime_point_to_source(point(frame, geometry['bounds_origin'])))
    radius = geometry['bounds_radius'] * max(abs(value) for value in frame['scale']) * 100
    parameters = refresh(definition)
    faces = []
    for face in range(6):
        directions = cube_directions(face, resolution)
        rgba = np.ones((len(directions), 4))
        for start in range(0, len(directions), 256):
            rays = directions[start:start + 256]
            triangle, weights = mesh_hits(origin, rays, positions, indices, sky['sky_distance_threshold'])
            hit = np.einsum('nj,njk->nk', weights, positions[indices[triangle]])
            context = dict(camera_vector=np.array([runtime_direction_to_source(-ray) for ray in rays]),
                           origin=object_origin, position=np.array([runtime_point_to_source(value) for value in hit]),
                           radius=radius, time=capture_time,
                           uv0=np.einsum('nj,njk->nk', weights, uv[0][triangle]),
                           uv1=np.einsum('nj,njk->nk', weights, uv[1][triangle]))
            colors = material.evaluate(context, parameters)
            if sky['lower_hemisphere_black']:
                colors[rays[:, 1] < 0] = 0
            rgba[start:start + len(rays), :3] = colors
            if progress and start % 4096 == 0:
                print(json.dumps({'face': face, 'pixels': start + len(rays)}), flush=True)
        faces.append(rgba.reshape(resolution, resolution, 4))
    result = np.asarray(faces)
    if not np.isfinite(result).all() or np.abs(result).max() > 65504:
        raise ValueError('Source radiance does not fit the source half-float capture format')
    return result.astype('<f2')


def check_scene_bounds(scene, sky):
    maximum = 0.0
    for line in scene.splitlines():
        if 'mesh:' not in line and 'class: "Landscape"' not in line and 'class: "TextRenderActor"' not in line:
            continue
        match = re.search(r'bounds_origin: \(([^)]+)\), bounds_extent: \(([^)]+)\)', line)
        if not match:
            raise ValueError('Renderable scene owner has no imported source bounds')
        center, extent = [np.fromstring(value, sep=',') for value in match.groups()]
        origin = np.asarray(runtime_point_to_source(sky['capture_origin']))
        maximum = max(maximum, np.linalg.norm(np.abs(center - origin) + extent) * 0.01)
    if maximum >= sky['sky_distance_threshold']:
        raise ValueError('Source capture requires geometry beyond the sky sphere')
    return maximum


def self_test(material, definition, geometry, sky):
    expected = np.array([[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]])
    assert np.array_equal(np.array([cube_directions(face, 1)[0] for face in range(6)]), expected)
    corners = np.array([[1, .5, .5], [-1, .5, -.5], [-.5, 1, -.5],
                        [-.5, -1, .5], [-.5, .5, 1], [.5, .5, -1]])
    assert np.allclose([cube_directions(face, 2)[0] for face in range(6)], corners / np.sqrt(1.5))
    assert runtime_direction_to_source([0, 1, 0]) == [0, 0, 1]
    toy_positions = np.array([[-1, -1, 2], [1, -1, 2], [0, 1, 2]])
    triangle, weights = mesh_hits(np.zeros(3), np.array([[0, 0, 1]]), toy_positions, np.array([[0, 1, 2]]), 1)
    assert triangle[0] == 0 and np.allclose(weights[0], [.25, .25, .5])
    try:
        mesh_hits(np.zeros(3), np.array([[0, 0, 1]]), toy_positions, np.array([[0, 1, 2]]), 3)
    except ValueError:
        pass
    else:
        raise AssertionError('Source capture near plane was ignored')
    rays = cube_directions(2, 2)
    radius = geometry['bounds_radius'] * max(definition['transform']['scale']) * 100
    context = dict(camera_vector=np.array([runtime_direction_to_source(-ray) for ray in rays]),
                   origin=np.zeros(3), position=np.array([runtime_direction_to_source(ray) for ray in rays]) * radius,
                   radius=radius, time=0.0, uv0=np.array([[.31, .47]] * 4), uv1=np.array([[.67, .29]] * 4))
    parameters = refresh(definition)
    batch = material.evaluate(context, parameters)
    scalar = [material.evaluate({name: value[index] if name in ['camera_vector', 'position', 'uv0', 'uv1'] else value
                                for name, value in context.items()}, parameters) for index in range(4)]
    assert np.allclose(batch, scalar, rtol=0, atol=1e-12)
    small = capture(material, definition, geometry, sky, 4, 0, False)
    directions = np.array([cube_directions(face, 4).reshape(4, 4, 3) for face in range(6)])
    assert np.all(small[..., :3][directions[..., 1] < 0] == 0)
    assert small[..., :3][directions[..., 1] > 0].max() > 0
    clear = capture(material, definition, geometry, dict(sky, lower_hemisphere_black=False), 4, 0, False)
    assert clear[..., :3][directions[..., 1] < 0].max() > 0
    assert np.all(small[..., 3] == 1)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--definition', type=Path, required=True)
    parser.add_argument('--geometry', type=Path, required=True)
    parser.add_argument('--lighting', type=Path, required=True)
    parser.add_argument('--scene', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--capture-time', type=float, default=0.0)
    parser.add_argument('--node', default='node')
    parser.add_argument('--pngjs-module', default='pngjs')
    parser.add_argument('--self-test', action='store_true')
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    source = json.loads((args.source / 'M_Sky_Panning_Clouds2.ue.json').read_text(encoding='utf-8-sig'))
    definition = json.loads(args.definition.read_text(encoding='utf-8'))
    geometry = json.loads(args.geometry.read_text(encoding='utf-8'))
    sky = json.loads(args.lighting.read_text(encoding='utf-8'))['sky_light']
    if sky['real_time_capture']:
        raise ValueError('Offline capture requires the source non-realtime lifecycle')
    maximum = check_scene_bounds(args.scene.read_text(encoding='utf-8'), sky)
    material = SourceMaterial(source, args.source, args.node, args.pngjs_module)
    if args.self_test:
        self_test(material, definition, geometry, sky)
    pixels = capture(material, definition, geometry, sky, sky['capture_resolution'], args.capture_time)
    data = pixels.tobytes()
    if args.check:
        if args.output.read_bytes() != data:
            raise ValueError('Installed sky capture differs from the source material')
    else:
        args.output.write_bytes(data)
    report = dict(success=True, format='playground_sky_capture_v1', resolution=sky['capture_resolution'],
                  capture_time=args.capture_time, geometry_maximum_distance=maximum, self_test=args.self_test,
                  check=args.check, sha256=hashlib.sha256(data).hexdigest(), bytes=len(data),
                  maximum_radiance=float(pixels[..., :3].max()), face_mean_rgb=pixels[..., :3].astype(float).mean(axis=(1, 2)).tolist())
    args.report.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(report), flush=True)


if __name__ == '__main__':
    main()
