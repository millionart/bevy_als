import gzip
import json
import re
import struct
from pathlib import Path


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def load(path):
    data = path.read_bytes() if path.is_file() else gzip.decompress(path.with_suffix(".json.gz").read_bytes())
    return json.loads(data)


directory = Path(__file__).resolve().parent
root = directory.parents[3]
montages = (root / "assets/ALS/Data/AnimationInstance/Grounded/als_standing_montages.ron").read_text()
cases = []
for stance, prefix in (("standing", ""), ("crouching", "Crouch_")):
    request = load(directory / f"{stance}.request.json")["samples"][0]
    rate = f32(request["play_rate"])
    for angle in (90, 180):
        for side in ("left", "right"):
            sequence = f"A_Als_{prefix}Turn_{angle}_{side.title()}"
            native = load(directory / f"{stance}-{angle}-{side}.native.json")
            assert native["asset_path"].endswith(f"/{sequence}.{sequence}")
            probe = native["montage_clock_probe"]
            length = probe["montage_length"]
            match = re.search(rf'^"{sequence}": \(clip: "[^"\n]+", duration: ([\d.]+),', montages, re.M)
            assert match is not None and f32(float(match[1])) == length
            frames = probe["samples"][0]["frames"]
            assert all(frame["valid"] and frame["playing"] and not frame["stopped"] for frame in frames)
            assert [frame["weight"] for frame in frames] == [0.0, 0.0, 0.15625, 0.5, 1.0]
            position = 0.0
            for step, frame in zip(request["steps"], frames, strict=True):
                assert frame["delta_seconds"] == f32(step["delta_seconds"])
                position = f32(position + f32(rate * frame["delta_seconds"]))
                assert frame["position"] == position
            assert frames[0]["position"] == frames[1]["position"] == 0.0
            cases.append({"sequence": sequence, "crouching": stance == "crouching", "angle": angle,
                          "right": side == "right", "length": length, "frames": frames})

assert len(cases) == 8
assert all(case["frames"][0]["weight"] <= f32(2**-23) for case in cases)
print(json.dumps({"cases": cases, "old_ready_instance_removals": 8}, indent=2))
