import gzip
import json
import re
import struct
from pathlib import Path


AUDIT = Path(__file__).resolve().parent
ROOT = AUDIT.parents[3]


def read(name):
    path = AUDIT / name
    if path.is_file():
        return json.loads(path.read_bytes())
    return json.loads(gzip.decompress(path.with_suffix(".json.gz").read_bytes()))


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def lerp(left, right, alpha):
    return f32(left + f32(f32(right - left) * alpha))


def evaluate(keys, time, *, mode="source"):
    if time <= keys[0]["time"]:
        return keys[0]["value"]
    if time >= keys[-1]["time"]:
        return keys[-1]["value"]
    left, right = next((left, right) for left, right in zip(keys, keys[1:]) if left["time"] <= time < right["time"])
    span = f32(right["time"] - left["time"])
    alpha = f32(f32(time - left["time"]) / span)
    first, last = left["value"], right["value"]
    leave, arrive = left["leave_tangent"], right["arrive_tangent"]
    if left["interp_mode"] == 0 and mode not in ("hermite", "missing_mode"):
        return lerp(first, last, alpha)
    if mode == "hermite":
        a2, a3 = f32(alpha * alpha), f32(f32(alpha * alpha) * alpha)
        h00 = f32(f32(f32(2 * a3) - f32(3 * a2)) + 1)
        h10 = f32(f32(a3 - f32(2 * a2)) + alpha)
        h01 = f32(f32(-2 * a3) + f32(3 * a2))
        h11 = f32(a3 - a2)
        values = [f32(h00 * first), f32(f32(h10 * span) * leave),
                  f32(h01 * last), f32(f32(h11 * span) * arrive)]
        return f32(f32(f32(values[0] + values[1]) + values[2]) + values[3])
    first_offset = f32(leave * span)
    second_offset = f32(arrive * span)
    if mode in ("divide", "legacy"):
        p1 = f32(first + f32(first_offset / 3))
        p2 = f32(last - f32(second_offset / 3))
    else:
        third = f32(1 / 3)
        p1 = f32(first + f32(first_offset * third))
        p2 = f32(last - f32(second_offset * third))
    if mode in ("polynomial", "legacy"):
        inverse = f32(1 - alpha)
        p0_term = f32(f32(f32(inverse * inverse) * inverse) * first)
        p1_term = f32(f32(f32(f32(3 * inverse) * inverse) * alpha) * p1)
        p2_term = f32(f32(f32(f32(3 * inverse) * alpha) * alpha) * p2)
        p3_term = f32(f32(f32(alpha * alpha) * alpha) * last)
        return f32(f32(f32(p0_term + p1_term) + p2_term) + p3_term)
    p01, p12, p23 = lerp(first, p1, alpha), lerp(p1, p2, alpha), lerp(p2, last, alpha)
    return lerp(lerp(p01, p12, alpha), lerp(p12, p23, alpha), alpha)


def check_curve(row, keys, variants, control_keys=None):
    native = row["custom_curve_time_samples"]
    assert all(evaluate(keys, item["time"]) == item["value"] for item in native), row["name"]
    controls = keys if control_keys is None else control_keys
    return {
        variant: sum(evaluate(controls, item["time"], mode=variant) != item["value"] for item in native)
        for variant in variants
    }


