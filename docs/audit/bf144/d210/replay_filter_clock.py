import gzip
import json
import struct
from pathlib import Path


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def filtered(samples, time, window):
    total = summed = 0.0
    for sample in samples:
        if sample is None:
            continue
        stamp, value = sample
        diff = f32(time - stamp)
        if diff > window:
            continue
        age = f32(diff / window)
        weight = f32(1.0 - f32(f32(age * age) * age))
        if weight > 0.0:
            total = f32(total + weight)
            summed = f32(summed + f32(weight * value))
    return f32(summed / total) if total else 0.0


def run(frames, key, window, mode):
    samples = []
    time = output = 0.0
    stack_index = 0
    accepted = False
    result = []
    for frame in frames:
        delta = f32(frame["delta_seconds"])
        value = f32(frame[key])
        if mode == "legacy" and not accepted:
            samples.append((time, value))
            output = value
            accepted = True
        elif delta > f32(1e-6 if mode in ("legacy", "old_gate") else 1e-4):
            if accepted or mode != "old_first_clock":
                time = f32(time + delta)
            accepted = True
            if mode == "ring":
                if not samples:
                    samples = [None] * 10
                samples = [None if sample is not None and f32(time - sample[0]) > window else sample for sample in samples]
                available = [(stack_index + offset) % len(samples) for offset in range(len(samples))]
                index = next((index for index in available if samples[index] is None), len(samples))
                if index == len(samples):
                    samples.extend([None] * 5)
                samples[index] = (time, value)
                stack_index = (index + 1) % len(samples)
            else:
                samples = [sample for sample in samples if f32(time - sample[0]) <= f32(window + f32(1e-6))]
                samples.append((time, value))
            output = filtered(samples, time, window)
        result.append([output, time])
    return result


def read_native(root, name):
    path = root / f"{name}.native.json"
    raw = path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix(".json.gz").read_bytes())
    return json.loads(raw)


def main():
    root = Path(__file__).parent
    windows = [f32(0.2), f32(0.4)]
    cases = []
    summary = {}
    for name in ("edges", "regular"):
        raw = read_native(root, name)
        params = [value["exported_text"] for value in raw["asset_properties"] if value["name"] == "InterpolationParam"]
        assert len(params) == 3 and all(
            f"InterpolationTime={value:.6f}" in entry and "BSIT_Cubic" in entry
            for value, entry in zip(windows, params[:2], strict=True)
        )
        source = raw["blendspace_probe"]["frames"]
        request = json.loads((root / f"{name}.request.json").read_bytes())["frames"]
        assert len(source) == len(request) and all(frame["input"] == step for frame, step in zip(source, request, strict=True))
        variants = {
            mode: list(zip(*(run(request, key, window, mode) for key, window in zip("xy", windows, strict=True)), strict=True))
            for mode in ("ring", "clock_only", "legacy", "old_gate", "old_first_clock")
        }
        native = [[frame["filtered_position"][axis] for axis in "xy"] for frame in source]
        assert [[axis[0] for axis in pair] for pair in variants["ring"]] == native, name
        exact_frames = []
        mismatches = {mode: 0 for mode in ("clock_only", "legacy", "old_gate", "old_first_clock")}
        for index, (step, output) in enumerate(zip(request, native, strict=True)):
            fixed = variants["clock_only"][index]
            mismatches["clock_only"] += [axis[0] for axis in fixed] != output
            for mode in ("legacy", "old_gate", "old_first_clock"):
                mismatches[mode] += [axis[0] for axis in variants[mode][index]] != [axis[0] for axis in fixed]
            if index < 9:
                assert [axis[0] for axis in fixed] == output
                exact_frames.append({"input": [step["x"], step["y"]], "delta": step["delta_seconds"],
                                     "position": output, "time": fixed[0][1]})
        if name == "edges":
            assert native[0] == native[1] == native[2] == [0.0, 0.0]
            assert native[3] == native[4] == native[5] == [f32(0.4), f32(0.77)]
            assert mismatches["old_gate"] > 0
        else:
            assert mismatches["old_gate"] == 0 and mismatches["old_first_clock"] > 0
        cases.append({"name": name, "frames": exact_frames})
        summary[name] = {"native_frames": len(native), "gate_ablation": mismatches["old_gate"],
                         "first_clock_ablation": mismatches["old_first_clock"],
                         "old_gate_and_clock": mismatches["legacy"], "remaining_slot_order": mismatches["clock_only"]}
    print(json.dumps({"windows": windows, "cases": cases, "summary": summary}, indent=2))


if __name__ == "__main__":
    main()
