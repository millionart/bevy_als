import json
import math
import struct
from pathlib import Path


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


SMALL_NUMBER = f32(1e-8)


def dot(a, b):
    return a[0] * b[0] + a[2] * b[2] + a[1] * b[1]


def cross(a, b):
    return (a[2] * b[1] - a[1] * b[2], a[0] * b[2] - a[2] * b[0], a[1] * b[0] - a[0] * b[1])


def sub(a, b):
    return tuple(x - y for x, y in zip(a, b))


def add(a, b):
    return tuple(x + y for x, y in zip(a, b))


def mul(a, factor):
    return tuple(x * factor for x in a)


def runtime(point):
    if isinstance(point, dict):
        return tuple(point[axis] for axis in "xzy")
    return point[0], point[2], point[1]


def safe_normal(a):
    squared = dot(a, a)
    if squared < SMALL_NUMBER:
        return (0.0, 0.0, 0.0)
    return a if squared == 1.0 else mul(a, 1.0 / math.sqrt(squared))


def box_intersects(bounds, start, direction, inverse, length):
    low, high = bounds
    near, far = 0.0, length
    for axis in range(3):
        if abs(direction[axis]) < SMALL_NUMBER:
            if start[axis] < low[axis] or start[axis] > high[axis]:
                return False
            continue
        first = f32(f32(low[axis] - start[axis]) * inverse[axis])
        second = f32(f32(high[axis] - start[axis]) * inverse[axis])
        near = max(near, min(first, second))
        far = min(far, max(first, second))
    return near <= far


def triangle(a, b, c, start, direction, length, cull, tolerance):
    ab, ac = sub(b, a), sub(c, a)
    normal = cross(ab, ac)
    negative = mul(direction, -1.0)
    denominator = dot(negative, normal)
    if (cull and dot(direction, normal) > 0.0) or abs(denominator) < SMALL_NUMBER:
        return None
    inverse = 1.0 / denominator
    offset = sub(start, a)
    time = dot(offset, normal) * inverse
    if time < 0.0 or time > length:
        return None
    relative = cross(negative, offset)
    u, v = dot(ac, relative) * inverse, -dot(ab, relative) * inverse
    if u < -tolerance or u > 1.0 + tolerance or v < -tolerance or u + v > 1.0 + tolerance:
        return None
    return time, mul(safe_normal(normal), math.copysign(1.0, denominator)), (u, v)


def trace_core(mesh, start, direction, length, cull, mode):
    current = f32(length)
    start32 = tuple(map(f32, start))
    direction32 = tuple(map(f32, direction))
    inverse = tuple(f32(1.0 / x) if abs(x) >= SMALL_NUMBER else 0.0 for x in direction32)
    visited = []
    nodes = [0]
    hit = None
    while nodes:
        for child in mesh["nodes"][nodes.pop()]:
            if child is None or not box_intersects(child["bounds_cm"], start32, direction32, inverse, current):
                continue
            first, count = child["index"], child["face_count"]
            if not count:
                nodes.append(first)
                continue
            for face in range(first, first + count):
                if not box_intersects(mesh["face_bounds_cm"][face], start32, direction32, inverse, current):
                    continue
                points = [mesh["vertices_cm"][index] for index in mesh["triangles"][face]]
                if mode == "round_to_meters":
                    points = [tuple(f32(x * f32(0.01)) * 100.0 for x in point) for point in points]
                candidate = triangle(*points, start, direction, current, cull,
                                     0.0 if mode == "strict_barycentric" else SMALL_NUMBER)
                visited.append((face, candidate))
                if candidate is None:
                    continue
                time, normal, _ = candidate
                if mode == "strict_nearest" and hit is not None and time >= hit[1]:
                    continue
                hit = (face, time, add(start, mul(direction, time)), normal)
                current = time if mode == "unrounded_length" else f32(time)
    return (hit if hit is not None and hit[1] <= length else None), visited


