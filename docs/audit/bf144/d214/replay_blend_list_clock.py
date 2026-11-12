import gzip
import json
import struct
from pathlib import Path


AUDIT = Path(__file__).resolve().parent


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def read(name):
    path = AUDIT / name
    raw = path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix(".json.gz").read_bytes())
    return json.loads(raw)


def blend_alpha(mode, alpha):
    alpha = min(max(alpha, 0.0), 1.0)
    square = f32(alpha * alpha)
    if mode == "Cubic":
        cube = f32(square * alpha)
        return min(max(f32(f32(-2.0 * cube) + f32(3.0 * square)), 0.0), 1.0)
    if mode == "HermiteCubic":
        return f32(square * f32(3.0 - f32(2.0 * alpha)))
    return alpha


def simulate(steps, times, mode, variant="source"):
    weights = [1.0] + [0.0] * (len(times) - 1)
    clocks = [{"value": weight} for weight in weights]
    last = None
    duration = 0.0
    frames = []
    for step in steps:
        active = step["active"]
        delta = f32(step["delta_seconds"])
        changed = active != last
        if changed:
            duration = f32(times[active] * f32(1.0 - weights[active])) if last is not None else 0.0
            last = active
        for index, clock in enumerate(clocks):
            target = float(index == active)
            if changed:
                begin = weights[index]
                span = f32(target - begin)
                threshold = f32(1e-6) if variant == "tiny" else 0.0
                alpha = 1.0 if duration <= threshold or span == 0.0 else min(max(f32(f32(clock["value"] - begin) / span), 0.0), 1.0)
                remaining = f32(duration * f32(1.0 - alpha))
                clock.update(begin=begin, span=span, alpha=alpha, remaining=remaining,
                             elapsed=0.0, initial=alpha, initial_time=remaining)
                clock["value"] = f32(begin + f32(span * blend_alpha(mode, alpha)))
            terminal = f32(clock["begin"] + f32(clock["span"] * blend_alpha(mode, 1.0)))
            if clock["value"] != terminal:
                if clock["remaining"] > delta:
                    prior = clock["remaining"]
                    clock["remaining"] = f32(prior - delta)
                    if variant == "elapsed":
                        clock["elapsed"] = f32(clock["elapsed"] + delta)
                        factor = f32(clock["elapsed"] / clock["initial_time"])
                        clock["alpha"] = f32(clock["initial"] + f32(f32(1.0 - clock["initial"]) * factor))
                    else:
                        clock["alpha"] = f32(clock["alpha"] + f32(f32(f32(1.0 - clock["alpha"]) / prior) * delta))
                    clock["alpha"] = min(max(clock["alpha"], 0.0), 1.0)
                else:
                    clock["remaining"] = 0.0
                    clock["alpha"] = 1.0
                clock["value"] = f32(clock["begin"] + f32(clock["span"] * blend_alpha(mode, clock["alpha"])))
            weights[index] = clock["value"]
        total = 0.0
        for weight in weights:
            total = f32(total + weight)
        if total > f32(1e-5) and abs(f32(total - 1.0)) > f32(1e-5):
            reciprocal = f32(1.0 / total)
            weights = [f32(weight * reciprocal) for weight in weights]
        frames.append({"duration": duration, "weights": weights.copy(), "active": active, "delta": delta})
    return frames


def main():
    result = []
    summary = {}
    for name in ("gait", "hermite"):
        request = read(f"{name}.request.json")
        native = read(f"{name}.native.json")
        mode = request["mode"]
        times = list(map(f32, request["blend_times"]))
        if name == "gait":
            node = next(node for graph in native["graphs"] for node in graph["nodes"]
                        if node["node_guid"] == "d68d1f35-4127-d675-2dfd-8697d49c44ad")
            fields = next(prop for prop in node["properties"] if prop["name"] == "Node")["fields"]
            assert next(field for field in fields if field["name"] == "BlendType")["exported_text"] == mode
            exposed = {pin["name"]: pin["default_value"] for pin in node["pins"] if pin["name"].startswith("BlendTime_")}
            assert [f32(float(exposed[f"BlendTime_{index}"])) for index in range(len(times))] == times
        cases = []
        differences = {"elapsed": 0, "tiny": 0, "other_mode": 0}
        for requested, observed in zip(request["samples"], native["blend_list_probe"]["samples"], strict=True):
            assert requested["name"] == observed["name"]
            frames = simulate(requested["steps"], times, mode)
            for index, (expected, frame) in enumerate(zip(frames, observed["frames"], strict=True)):
                assert all(expected[key] == frame[key] for key in ("duration", "weights", "active")), (name, requested["name"], index, expected, frame["weights"])
            alternate = "Cubic" if mode == "HermiteCubic" else "HermiteCubic"
            for variant in differences:
                altered = simulate(requested["steps"], times, alternate if variant == "other_mode" else mode, variant)
                differences[variant] += sum(old != restored for old, restored in zip(altered, frames, strict=True))
            cases.append({"name": observed["name"], "frames": frames})
        assert differences["elapsed"] and differences["other_mode"]
        if name == "gait":
            assert differences["tiny"] and cases[0]["frames"][2]["duration"] == f32(1.7881394143159923e-8)
        result.append({"mode": mode, "times": times, "cases": cases})
        summary[name] = {"frames": sum(len(case["frames"]) for case in cases), "ablations": differences}
    print(json.dumps({"probes": result, "summary": summary}, indent=2))


if __name__ == "__main__":
    main()
