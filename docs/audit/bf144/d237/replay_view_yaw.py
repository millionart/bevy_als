import argparse
import gzip
import json
import math
import struct
from pathlib import Path


HERE = Path(__file__).resolve().parent


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def unwind(angle):
    while angle > 180.0:
        angle = f32(angle - 360.0)
    while angle < -180.0:
        angle = f32(angle + 360.0)
    return angle


def direction(angle):
    if abs(angle) <= 75.0:
        return "Forward"
    if abs(angle) <= 115.0:
        return "Right" if angle > 0.0 else "Left"
    return "Backward"


def request():
    original = json.loads((HERE.parent / "447d/request.json").read_bytes())
    view = f32(math.radians(120.0))
    for index, (velocity, radians) in enumerate(((45.0, view), (5.0, view), (-45.0, -view), (-5.0, -view))):
        original["samples"].append({"name": f"movement_{index}", "rotation_xyzw": [0, 0, 0, 1],
                                    "yaw_difference": {"velocity_yaw_degrees": velocity, "view_yaw_radians": radians}})
    (HERE / "yaw.request.json").write_text(json.dumps(original, separators=(",", ":")) + "\n")


def replay():
    expected = json.loads((HERE / "yaw.request.json").read_bytes())["samples"]
    path = HERE / "yaw.native.json"
    native = json.loads(path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix(".json.gz").read_bytes()))
    changed = {"head": 0, "movement": 0}
    witnesses = []
    for case, row in zip(expected, native["geometry_query_probe"]["samples"], strict=True):
        assert case == row["input"]
        angles = case["yaw_difference"]
        reference = f32(angles["velocity_yaw_degrees"])
        view = angles["view_yaw_radians"]
        degrees = math.degrees(view)
        assert degrees == row["view_yaw_degrees"]
        corrected = unwind(f32(reference - degrees))
        assert corrected == row["yaw_difference"]
        early = unwind(f32(reference - f32(view * f32(180.0 / math.pi))))
        if case["name"].startswith("movement_"):
            actual = direction(-corrected)
            wrong = direction(-early)
            changed["movement"] += actual != wrong
            witnesses.append({"case": case["name"], "angle": -corrected, "old_angle": -early,
                              "direction": actual, "old_direction": wrong})
        else:
            changed["head"] += corrected != early
            if case["name"] == "actual-target-query-79":
                assert corrected == 45.0 and early == 44.999996185302734
                witnesses.append({"case": case["name"], "angle": corrected, "old_angle": early})
    assert changed == {"head": 319, "movement": 4}
    return {"result": "PASS_VIEW_YAW_LATE_CAST", "native_cases": len(expected), "early_cast_differences": changed,
            "witnesses": witnesses}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--request", action="store_true")
    if parser.parse_args().request:
        request()
    else:
        print(json.dumps(replay(), indent=2))
