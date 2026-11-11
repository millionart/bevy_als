import gzip
import json
import math
import struct
from pathlib import Path


directory = Path(__file__).resolve().parent
request = json.loads((directory / "inertia.request.json").read_bytes())
with gzip.open(directory / "inertia.native.json.gz") as stream:
    native = json.load(stream)["inertialization_probe"]
assert native["node_property"] == request["node_property"]


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def runtime(quaternion):
    return [f32(-quaternion[0]), f32(-quaternion[2]), f32(-quaternion[1]), f32(quaternion[3])]


def multiply(a, b):
    x, y, z, w = a
    xx, yy, zz, ww = b
    return [w * xx + x * ww + y * zz - z * yy,
            w * yy - x * zz + y * ww + z * xx,
            w * zz + x * yy - y * xx + z * ww,
            w * ww - x * xx - y * yy - z * zz]


def normalize(q):
    length = math.sqrt(sum(value * value for value in q))
    return [value / length for value in q]


def unwind(value):
    if value > f32(math.pi):
        return value - f32(math.tau)
    if value < -f32(math.pi):
        return value + f32(math.tau)
    return value


def difference(previous, older, target, delta, normalize_difference):
    inverse = [-target[0], -target[1], -target[2], target[3]]
    current = multiply(previous, inverse)
    prior = multiply(older, inverse)
    if normalize_difference:
        current, prior = normalize(current), normalize(prior)
    angle = f32(unwind(f32(2 * math.acos(max(-1.0, min(1.0, current[3]))))))
    xyz = current[:3]
    axis = normalize(xyz) if sum(value * value for value in xyz) >= 1e-8 else [1.0, 0.0, 0.0]
    if angle < 0:
        axis = [-value for value in axis]
        angle = -angle
    projected = sum(a * b for a, b in zip(prior[:3], axis))
    previous_angle = f32(unwind(2 * math.atan2(projected, prior[3])))
    speed = f32(f32(unwind(f32(angle - previous_angle))) / delta) if angle > 1e-4 and delta > 1e-4 else 0.0
    return axis, angle, speed


def pose_rotation(axis, angle, speed, target, time, duration):
    speed = min(speed, 0.0)
    if speed < -1e-4:
        duration = min(duration, -5 * angle / speed)
    if time >= duration - 1e-7:
        return normalize(target)
    acceleration = max((-8 * duration * speed - 20 * angle) / duration ** 2, 0.0)
    a = -(acceleration * duration ** 2 + 6 * duration * speed + 12 * angle) / (2 * duration ** 5)
    b = (3 * acceleration * duration ** 2 + 16 * duration * speed + 30 * angle) / (2 * duration ** 4)
    c = -(3 * acceleration * duration ** 2 + 12 * duration * speed + 20 * angle) / (2 * duration ** 3)
    offset = f32(((((a * time + b) * time + c) * time + acceleration / 2) * time + speed) * time + angle)
    sine, cosine = math.sin(offset / 2), math.cos(offset / 2)
    return normalize(multiply([f32(value) * sine for value in axis] + [cosine], target))


def rotation_distance(a, b):
    return min(math.dist(a, b), math.dist(a, [-value for value in b]))


cases = []
for sample, observed in zip(request["samples"], native["samples"], strict=True):
    assert sample["name"] == observed["name"]
    rotations = [[f32(frame["transform"]["rotation"][axis]) for axis in "xyzw"] for frame in observed["frames"]]
    indices = [2, 3] if sample["name"] == "consecutive_requests" else [2]
    for index in indices:
        step = sample["steps"][index]
        previous, older = rotations[index - 1], rotations[index - 2]
        target = step["rotation"]
        delta = f32(sample["steps"][index - 1]["delta_seconds"])
        elapsed = f32(step["delta_seconds"])
        duration = f32(step["request_duration"])
        fixed = difference(previous, older, target, delta, False)
        old = difference(previous, older, target, delta, True)
        actual = pose_rotation(*fixed, target, elapsed, duration)
        legacy = pose_rotation(*old, target, elapsed, duration)
        error = rotation_distance(actual, rotations[index])
        cases.append({"name": sample["name"], "frame": index, "older": runtime(older),
                      "previous": runtime(previous), "target": runtime(target), "delta": delta,
                      "elapsed": elapsed, "duration": duration, "native_rotation": runtime(rotations[index]),
                      "projected_error": error,
                      "normalized_difference_error": rotation_distance(legacy, rotations[index])})
assert all(case["normalized_difference_error"] > 1e-4 for case in cases[:3])
assert all(case["projected_error"] < 1.5e-5 for case in cases), cases
print(json.dumps({"cases": cases}, indent=2))
