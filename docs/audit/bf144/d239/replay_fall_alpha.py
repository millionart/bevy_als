import argparse
import gzip
import json
import re
import struct
import sys
from pathlib import Path


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
sys.path.insert(0, str(HERE.parent / "d232b"))
from replay_pose_chain import accumulate, components, input_transform, project, request_transform, source_rotations


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def inputs():
    return [
        ("first_fall", [-0.3269999623298645]),
        ("zero", [0.0]),
        ("faster_than_range", [-14.0]),
        ("warm_fall", [-0.3269999623298645, -0.6540000438690186, -0.9810000061988831, -1.308000087738037,
                       -1.6349999904632568, -1.9619998931884766, -2.2890000343322754, -0.6, 0.0]),
    ]

COMPLEMENT_INPUTS = [("complement_fall", [-0.16349999606609344])]


def write_request():
    for stem, cases in (("fall", inputs()), ("complement", COMPLEMENT_INPUTS)):
        samples = [{"name": name, "steps": [
            {"active": 0, "delta_seconds": f32(1.0 / 30.0), "two_way_alpha": f32(f32(speed) * 100.0)}
            for speed in speeds]} for name, speeds in cases]
        request = {"mode": "Linear", "blend_times": [0.0], "two_way": {
            "property": "AnimGraphNode_TwoWayBlend_1", "child": 0}, "samples": samples}
        (HERE / f"{stem}.request.json").write_text(json.dumps(request, indent=2) + "\n")
    sequence = {"mesh_asset": "/ALS/ALS/Character/SKM_Als.SKM_Als", "animation_pose": True,
                "prepare_compressed": True, "times": [0.0]}
    (HERE / "sequence.request.json").write_text(json.dumps(sequence, indent=2) + "\n")


def read(stem):
    path = HERE / f"{stem}.native.json"
    return json.loads(path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix(".json.gz").read_bytes()))


def interpolate(previous, target, delta):
    if previous is None:
        return target
    difference = f32(target - previous)
    if f32(difference * difference) < f32(1e-8):
        return target
    return f32(previous + f32(difference * f32(delta * 5.0)))


def replay_alphas(stem, cases):
    native = read(stem)
    prop, = [prop for prop in native["blueprint_cdo_properties"] if prop["name"] == "AnimGraphNode_TwoWayBlend_1"]
    mapping, = [value["exported_text"] for value in prop["fields"] if value["name"] == "AlphaScaleBiasClamp"]
    assert "Max=-1000.000000" in mapping and "bInterpResult=True" in mapping
    frames = []
    for (name, speeds), sample in zip(cases, native["blend_list_probe"]["samples"], strict=True):
        assert name == sample["name"]
        current = early = None
        for speed, frame in zip(speeds, sample["frames"], strict=True):
            physical = f32(speed)
            cm = f32(physical * 100.0)
            dt = f32(1.0 / 30.0)
            current = interpolate(current, f32(cm / -1000.0), dt)
            early = interpolate(early, f32(physical / -10.0), dt)
            value, old = (min(max(x, 0.0), 1.0) for x in (current, early))
            assert frame["two_way"]["alpha"] == value, (name, physical, frame["two_way"]["alpha"], value)
            frames.append({"case": name, "mps": physical, "cmps": cm, "alpha": value, "old_alpha": old})
    return frames


def evaluation_weights(alpha):
    first = f32(1.0 - alpha)
    return first, f32(1.0 - first)


def weights_request(alpha):
    first = read("fast")["sequence_pose_probe"]["frames"][0]["bones"]
    second = read("slow")["sequence_pose_probe"]["frames"][0]["bones"]
    weights = evaluation_weights(alpha)
    samples = []
    for a, b in zip(first, second, strict=True):
        assert a["name"] == b["name"]
        base, overlay = (request_transform(bone["local_transform"]) for bone in (a, b))
        samples.append({"name": a["name"], "profile": "MoveDirectionChange", "mode": "Linear", "duration": 1.0,
                        "curve_a": 0.0, "curve_b": 1.0, "from": base, "to": overlay, "deltas": [alpha],
                        "pose_mode": "two", "weighted_inputs": [
                            {"transform": base, "weight": weights[0]}, {"transform": overlay, "weight": alpha},
                        ]})
    return {"samples": samples}


