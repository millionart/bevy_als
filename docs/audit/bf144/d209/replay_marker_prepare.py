import gzip
import json
import struct
from pathlib import Path


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def marker_indices(markers, duration, time):
    next_index = next((index for index, marker in enumerate(markers) if marker["time"] >= time), len(markers))
    previous_index = (next_index - 1) % len(markers)
    previous = markers[previous_index]["time"]
    following = markers[next_index % len(markers)]["time"]
    if next_index == 0:
        previous = f32(previous - duration)
    if next_index == len(markers):
        following = f32(following + duration)
    return previous_index, next_index % len(markers), f32(previous - time), f32(following - time)


def marker_position(markers, duration, time, previous_index, next_index):
    previous = markers[previous_index]["time"]
    following = markers[next_index]["time"]
    if previous > following:
        if previous > time:
            previous = f32(previous - duration)
        if following < time:
            following = f32(following + duration)
    elif previous > time:
        time = f32(time + duration)
    alpha = f32(f32(time - previous) / f32(following - previous))
    return [markers[previous_index]["name"], markers[next_index]["name"], alpha]


def advance(markers, duration, time, delta, record):
    previous_index, next_index, previous_distance, next_distance = record
    remaining = delta
    while remaining > next_distance:
        time = markers[next_index]["time"]
        remaining = f32(remaining - next_distance)
        previous_index = next_index
        previous_distance = 0.0
        next_index = (next_index + 1) % len(markers)
        following = f32(markers[next_index]["time"] + (duration if next_index == 0 else 0.0))
        next_distance = f32(following - time)
    time = f32(time + remaining)
    assert time < duration
    return time, (previous_index, next_index, f32(previous_distance - remaining), f32(next_distance - remaining))


def run(steps, index, player, *, reset_at_gap):
    duration, markers = player["duration"], player["markers"]
    time = 0.0
    prior_active = False
    record = None
    frames = []
    for step in steps:
        if step["weights"][index] is None:
            frames.append(None)
            prior_active = False
            continue
        if record is None or (reset_at_gap and not prior_active):
            record = marker_indices(markers, duration, time)
        delta = f32(step["delta_seconds"])
        start = marker_position(markers, duration, time, record[0], record[1])
        previous = time
        time, record = advance(markers, duration, time, delta, record)
        end = marker_position(markers, duration, time, record[0], record[1])
        frames.append({"delta": delta, "time": [previous, time], "start": start, "end": end,
                       "reset": not prior_active})
        prior_active = True
    return frames


def main():
    root = Path(__file__).parent
    path = root / "marker.native.json"
    raw = path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix(".json.gz").read_bytes())
    native = json.loads(raw)
    lifecycle = native["cached_pose_lifecycle_probe"]["frames"]
    assert [lifecycle[index]["child_position"] for index in (3, 4, 5)] == [f32(0.6)] * 3
    assert lifecycle[6]["child_initializations"] == 1 and lifecycle[6]["child_updates"] == 5
    assert lifecycle[6]["child_position"] == f32(f32(0.6) + f32(0.1))
    assert lifecycle[8]["child_initializations"] == 2
    source = native["sync_group_clock_probe"]
    request = json.loads((root / "marker.request.json").read_bytes())
    cases = []
    differences = {}
    for requested, observed in zip(request["samples"], source["samples"], strict=True):
        name = requested["name"]
        assert name == observed["name"]
        index, _, gap = map(int, name.split("_"))
        player = source["players"][index]
        frames = run(requested["steps"], index, player, reset_at_gap=True)
        stale = run(requested["steps"], index, player, reset_at_gap=False)
        for frame, native_frame in zip(frames, observed["frames"], strict=True):
            clock = native_frame["players"][index]
            if frame is None:
                assert native_frame["order"] == [] and not clock["ticked"]
                continue
            assert native_frame["leader"] == index and clock["ticked"]
            assert frame["time"] == [clock["previous"], clock["position"]], name
            for position in ("start", "end"):
                marker = native_frame[f"marker_{position}"]
                assert marker["valid"] and frame[position] == [marker["previous"], marker["next"], marker["alpha"]], (name, position)
        count = sum(current != old for current, old in zip(frames, stale, strict=True))
        if count:
            assert gap
            differences[name] = count
        if count or name in ("1_38_0", "2_35_0"):
            cases.append({"name": name, "duration": player["duration"], "markers": player["markers"], "frames": frames})
    assert differences == {"1_38_1": 5, "2_34_1": 4, "2_35_1": 5}, differences
    print(json.dumps({"cases": cases, "native_frames": sum(len(case["frames"]) for case in source["samples"]),
                      "late_clear_ablation": differences}, indent=2))


if __name__ == "__main__":
    main()
