import gzip
import json
import struct
from pathlib import Path


directory = Path(__file__).resolve().parent
request = json.loads((directory / "inertia.request.json").read_bytes())
with gzip.open(directory / "inertia.native.json.gz") as stream:
    native = json.load(stream)["inertialization_probe"]
assert not native["ignore_deficit"] and native["node_property"] == request["node_property"]


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def update_timer(state, delta, requests, clear_inactive):
    duration, elapsed, deficit, active = state
    if requests:
        remaining = max(f32(duration - elapsed), 0.0)
        applied = remaining if active and deficit > 0.0 else 0.0
        if active:
            deficit = remaining
        duration = min(max(f32(f32(requested) - applied), 0.0) for requested in requests)
        elapsed = 0.0
        active = duration > 2 ** -23
    if active:
        elapsed = f32(elapsed + delta)
        if elapsed >= duration:
            deficit = 0.0
            active = False
        else:
            deficit = max(f32(deficit - delta), 0.0)
    elif clear_inactive:
        deficit = 0.0
    return duration, elapsed, deficit, active


cases = []
for sample, raw in zip(request["samples"], native["samples"], strict=True):
    assert sample["name"] == raw["name"]
    fixed = legacy = (0.0, 0.0, 0.0, False)
    frames = []
    for step, frame in zip(sample["steps"], raw["frames"], strict=True):
        delta = f32(step["delta_seconds"])
        requests = step.get("request_durations", [])
        if "request_duration" in step:
            requests = [step["request_duration"]]
        fixed = update_timer(fixed, delta, requests, True)
        legacy = update_timer(legacy, delta, requests, False)
        frames.append({
            "delta_seconds": delta, "request_durations": requests,
            "input_m": f32(step["translation_cm"][0] / 100),
            "native_m": f32(frame["transform"]["translation"]["x"] / 100),
            "expected_duration_seconds": fixed[0], "expected_deficit_seconds": fixed[2],
            "legacy_duration_seconds": legacy[0], "legacy_deficit_seconds": legacy[2],
        })
    assert frames[4]["expected_deficit_seconds"] == 0 and frames[4]["legacy_deficit_seconds"] > 0
    reentry = frames[6]
    assert reentry["native_m"] > 0
    assert reentry["expected_duration_seconds"] == f32(0.2)
    assert reentry["legacy_duration_seconds"] < reentry["delta_seconds"] + 1e-7
    cases.append({"name": sample["name"], "frames": frames})
assert len(cases) == 6
profile_base = f32(0.2)
profile_maximum = f32(0.4)
profile_delta = f32(0.02)
profile_elapsed = f32(f32(0.19) + profile_delta)
profile_deficit = 0.0 if profile_elapsed >= profile_base else f32(f32(0.05) - profile_delta)
legacy_profile_deficit = 0.0 if profile_elapsed >= profile_maximum else f32(f32(0.05) - profile_delta)
assert profile_deficit == 0.0 < legacy_profile_deficit and profile_elapsed < profile_maximum
print(json.dumps({"cases": cases, "legacy_reentry_failures": len(cases),
                  "profile_control": {"base_duration": profile_base, "max_duration": profile_maximum,
                                      "elapsed": profile_elapsed, "deficit": profile_deficit,
                                      "legacy_deficit": legacy_profile_deficit}}, indent=2))
