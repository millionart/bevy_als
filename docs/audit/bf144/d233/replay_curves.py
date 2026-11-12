import argparse
import json
import struct
import sys
from pathlib import Path


HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "d232b"))
from replay_pose_chain import read_capture


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def write_request():
    pose = {"translation_cm": [0, 0, 0], "rotation": [0, 0, 0, 1], "scale": [1, 1, 1]}
    inputs = [
        ("sprint_block_2", 0.24301831424236298, 0.24301831424236298, 0.7407407164573669),
        ("sprint_block_4", 0.2274143546819687, 0.2274143546819687, 0.2592592239379883),
        ("first_value", 0.24301831424236298, 0.0, 0.375),
        ("second_value", 0.0, 0.2274143546819687, 0.375),
        ("mixed", -1.0, 90.0, 0.625),
        ("first_endpoint", -1.0, 90.0, 0.0),
        ("second_endpoint", -1.0, 90.0, 1.0),
    ]
    samples = [{"name": name, "profile": "MoveDirectionChange", "mode": "Linear", "duration": 1.0,
                "curve_a": a, "curve_b": b, "from": pose, "to": pose, "deltas": [alpha]}
               for name, a, b, alpha in inputs]
    (HERE / "curves.request.json").write_text(json.dumps({"samples": samples}, indent=2) + "\n")


def replay():
    request = json.loads((HERE / "curves.request.json").read_bytes())["samples"]
    path = HERE / "curves.native.json"
    native = json.loads(path.read_bytes()) if path.exists() else read_capture(path.with_suffix(".json.gz"))
    results = []
    for case, row in zip(request, native["blend_profile_probe"]["samples"], strict=True):
        assert case["name"] == row["name"]
        frame, = row["frames"]
        a, b = map(f32, (case["curve_a"], case["curve_b"]))
        alpha = f32(case["deltas"][0])
        assert frame["alpha"] == frame["two_way_effective_alpha"] == alpha
        value = f32(a + f32(f32(b - a) * alpha))
        assert frame["two_way_curve"] == value
        weighted = f32(f32(a * f32(1.0 - alpha)) + f32(b * alpha))
        results.append({"name": case["name"], "alpha": alpha, "lerp": value, "weighted": weighted})
    assert results[0]["weighted"] == 0.24301832914352417
    assert results[1]["weighted"] == 0.2274143397808075
    assert all(row["lerp"] != row["weighted"] for row in results[:2])
    return {"result": "PASS_TWO_WAY_CURVE_LERP", "cases": results}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--request", action="store_true")
    if parser.parse_args().request:
        write_request()
    else:
        print(json.dumps(replay(), indent=2))
