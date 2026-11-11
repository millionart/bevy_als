import json
import math
import struct
from pathlib import Path


def basis(q):
    x, y, z, w = q
    return -x, -z, -y, w


def conjugate(q):
    x, y, z, w = q
    return -x, -y, -z, w


def product(left, right):
    x, y, z, w = left
    a, b, c, d = right
    return (
        w * a + x * d + y * c - z * b,
        w * b - x * c + y * d + z * a,
        w * c + x * b - y * a + z * d,
        w * d - x * a - y * b - z * c,
    )


def roll(q):
    x, y, z, w = q
    return math.degrees(math.atan2(-2 * (w * x + y * z), 1 - 2 * (x * x + y * y)))


directory = Path(__file__).resolve().parent
requests = json.loads((directory / "products.request.json").read_text())["samples"]
samples = json.loads((directory / "products.native.json").read_text())["geometry_query_probe"]["samples"]
cases = []
old_mismatches = []
for request, sample in zip(requests, samples, strict=True):
    name = request["name"]
    assert name == sample["input"]["name"]
    start = request["rotation_xyzw"]
    factors = [factor["rotation_xyzw"] for factor in request["rotation_factors"]]
    source = start
    original = basis(start)
    for factor in factors:
        source = product(source, factor)
        original = product(original, basis(factor))
    expected = tuple(sample["rotation_product"][axis] for axis in "xyzw")
    assert source == expected, name
    if original != basis(expected):
        old_mismatches.append(name)
    cases.append({"name": name, "start": basis(start), "factors": list(map(basis, factors)),
                  "expected": basis(source)})
    if name != "controlled_roll":
        continue

    native_roll = sample["rotation_product_degrees"]["roll"]
    assert roll(source) == native_roll == -180.0
    old_roll = roll(basis(original))
    assert old_roll == 180.0
    target_roll = struct.unpack("<f", struct.pack("<f", -177.4759))[0]
    limit = lambda angle: min(max(target_roll, min(0.0, angle)), max(0.0, angle))
    assert limit(native_roll) == target_roll and limit(old_roll) == 0.0
    half = math.radians(target_roll) / 2
    calf, initial = conjugate(start), conjugate(factors[1])
    target = product(calf, product((-math.sin(half), 0.0, 0.0, math.cos(half)), initial))
    foot = {"calf": basis(calf), "foot": basis(factors[0]), "initial": basis(initial),
            "target": basis(target), "native_roll": native_roll, "original_roll": old_roll}

assert old_mismatches
print(json.dumps({"cases": cases, "foot": foot, "runtime_basis_mismatches": old_mismatches}, indent=2))
