import json
import math
import struct
from pathlib import Path


def f32(value):
    return struct.unpack("f", struct.pack("f", value))[0]


directory = Path(__file__).resolve().parent


def replay(name):
    report = json.loads((directory / f"{name}.native.json").read_text())
    cases = []
    for sample in report["geometry_query_probe"]["samples"]:
        request = sample["input"]
        scaled = False
        time = None
        if "box_raycast" in sample:
            native = sample["box_raycast"]
            limit = request["box_raycast"]["length_cm"]
            if native["hit"]:
                time = native["distance_cm"]
        else:
            native = sample["mesh_raycast"]
            shape, = [shape for shape in native["shapes"] if shape["triangle_mesh"] and shape["complex"]]
            scaled = "core_inverse_scale" in shape
            mesh = request["mesh_raycast"]
            limit = f32(math.dist(mesh["start_cm"], mesh["end_cm"]))
            visits = shape["ray_visits"]
            assert visits["agrees_with_core"]
            if visits["hit"]:
                found = [visit for visit in visits["visits"] if visit["hit"]]
                time = found[-1]["distance_cm"] / visits["length_scale"]
        allowed = time is not None and (time < limit or (not scaled and time == limit))
        assert native["hit"] == allowed, (request["name"], time, limit)
        cases.append({"name": request["name"], "scaled_wrapper": scaled, "kernel_distance_cm": time,
                      "ray_length_cm": limit, "hit": allowed, "strict_outer_limit": time is not None and time < limit})
    return cases


cases = replay("computed-limit")
assert len(cases) == 12
by_name = {case["name"]: case for case in cases}
assert by_name["stairs_1_at"]["hit"] and by_name["stairs_1.00005_at"]["hit"]
assert not by_name["stairs_2_at"]["hit"] and by_name["stairs_2_at"]["kernel_distance_cm"] == 128.0
assert [case["name"] for case in cases if case["hit"] != case["strict_outer_limit"]] == [
    "box_at", "stairs_1_at", "stairs_1.00005_at",
]
rounding_cases = {case["name"]: case for case in replay("endpoint")}
assert rounding_cases["stairs_1_at"]["kernel_distance_cm"] is None
assert rounding_cases["stairs_1.25_at"]["kernel_distance_cm"] < rounding_cases["stairs_1.25_at"]["ray_length_cm"]
normal = 1.0 / 1.25
foot_offset = -0.8 + (0.134656 / normal - 0.134656)
assert abs(foot_offset - (-0.766336)) < 1e-12
print(json.dumps({"cases": cases, "rounding_controls": list(rounding_cases.values()),
                  "endpoint_rig_offset_m": foot_offset}, indent=2))
