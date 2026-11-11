import gzip
import json
import math
import struct
import sys
from pathlib import Path


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def unwind(angle):
    while angle > 180.0:
        angle = f32(angle - 360.0)
    while angle < -180.0:
        angle = f32(angle + 360.0)
    return angle


def remap(angle):
    return f32(angle - 360.0) if angle > 175.0 else angle


directory = Path(__file__).resolve().parent
if sys.argv[1:] == ["--request"]:
    yaw175 = [0.0, 0.9990482330322266, 0.0, 0.04361932352185249]
    yaw5 = [0.0, 0.04361937940120697, 0.0, 0.9990479946136475]
    yaw_negative5 = [0.0, -0.04361937567591667, 0.0, 0.9990479946136475]
    samples = []
    for name, runtime, input_degrees, target_radians, mode in (
        ("175_input", yaw175, 0.0, None, "velocity"),
        ("175_target", yaw175, None, 0.0, "velocity"),
        ("175_below", yaw175, 0.0001, None, "velocity"),
        ("175_above", yaw175, -0.0001, None, "velocity"),
        ("zero_input", yaw5, 5.0, None, "velocity"),
        ("zero_target", yaw_negative5, None, f32(-5.0 * f32(math.pi / 180.0)), "velocity"),
        ("ordinary", yaw5, -45.0, None, "velocity"),
        ("aiming", yaw175, 0.0, None, "aiming"),
        ("exact_zero", [0.0, 0.0, 0.0, 1.0], 0.0, None, "velocity"),
    ):
        x, y, z, w = runtime
        samples.append({"name": name, "rotation_xyzw": [z, -x, -y, w], "actor": runtime,
                        "input_degrees": input_degrees, "target_radians": target_radians, "mode": mode})
    print(json.dumps({"samples": samples}, indent=2))
    sys.exit()

requests = json.loads((directory / "yaw.request.json").read_text())["samples"]
path = directory / "yaw.native.json"
native = json.loads(path.read_bytes() if path.is_file() else gzip.decompress(path.with_suffix(".json.gz").read_bytes()))
transition_settings = next(value for value in native["asset_properties"] if value["name"] == "Transitions")
rate = next(value for value in transition_settings["fields"] if value["name"] == "QuickStopPlayRate")
rates = [child["numeric_value"] for child in rate["fields"]]
assert rates == [1.75, 3.0]
cases = []
original_side_failures = []
degrees = f32(180.0 / math.pi)
radians = f32(math.pi / 180.0)
for request, row in zip(requests, native["geometry_query_probe"]["samples"], strict=True):
    assert request["name"] == row["input"]["name"]
    x, y, z, w = request["rotation_xyzw"]
    actor_yaw = row["rotator_degrees"]["yaw"]
    assert abs(actor_yaw - math.atan2(2.0 * (w * z + x * y), 1.0 - 2.0 * (y * y + z * z)) * (180.0 / math.pi)) < 1e-12
    selected = request["input_degrees"]
    target = request["target_radians"]
    source_selected = f32(-selected) if selected is not None else f32(-math.degrees(target))
    angle = remap(unwind(f32(source_selected - actor_yaw))) if request["mode"] == "velocity" else 0.0
    alpha = f32(abs(angle) / 180.0)
    play_rate = f32(rates[0] + f32((rates[1] - rates[0]) * alpha))
    runtime_y, runtime_w = request["actor"][1], request["actor"][3]
    forward_x = f32(-runtime_y * f32(2.0 * runtime_w))
    forward_z = f32(f32(runtime_y * runtime_y) - f32(runtime_w * runtime_w))
    old_actor = -f32(f32(math.atan2(-forward_x, -forward_z)) * degrees)
    old_selected = -f32((f32(selected * radians) if selected is not None else target) * degrees)
    old_difference = f32(old_selected - old_actor)
    old_angle = f32(old_difference % 360.0)
    if old_angle > 180.0:
        old_angle = f32(old_angle - 360.0)
    old_angle = remap(old_angle) if request["mode"] == "velocity" else 0.0
    if (old_angle > 0.0) != (angle > 0.0):
        original_side_failures.append(request["name"])
    cases.append({"name": request["name"], "actor": request["actor"], "input_degrees": selected,
                  "target_radians": target, "mode": request["mode"], "source_actor_yaw": actor_yaw,
                  "remaining_angle": angle, "right": angle > 0.0, "play_rate": play_rate,
                  "original_angle": old_angle})

assert {"175_input", "175_target", "zero_input", "zero_target"} <= set(original_side_failures)
assert cases[0]["remaining_angle"] == 175.0 and cases[-1]["remaining_angle"] == 0.0
print(json.dumps({"cases": cases, "old_side_mismatches": original_side_failures}, indent=2))
