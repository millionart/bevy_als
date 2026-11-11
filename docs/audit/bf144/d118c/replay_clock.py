import argparse
import json
import struct
from pathlib import Path


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def blend(alpha, start, target):
    shaped = f32(f32(alpha * alpha) * f32(3.0 - f32(2.0 * alpha)))
    return f32(start + f32(f32(target - start) * shaped))


def replay(sample, settings, length, ablation):
    start_time = position = min(max(f32(sample["start_time"]), 0.0), length)
    rate = f32(sample["play_rate"])
    end = length if rate > 0.0 else 0.0
    weight = alpha = from_weight = elapsed = total_elapsed = carry = 0.0
    duration = remaining = f32(settings["blend_in_seconds"])
    stopped = False
    valid = playing = True
    for step in sample["steps"]:
        delta = f32(step["delta_seconds"])
        if valid:
            stop = step.get("stop_seconds")
            if stop is not None and (not stopped or f32(stop) < duration):
                from_weight, duration = weight, f32(stop)
                alpha = elapsed = 0.0
                remaining = duration
                stopped = True
                if duration <= 0.0:
                    playing = False
            target = 0.0 if stopped else 1.0
            if weight != target:
                elapsed = f32(elapsed + delta)
                if ablation == "ratio_alpha":
                    alpha = min(f32(elapsed / duration), 1.0) if duration else 1.0
                elif remaining > delta:
                    alpha = min(f32(alpha + f32(f32(f32(1.0 - alpha) / remaining) * delta)), 1.0)
                    remaining = f32(remaining - delta)
                else:
                    alpha = 1.0
                    remaining = 0.0
                weight = blend(alpha, from_weight, target)
            total_elapsed = f32(total_elapsed + delta)
            if playing or ablation == "no_end_hold":
                if carry < 10.0:
                    carry = f32(carry + delta)
                for _ in range(9):
                    if carry <= 1e-8 or abs(rate) <= 1e-8:
                        break
                    boundary = f32(end - position)
                    move = f32(carry * rate)
                    reached_end = move >= boundary if rate > 0.0 else move <= boundary
                    if reached_end:
                        move = boundary
                    position = f32(position + move)
                    if ablation == "elapsed_position" and not reached_end:
                        position = min(max(f32(start_time + f32(total_elapsed * rate)), 0.0), length)
                    if not stopped:
                        if ablation == "elapsed_trigger":
                            time_to_end = f32(abs(f32(f32(end - start_time) / rate)) - total_elapsed)
                        else:
                            time_to_end = abs(f32(f32(end - position) / rate))
                        if time_to_end <= f32(1e-4):
                            from_weight = weight
                            duration = remaining = f32(settings["blend_out_seconds"])
                            alpha = elapsed = 0.0
                            stopped = True
                    if reached_end:
                        playing = False
                        position = f32(length - f32(5e-5)) if rate > 0.0 else 0.0
                        carry = 0.0
                        break
                    if move == 0.0:
                        break
                    carry = max(f32(carry - f32(move / rate)), 0.0)
                    if ablation == "single_step":
                        break
                if ablation in ("single_step", "no_frame_carry"):
                    carry = 0.0
            if stopped and weight == 0.0:
                valid = playing = False
        yield {"position":position, "weight":weight, "valid":valid, "playing":playing and valid, "stopped":stopped}


def compare(root):
    stop = ("stop-clock.request.json", "stop-left.native.json", "stop-right.native.json", "crouch-right.native.json")
    step = ("step-clock.request.json", "crouch-step.native.json")
    quick = ("quick-clock.request.json", "crouch-quick.native.json")
    fields = ("position", "weight", "valid", "playing", "stopped")
    modes = ("source", "single_step", "no_frame_carry", "elapsed_position", "elapsed_trigger", "ratio_alpha", "no_end_hold")
    counts = {mode:{field:0 for field in fields} for mode in modes}
    frames = 0
    for request_name, *reports in [stop, step, quick]:
        request = json.loads((root / request_name).read_text())
        for report_name in reports:
            probe = json.loads((root / report_name).read_text(encoding="utf-8-sig"))["montage_clock_probe"]
            assert len(probe["samples"]) == len(request["samples"])
            for native, sample in zip(probe["samples"], request["samples"]):
                assert native["name"] == sample["name"]
                expected = native["frames"]
                frames += len(expected)
                for mode in modes:
                    actual = list(replay(sample, request["dynamic_montage"], probe["montage_length"], mode))
                    assert len(actual) == len(expected)
                    for left, right in zip(actual, expected):
                        for field in fields:
                            counts[mode][field] += left[field] != right[field]
    for mode in ("source", "single_step", "no_frame_carry"):
        assert not any(counts[mode].values()), counts[mode]
    assert counts["elapsed_position"]["position"] > 0
    assert counts["elapsed_trigger"]["stopped"] > 0
    assert counts["ratio_alpha"]["weight"] > 0
    assert counts["no_end_hold"]["position"] > 0
    return {"frames":frames, "differences":counts}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    data = compare(Path(__file__).resolve().parent)
    output = json.dumps(data, indent=2) + "\n"
    if args.output:
        args.output.write_text(output)
    print(output, end="")
