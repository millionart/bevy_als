import json
import math
import os


root = os.environ.get("BEVY_ALS_ROOT")
if root is None:
    raise RuntimeError("BEVY_ALS_ROOT is required")

source_path = os.path.join(root, "tools", "tmp", "export", "ground_dynamics_curves", "curves.json")
with open(source_path, encoding="utf-8") as stream:
    source = json.load(stream)

key_times = [0.0, 1.0, 2.0, 3.0]
axes = ["x", "y", "z"]


def sample_at(samples, time):
    for sample in samples:
        if abs(sample["time"] - time) < 1e-6:
            return sample
    raise RuntimeError("missing sample at %.6f" % time)


def basis(time):
    return (
        2.0 * time * time * time - 3.0 * time * time + 1.0,
        time * time * time - 2.0 * time * time + time,
        -2.0 * time * time * time + 3.0 * time * time,
        time * time * time - time * time,
    )


def solve_tangents(start_value, end_value, quarter_value, three_quarter_value):
    h00a, h10a, h01a, h11a = basis(0.25)
    h00b, h10b, h01b, h11b = basis(0.75)
    rhs_a = quarter_value - h00a * start_value - h01a * end_value
    rhs_b = three_quarter_value - h00b * start_value - h01b * end_value
    determinant = h10a * h11b - h10b * h11a
    if abs(determinant) <= 1e-12:
        raise RuntimeError("singular Hermite basis")
    start_tangent = (rhs_a * h11b - rhs_b * h11a) / determinant
    end_tangent = (h10a * rhs_b - h10b * rhs_a) / determinant
    return start_tangent, end_tangent


def evaluate(segment, time):
    duration = segment["end"] - segment["start"]
    normalized = (time - segment["start"]) / duration
    h00, h10, h01, h11 = basis(normalized)
    return (
        h00 * segment["start_value"]
        + h10 * duration * segment["start_tangent"]
        + h01 * segment["end_value"]
        + h11 * duration * segment["end_tangent"]
    )


result = {}
for name, curve in source.items():
    samples = curve["samples"]
    axes_result = {}
    for axis in axes:
        segments = []
        for start, end in zip(key_times, key_times[1:]):
            start_value = sample_at(samples, start)[axis]
            end_value = sample_at(samples, end)[axis]
            quarter_value = sample_at(samples, start + 0.25)[axis]
            three_quarter_value = sample_at(samples, start + 0.75)[axis]
            start_tangent, end_tangent = solve_tangents(
                start_value,
                end_value,
                quarter_value,
                three_quarter_value,
            )
            segments.append(
                {
                    "start": start,
                    "end": end,
                    "start_value": start_value,
                    "end_value": end_value,
                    "start_tangent": start_tangent,
                    "end_tangent": end_tangent,
                }
            )
        max_error = 0.0
        for sample in samples:
            time = sample["time"]
            segment = segments[min(int(math.floor(time)), len(segments) - 1)]
            max_error = max(max_error, abs(evaluate(segment, time) - sample[axis]))
        axes_result[axis] = {"segments": segments, "max_error": max_error}
    result[name] = {"asset": curve["asset"], "axes": axes_result}

out_path = os.path.join(root, "tools", "tmp", "export", "ground_dynamics_curves", "cubic.json")
with open(out_path, "w", encoding="utf-8") as stream:
    json.dump(result, stream, indent=1)
print("PYEXPORT " + json.dumps({"out": out_path, "assets": len(result)}))