def replay_pose(frame, work):
    request = weights_request(frame["alpha"])
    assert json.dumps(request, separators=(",", ":")) + "\n" == (HERE / "weights.request.json").read_text()
    native = read("weights")["blend_profile_probe"]["samples"]
    normalize, _ = source_rotations(work)
    correct_weights = evaluation_weights(frame["alpha"])
    old_weights = evaluation_weights(frame["old_alpha"])
    assert correct_weights != old_weights
    differences = projected = 0
    witness = None
    for sample, row in zip(request["samples"], native, strict=True):
        assert sample["name"] == row["name"]
        sample_frame, = row["frames"]
        assert sample_frame["two_way_effective_alpha"] == correct_weights[1]
        poses = [input_transform(item["transform"]) for item in sample["weighted_inputs"]]
        expected = components(sample_frame["weighted_pose"])
        assert accumulate(poses, correct_weights, normalize, 1) == expected
        old = accumulate(poses, old_weights, normalize, 1)
        differences += old != expected
        if project(old) != project(expected):
            projected += 1
            witness = witness or {"bone": row["name"], "source": project(expected), "old": project(old)}
    assert len(native) == 79 and differences > 0 and projected > 0
    return {"bones": len(native), "evaluation_weights": correct_weights, "old_evaluation_weights": old_weights,
            "old_double_mismatches": differences, "old_f32_mismatches": projected, "witness": witness}


def installed_ranges():
    source = {}
    for graph in read("fall")["graphs"]:
        for node in graph["nodes"]:
            if node["class_path"] != "/Script/AnimGraph.AnimGraphNode_TwoWayBlend":
                continue
            blend, = [field for field in node["properties"] if field["name"] == "BlendNode"]
            clamp, = [field for field in blend["fields"] if field["name"] == "AlphaScaleBiasClamp"]
            bounds, = [field for field in clamp["fields"] if field["name"] == "InRange"]
            values = {field["name"]: field["numeric_value"] for field in bounds["fields"]}
            native = [values["Min"], values["Max"]]
            assert source.setdefault(node["node_guid"], native) == native
    states = ROOT / "assets/ALS/Character/AnimationInstances/AB_Als_Locomotion/AnimGraph/SM9_Locomotion_States"
    ranges = {}
    pattern = re.compile(r'id: "([^"]+)".*alpha_key: "in_air_state\.vertical_velocity_world_space".*'
                         r'input_min: ([^,]+), input_max: ([^,]+)')
    for path in sorted(states.glob("*.animgraph.ron")):
        for guid, minimum, maximum in pattern.findall(path.read_text()):
            assert [float(minimum), float(maximum)] == source[guid]
            ranges[guid] = {"state": path.name, "source_range_cmps": source[guid]}
    assert len(ranges) == 7
    return ranges


def replay(work):
    complement, = replay_alphas("complement", COMPLEMENT_INPUTS)
    frames = replay_alphas("fall", inputs()) + [complement]
    first = frames[0]
    assert first["alpha"] == 0.03269999846816063 and first["old_alpha"] == 0.03269999474287033
    assert evaluation_weights(first["alpha"]) == evaluation_weights(first["old_alpha"])
    differences = sum(frame["alpha"] != frame["old_alpha"] for frame in frames)
    assert differences > 0
    return {"result": "PASS_FALL_SOURCE_UNIT_ALPHA", "native_frames": len(frames),
            "old_alpha_mismatches": differences, "frames": frames, "pose": replay_pose(complement, work),
            "installed_ranges": installed_ranges()}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--request", action="store_true")
    parser.add_argument("--weights", action="store_true")
    parser.add_argument("--work", type=Path, default=ROOT / "target/d239")
    args = parser.parse_args()
    if args.request:
        write_request()
    elif args.weights:
        frame, = replay_alphas("complement", COMPLEMENT_INPUTS)
        (HERE / "weights.request.json").write_text(json.dumps(weights_request(frame["alpha"]), separators=(",", ":")) + "\n")
    else:
        print(json.dumps(replay(args.work), indent=2))