def trace_sample(mesh, sample, mode="source"):
    ray = sample["input"]["mesh_raycast"]
    assert sample["input"]["rotation_xyzw"] == [0, 0, 0, 1]
    origin = runtime(ray["start_cm"])
    delta = sub(runtime(ray["end_cm"]), origin)
    length = f32(math.sqrt(dot(delta, delta)))
    translation = runtime(ray["translation_cm"])
    start = sub(origin, translation)
    direction = mul(delta, 1.0 / length)
    scale = tuple(map(f32, runtime(ray["scale"])))
    scaled = any(abs(value - 1.0) >= f32(1e-4) for value in scale)
    inverse = tuple(f32(1.0 / value) for value in scale)
    length_scale = 1.0
    if scaled:
        start = tuple(f32(f32(x) * s) for x, s in zip(start, inverse))
        direction = tuple(f32(f32(x) * s) for x, s in zip(direction, inverse))
        length_scale = math.sqrt(dot(direction, direction))
        direction = mul(direction, 1.0 / length_scale)
    core_length = length * length_scale
    hit, visits = trace_core(mesh, start, direction, core_length, mode != "double_sided", mode)
    if hit is None:
        return None, (start, direction, core_length, visits)
    face, time, point, normal = hit
    if scaled:
        time *= 1.0 / length_scale
        if time >= length:
            return None, (start, direction, core_length, visits)
        point = tuple(f32(f32(x) * s) for x, s in zip(point, scale))
        normal = tuple(f32(f32(x) * s) for x, s in zip(normal, inverse))
        squared = f32(f32(f32(normal[0] * normal[0]) + f32(normal[2] * normal[2])) + f32(normal[1] * normal[1]))
        reciprocal = f32(1.0 / f32(math.sqrt(squared)))
        normal = tuple(f32(x * reciprocal) for x in normal)
    return (face, f32(time), add(point, translation), safe_normal(normal)), (start, direction, core_length, visits)


directory = Path(__file__).resolve().parent
root = directory.parents[3]
meshes = json.loads((root / "assets/ALSExtras/Environment/Meshes/playground_mesh_ray_data.runtime.json").read_text())
cases = []
ablations = {name: [] for name in ("strict_nearest", "strict_barycentric", "unrounded_length", "round_to_meters", "double_sided")}
for item, filename in [("d162a", "edge-visits"), ("d159", "computed-limit"), ("d159", "endpoint"), ("d161", "barrel-ray"), ("d162b", "rig-edges")]:
    report = json.loads((directory / f"../{item}/{filename}.native.json").read_text())
    for sample in report["geometry_query_probe"]["samples"]:
        if "mesh_raycast" not in sample:
            continue
        request = sample["input"]["mesh_raycast"]
        asset = request["asset_path"].rsplit(".", 1)[1]
        mesh = meshes[asset]
        name = sample["input"]["name"]
        native = sample["mesh_raycast"]
        shape, = [shape for shape in native["shapes"] if shape["triangle_mesh"] and shape["complex"]]
        expected = shape["ray_visits"]
        hit, (start, direction, length, visits) = trace_sample(mesh, sample)
        assert start == runtime(expected["start_cm"]) and direction == runtime(expected["direction"]) and length == expected["length_cm"], name
        assert [face for face, _ in visits] == [visit["internal_face"] for visit in expected["visits"]], name
        for (face, result), visit in zip(visits, expected["visits"]):
            assert (result is not None) == visit["hit"], (name, face)
            if result is not None:
                assert result[0] == visit["distance_cm"] and result[2] == tuple(visit["barycentric_uv"]), (name, face)
        assert (hit is not None) == native["hit"], name
        if hit is not None:
            face, distance, point, normal = hit
            assert face == expected["selected_internal_face"] and distance == native["distance_cm"], name
            assert math.dist(point, runtime(native["position_cm"])) <= 1e-10, name
            assert math.dist(normal, runtime(native["normal"])) <= 1e-14, name
        cases.append({
            "fixture": filename, "case": name, "mesh": asset, "visits": len(visits),
            "scaled": "core_inverse_scale" in shape,
            "start_cm": runtime(request["start_cm"]), "end_cm": runtime(request["end_cm"]),
            "translation_cm": runtime(request["translation_cm"]), "scale": runtime(request["scale"]),
            "rig_center_m": sample["input"].get("rig_center_m"),
            "hit": None if hit is None else {
                "face": hit[0], "position_cm": runtime(native["position_cm"]), "normal": runtime(native["normal"]),
            },
        })
        for mode, failures in ablations.items():
            changed, _ = trace_sample(mesh, sample, mode)
            if changed != hit:
                failures.append(f"{filename}/{name}")
assert all(ablations.values()), ablations
print(json.dumps({"cases": cases, "ablation_differences": ablations}, indent=2))
