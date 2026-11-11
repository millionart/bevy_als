import json
import struct
from pathlib import Path


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def replay(steps, length, marker, mode):
    weight = 1.0
    position = 0.0
    fade = None
    ended = False
    rate = f32(1.2)
    natural_window = f32(0.3)
    for step in steps:
        delta = f32(step["delta_seconds"])
        if weight > 0.0:
            duration = step.get("stop_seconds")
            if duration is not None and (fade is None or f32(duration) < fade[0]):
                fade = (f32(duration), f32(duration), 0.0, weight)
            if fade is not None:
                duration, remaining, alpha, start = fade
                if remaining > delta:
                    alpha = min(f32(alpha + f32(f32(f32(1.0 - alpha) / remaining) * delta)), 1.0)
                    remaining = f32(remaining - delta)
                else:
                    alpha = 1.0
                    remaining = 0.0
                squared = f32(alpha * alpha)
                cubic = f32(f32(-2.0 * f32(squared * alpha)) + f32(3.0 * squared))
                weight = f32(start + f32(-start * min(max(cubic, 0.0), 1.0)))
                fade = (duration, remaining, alpha, start)
            if not ended or mode == "no_hold":
                remaining_time = delta
                for _ in range(9):
                    if remaining_time <= 1e-8:
                        break
                    movement = f32(remaining_time * rate)
                    if position < marker <= f32(position + movement):
                        movement = f32(marker - position)
                    to_end = f32(length - position)
                    reaches_end = movement >= to_end
                    position = f32(position + min(movement, to_end))
                    until_end = f32(f32(length - position) / rate)
                    if fade is None and until_end <= natural_window:
                        fade = (until_end, until_end, 0.0, weight)
                        if until_end <= 0:
                            weight = 0.0
                    if reaches_end:
                        ended = True
                        position = length if mode == "no_offset" else f32(length - f32(5e-5))
                        break
                    remaining_time = max(f32(remaining_time - f32(movement / rate)), 0.0)
        yield position, weight, not ended and weight > 0.0


def report(root):
    modes = ("source", "no_offset", "no_hold")
    differences = {mode:0 for mode in modes}
    verified = {"d118a":0, "d120":0}
    active_held = 0
    for face in ("front","back"):
        prior = root.parent / "d118a"
        notify = json.loads((prior / f"{face}-notify.native.json").read_text())["notify_clock_probe"]
        markers = {e["end_trigger_time"] for case in notify["cases"] for e in case["events"]}
        assert len(markers) == 1
        marker = markers.pop()
        for folder in (prior, root):
            probe = json.loads((folder / f"{face}.native.json").read_text())["montage_clock_probe"]
            request = json.loads((folder / "clock.request.json").read_text())
            assert len(probe["samples"]) == len(request["samples"])
            for source, native in zip(request["samples"], probe["samples"]):
                assert source["name"] == native["name"]
                frames = native["frames"]
                assert len(source["steps"]) == len(frames)
                expected = [(f["position"],f["weight"],f["playing"]) for f in frames]
                verified[folder.name] += len(frames)
                active_held += sum(not f["playing"] and f["valid"] for f in frames)
                for mode in modes:
                    actual = list(replay(source["steps"], probe["montage_length"], marker, mode))
                    differences[mode] += sum(a != b for a,b in zip(actual, expected))
    assert differences["source"] == 0, differences
    assert differences["no_offset"] > 0 and differences["no_hold"] > 0
    return {"verified_frames":verified, "active_terminal_hold_frames":active_held, "frames_with_differences":differences}


if __name__ == "__main__":
    print(json.dumps(report(Path(__file__).resolve().parent),indent=2))
