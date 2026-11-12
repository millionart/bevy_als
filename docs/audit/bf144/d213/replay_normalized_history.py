import gzip
import json
import struct
from pathlib import Path


AUDIT = Path(__file__).resolve().parent


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def read(path):
    raw = path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix(".json.gz").read_bytes())
    return json.loads(raw)


def marker_time(position, markers, duration):
    times = {marker["name"]: marker["time"] for marker in markers}
    previous, following = times[position["previous"]], times[position["next"]]
    if previous >= following:
        previous = f32(previous - duration)
    time = f32(previous + f32(position["alpha"] * f32(following - previous)))
    return f32(time + duration) if time < 0.0 else time


def main():
    request = read(AUDIT / "normalized.request.json")
    source = read(AUDIT / "normalized.native.json")["sync_group_clock_probe"]
    clips = [read(path) for path in (AUDIT.parent / "d212/forward_walk.native.json", AUDIT / "run.native.json")]
    start = f32(request["players"][0]["start"])
    fixtures = []
    stale_seconds = {}
    cleared_sample_record = {}
    for requested, observed in zip(request["samples"], source["samples"], strict=True):
        name = requested["name"]
        assert name == observed["name"]
        frames = []
        previous_normalized = start
        previous_sample = None
        active = False
        boundary = 15 if name.endswith("_15") else 13
        for index, (step, frame) in enumerate(zip(requested["steps"], observed["frames"], strict=True)):
            player = frame["players"][0]
            fixture = {"delta": step["delta_seconds"], "weights": step["weights"], "clock": None,
                       "run": False, "boundary": False, "normalized": previous_normalized}
            if not player["ticked"]:
                assert not frame["order"] and player["position"] == previous_normalized
                assert previous_sample is not None
                active = False
                frames.append(fixture)
                continue

            sample = player["samples"][0]
            is_run = sample["sample_index"] == 2
            clip = clips[int(is_run)]
            timing = clip["sequence_timing"]
            duration = timing["duration"]
            assert sample["animation"] == clip["asset_path"]
            assert player["position"] == f32(sample["time"] / duration)
            projected = f32(previous_normalized * duration)
            if active:
                assert previous_sample is not None and sample["sample_index"] == previous_sample["sample_index"]
                assert sample["previous_time"] == previous_sample["time"], (name, index)
            else:
                assert sample["previous_time"] == projected, (name, index)
                if previous_sample is not None:
                    boundary = index
                    old = previous_sample["time"] if sample["sample_index"] == previous_sample["sample_index"] else f32(start * duration)
                    if old != projected:
                        stale_seconds[name] = [old, projected]
                    if frame["leader"] != 0:
                        assert frame["marker_start"]["valid"]
                        aligned = marker_time(frame["marker_start"], timing["markers"], duration)
                        assert abs(aligned - projected) > 0.2
                        cleared_sample_record[name] = [aligned, projected]
            fixture.update(clock=[sample["previous_time"], sample["time"]], run=is_run,
                           boundary=index == boundary, normalized=player["position"])
            previous_sample = sample
            previous_normalized = player["position"]
            active = True
            frames.append(fixture)
        fixtures.append({"name": name, "frames": frames})

    assert set(stale_seconds) == {"return_leader", "return_follower", "return_run_leader"}
    assert set(cleared_sample_record) == {"return_follower", "return_follower_15"}
    print(json.dumps({"cases": fixtures, "walk": clips[0]["sequence_timing"], "run": clips[1]["sequence_timing"],
                      "sequence": source["players"][1], "native_frames": sum(len(row["frames"]) for row in fixtures),
                      "stale_seconds_ablation": stale_seconds, "clear_sample_record_ablation": cleared_sample_record}, indent=2))


if __name__ == "__main__":
    main()
