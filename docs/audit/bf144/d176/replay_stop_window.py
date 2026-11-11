import gzip
import json
import struct
from pathlib import Path


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def load(path):
    data = path.read_bytes() if path.is_file() else gzip.decompress(path.with_suffix(".json.gz").read_bytes())
    return json.loads(data)


def model_stop(start, duration, rate, deltas, remaining_time):
    position = start
    for index, delta in enumerate(deltas):
        position = f32(position + f32(delta * rate))
        if remaining_time:
            if delta > f32(1e-8) and f32(f32(duration - position) / rate) <= f32(1e-4):
                return index
        elif position >= duration:
            return index
    return None


directory = Path(__file__).resolve().parent
original = load(directory.parent / "d175/zero-delta.replay.json")["cases"]
cases = []
old_stops = []
for asset in original:
    stance = "crouching" if asset["crouching"] else "standing"
    side = "right" if asset["right"] else "left"
    stem = f"{stance}-{asset['angle']}-{side}"
    native = load(directory / f"{stem}.native.json")
    request = load(directory / f"{stem}.request.json")
    assert request["dynamic_montage"]["inertial_blend_out"] and request["dynamic_montage"]["blend_out_trigger_seconds"] == 0.0
    assert native["asset_path"].endswith(f"/{asset['sequence']}.{asset['sequence']}")
    probe = native["montage_clock_probe"]
    assert probe["montage_length"] == asset["length"]
    samples = []
    for definition, sample in zip(request["samples"], probe["samples"], strict=True):
        assert definition["name"] == sample["name"]
        frames = sample["frames"]
        deltas = [frame["delta_seconds"] for frame in frames]
        assert deltas == [f32(step["delta_seconds"]) for step in definition["steps"]]
        first = next(index for index, frame in enumerate(frames) if frame["stopped"])
        assert frames[first]["weight"] == 0.0 and not frames[first]["playing"]
        start = f32(definition.get("start_time", 0.0))
        rate = f32(definition["play_rate"])
        expected = model_stop(start, asset["length"], rate, deltas, remaining_time=True)
        assert expected == first, (stem, sample["name"], expected, first)
        old = model_stop(start, asset["length"], rate, deltas, remaining_time=False)
        if old != first:
            old_stops.append({"sequence": asset["sequence"], "sample": sample["name"],
                              "source_frame": first + 1, "old_frame": None if old is None else old + 1})
        if sample["name"] == "playback_window":
            assert first == 1 and deltas[0] == 0.0
            assert f32(asset["length"] - frames[first]["position"]) > f32(1e-4)
        if sample["name"] == "before_window":
            assert first == 1
        samples.append({"name": sample["name"], "start": start, "deltas": deltas, "stop_frame": first})
    cases.append({key: value for key, value in asset.items() if key != "frames"} | {"samples": samples})

assert sum(row["sample"] == "60hz" for row in old_stops) == 6
assert len(cases) == 8
print(json.dumps({"cases": cases, "old_stops": old_stops}, separators=(",", ":")))
