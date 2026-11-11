import gzip
import json
import struct
import sys
from pathlib import Path


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def basis(q):
    x, y, z, w = q
    return -x, -z, -y, w


def inverse(q):
    x, y, z, w = q
    return -x, -y, -z, w


def product(left, right, grouped=False):
    x, y, z, w = left
    a, b, c, d = right
    terms = (
        (w * a, x * d, y * c, -z * b),
        (w * b, -x * c, y * d, z * a),
        (w * c, x * b, -y * a, z * d),
        (w * d, -x * a, -y * b, -z * c),
    )
    result = []
    for row in terms:
        first, second, third, fourth = map(f32, row)
        first_pair = f32(first + second)
        result.append(f32(first_pair + f32(third + fourth)) if grouped
                      else f32(f32(first_pair + third) + fourth))
    return tuple(result)


def sample(name, left, right):
    return {"name": name, "rotation_xyzw": left,
            "rotation_factors": [{"rotation_xyzw": right}], "capture_float_product": True}


directory = Path(__file__).resolve().parent
if sys.argv[1:] == ["--request"]:
    recording = json.loads((directory.parent / "451b/replay-inputs.json").read_text())
    cases = []
    for frame in (0, 19, 139, 317):
        row = recording["frames"][frame]
        assert row["source_frame"] == frame
        pose = {bone["name"]: basis(bone["rotation"]) for bone in row["current_base"]["bones"]}
        for parent_name, child_name in (("pelvis", "thigh_l"), ("clavicle_l", "upperarm_l"),
                                        ("upperarm_l", "lowerarm_l")):
            parent, child = pose[parent_name], pose[child_name]
            name = f"{frame}/{parent_name}/{child_name}"
            cases.append(sample(name + "/component", parent, child))
            cases.append(sample(name + "/local", inverse(parent), product(parent, child)))
    print(json.dumps({"samples": cases}, indent=2))
    sys.exit()

requests = json.loads((directory / "layered.request.json").read_text())["samples"]
report = directory / "layered.native.json"
native_data = report.read_bytes() if report.is_file() else gzip.decompress(report.with_suffix(".json.gz").read_bytes())
native = json.loads(native_data)["geometry_query_probe"]["samples"]
cases = []
old_component_mismatches = 0
old_local_mismatches = 0
component_only_mismatches = 0
local_only_mismatches = 0
for component, local in zip(native[::2], native[1::2], strict=True):
    component_request, local_request = requests[len(cases) * 2:len(cases) * 2 + 2]
    assert component_request["name"] == component["input"]["name"]
    assert local_request["name"] == local["input"]["name"]
    parent = tuple(component_request["rotation_xyzw"])
    child = tuple(component_request["rotation_factors"][0]["rotation_xyzw"])
    expected_mesh = tuple(component["rotation_product_f32"][axis] for axis in "xyzw")
    expected_local = tuple(local["rotation_product_f32"][axis] for axis in "xyzw")
    assert product(parent, child) == expected_mesh
    assert tuple(local_request["rotation_factors"][0]["rotation_xyzw"]) == expected_mesh
    assert product(inverse(parent), expected_mesh) == expected_local
    old_mesh = product(basis(parent), basis(child), grouped=True)
    old_local = product(inverse(basis(parent)), old_mesh, grouped=True)
    old_component_mismatches += old_mesh != basis(expected_mesh)
    old_local_mismatches += old_local != basis(expected_local)
    component_only_mismatches += basis(product(inverse(parent), basis(old_mesh))) != basis(expected_local)
    local_only_mismatches += product(inverse(basis(parent)), basis(expected_mesh), grouped=True) != basis(expected_local)
    cases.append({"name": component_request["name"], "parent": basis(parent), "child": basis(child),
                  "mesh": basis(expected_mesh), "local": basis(expected_local)})

assert len(cases) * 2 == len(requests)
assert all((old_component_mismatches, old_local_mismatches, component_only_mismatches, local_only_mismatches))
print(json.dumps({"cases": cases, "old_component_mismatches": old_component_mismatches,
                  "old_local_mismatches": old_local_mismatches, "component_only_mismatches": component_only_mismatches,
                  "local_only_mismatches": local_only_mismatches}, indent=2))