def main():
    curves = read("curves.native.json")["blend_profile_probe"]["samples"]
    stance = curves[0]
    keys = stance["custom_curve_keys"]
    assert len(keys) == 3
    for index, key in enumerate(keys):
        key["interp_mode"] = 2 if index < 2 else 0
    graph_base = ROOT / "assets/ALS/Character/AnimationInstances/AB_Als_Grounded/AnimGraph"
    machine = read("grounded.native.json")["state_machine_players"]["machines"][0]
    assert machine["name"] == "Grounded States"
    edges = {edge["transition_guid"] for state in machine["states"] for edge in state["exits"]
             if edge["blend_mode"] == 14 and edge["duration"] == 0.5}
    machine_text = (graph_base / "SM11_Grounded_States.fsm.ron").read_text()
    for transition in ("T9", "T19"):
        line = next(line for line in machine_text.splitlines() if f"/{transition}/Transition.transition.animgraph.ron" in line)
        assert "timed: Some(0.5)" in line and re.search(r'id: "([^"]+)"', line)[1] in edges
        graph = (graph_base / f"SM11_Grounded_States/{transition}/Transition.transition.animgraph.ron").read_text()
        assert "alpha_blend_mode: Custom" in graph
        fields = ("t", "v", "arrive_tangent", "leave_tangent")
        for field, name in zip(fields, ("time", "value", "arrive_tangent", "leave_tangent"), strict=True):
            actual = [f32(float(value)) for value in re.findall(rf"\b{field}: ([^,]+),", graph)]
            assert actual == [key[name] for key in keys], (transition, field)

    deltas = read("curves.request.json")["samples"][0]["deltas"]
    finish = []
    first_legacy_finish = None
    first_source_finish = None
    for delta, frame in zip(deltas, stance["frames"], strict=True):
        factor = min(frame["factor"], 1.0)
        assert evaluate(keys, factor) == frame["alpha"]
        assert evaluate(keys, frame["blend_alpha"]) == frame["blend_value"]
        assert frame["complete"] == (frame["blend_value"] == keys[-1]["value"])
        if first_legacy_finish is None and evaluate(keys, factor, mode="legacy") == keys[-1]["value"]:
            first_legacy_finish = frame["elapsed"]
        if first_source_finish is None and frame["complete"]:
            first_source_finish = frame["elapsed"]
        finish.append([delta, frame["alpha"], frame["complete"]])
    assert first_legacy_finish < first_source_finish < 0.5

    differences = {"stance": check_curve(stance, keys, ("legacy", "polynomial", "divide"))}
    assert differences["stance"]["polynomial"] > 0 and differences["stance"]["divide"] > 0
    stride = []
    target_stride = json.loads((ROOT / "assets/ALS/Data/AnimationInstance/Grounded/als_standing_curves.json").read_bytes())
    for row in curves[1:3]:
        stride_keys = row["custom_curve_keys"]
        target = target_stride[row["name"]]
        assert target["weight_modes"] == [0] * len(stride_keys)
        assert [f32(key["time"] / 100) for key in stride_keys] == [f32(value) for value in target["times"]]
        assert [key["value"] for key in stride_keys] == [f32(value) for value in target["values"]]
        for key, mode in zip(stride_keys, target["interps"], strict=True):
            key["interp_mode"] = {"Cubic": 2, "Linear": 0}[mode]
        differences[row["name"]] = check_curve(row, stride_keys, ("hermite",))
        stride.append({"name": row["name"], "keys": stride_keys,
                       "samples": [[sample["time"], sample["value"]] for sample in row["custom_curve_time_samples"]]})

    dynamics = []
    supplemental = {row["name"].removeprefix("movement-"): row for row in curves[3:]}
    target_dynamics = json.loads((ROOT / "assets/ALS/Data/Character/Movement/grounded_dynamics.runtime.json").read_bytes())
    channels = ("acceleration", "braking_deceleration", "ground_friction")
    for row in read("dynamics.native.json")["blend_profile_probe"]["samples"]:
        profile, axis = row["name"].rsplit("-", 1)
        axis = int(axis)
        native_keys = row["custom_curve_keys"]
        assert all(key["weight_mode"] == 0 for key in native_keys)
        if row["name"] in supplemental:
            control = supplemental[row["name"]]
            assert control["custom_curve_keys"] == native_keys
            values = {sample["time"]: sample["value"] for sample in row["custom_curve_time_samples"]}
            assert all(values[sample["time"]] == sample["value"] for sample in control["custom_curve_time_samples"])
        scale = 100.0 if axis < 2 else 1.0
        target_keys = [{**key, "interp_mode": {"Linear": 0, "Cubic": 2}[key["interpolation"]],
                        "value": f32(key["value"] * scale),
                        "arrive_tangent": f32(key["arrive_tangent"] * scale),
                        "leave_tangent": f32(key["leave_tangent"] * scale)}
                       for key in target_dynamics[profile][channels[axis]]["keys"]]
        assert [key["interp_mode"] for key in native_keys] == [key["interp_mode"] for key in target_keys]
        assert all(evaluate(target_keys, sample["time"]) == sample["value"] for sample in row["custom_curve_time_samples"])
        differences[row["name"]] = check_curve(row, native_keys, ("hermite", "missing_mode"), target_keys)
        dynamics.append({"profile": profile, "axis": axis,
                         "samples": [[sample["time"], f32(sample["value"] / scale)] for sample in row["custom_curve_time_samples"]]})
    assert any(differences[f"normal-{axis}"]["missing_mode"] > 0 for axis in range(3))
    print(json.dumps({"stance_keys": keys, "finish": finish, "first_completion": [first_legacy_finish, first_source_finish],
                      "stride": stride, "dynamics": dynamics, "ablations": differences}, indent=2))


if __name__ == "__main__":
    main()
