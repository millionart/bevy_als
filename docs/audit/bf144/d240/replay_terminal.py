import argparse
import json
import sys
from pathlib import Path


HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "d232b"))
from replay_pose_chain import components, f32, input_transform, project, read_capture, request_transform


CURVE = "PoseInAir"
DURATION = f32(0.1)


def read(stem, folder=HERE):
    path = folder / f"{stem}.native.json"
    return read_capture(path if path.exists() else path.with_suffix(".json.gz"))


def write_request():
    fast, slow = [
        read(stem, HERE.parent / "d239")["sequence_pose_probe"]["frames"][0]["bones"]
        for stem in ("fast", "slow")
    ]
    names = [bone["name"] for bone in fast]
    assert names == [bone["name"] for bone in slow] and len(names) == 79
    warm = [request_transform(bone["local_transform"]) for bone in fast]
    target = [request_transform(bone["local_transform"]) for bone in slow]
    warm[0] = {"translation_cm": [100.0, 0.0, 0.0], "rotation": [0.0, 0.0, 0.0, 1.0], "scale": [1.0, 1.0, 1.0]}
    target[0] = {"translation_cm": [25.0, 0.0, 0.0], "rotation": [0.0, 0.0, 0.0, 0.99999994], "scale": [1.0, 1.0, 1.0]}
    samples = []
    for name, delta, request in (("exact", 0.05, True), ("crossed", 1.0 / 30.0, True), ("control", 1.0 / 30.0, False)):
        steps = [{"delta_seconds": f32(delta), "bone_transforms": warm, "curves": {CURVE: 0.75}} for _ in range(2)]
        steps += [{"delta_seconds": f32(delta), "bone_transforms": target, "curves": {}} for _ in range(4)]
        if request:
            steps[2]["request_duration"] = DURATION
        samples.append({"name": name, "steps": steps})
    payload = {"node_property": "AnimGraphNode_Inertialization", "bone_names": names, "samples": samples}
    (HERE / "terminal.request.json").write_text(json.dumps(payload, separators=(",", ":")) + "\n")


def replay():
    request = json.loads((HERE / "terminal.request.json").read_bytes())
    native = read("terminal")["inertialization_probe"]
    assert native["enabled"] == 1 and native["node_property"] == request["node_property"]
    assert native["bone_names"] == request["bone_names"]
    cases = []
    terminal = []
    copies = 0
    for sample, observed in zip(request["samples"], native["samples"], strict=True):
        assert sample["name"] == observed["name"]
        elapsed = duration = 0.0
        frames = []
        for index, (step, raw) in enumerate(zip(sample["steps"], observed["frames"], strict=True)):
            delta = f32(step["delta_seconds"])
            requested = [step["request_duration"]] if "request_duration" in step else []
            if requested:
                elapsed = 0.0
                duration, = requested
            elapsed = f32(elapsed + delta)
            expired = duration > 0.0 and elapsed >= duration
            if expired:
                terminal.append({"case": sample["name"], "frame": index, "elapsed": elapsed, "duration": duration})
                duration = 0.0
            bone_inputs = [input_transform(value) for value in step["bone_transforms"]]
            bone_outputs = [components(value) for value in raw["bone_transforms"]]
            assert len(bone_inputs) == len(bone_outputs) == 79
            copy = duration == 0.0
            if copy:
                assert bone_inputs == bone_outputs and step["curves"] == raw["curves"]
                copies += len(bone_inputs)
            else:
                assert bone_inputs[0] != bone_outputs[0] and raw["curves"][CURVE] > 0.0
            root = project(bone_inputs[0])
            if expired:
                w = root[0][3]
                old_w = f32(w * f32(1.0 / abs(w)))
                assert root[0][:3] == (0.0, 0.0, 0.0) and old_w != w and CURVE not in raw["curves"]
                terminal[-1]["old_apply_w"] = old_w
                terminal[-1]["native_w"] = w
                terminal[-1]["old_apply_curve"] = {CURVE: 0.0}
            frames.append({"delta_seconds": delta, "request_durations": requested,
                           "input_m": root[1][0], "input_rotation": root[0],
                           "native_m": project(bone_outputs[0])[1][0],
                           "input_curve": step["curves"].get(CURVE), "native_curve": raw["curves"].get(CURVE),
                           "expected_duration_seconds": duration, "expected_deficit_seconds": 0.0,
                           "copy_input_pose": copy})
        cases.append({"name": sample["name"], "frames": frames})
    assert [row["frame"] for row in terminal] == [3, 4]
    assert terminal[0]["elapsed"] == DURATION < terminal[1]["elapsed"]
    return {"result": "PASS_INERTIALIZATION_TERMINAL_COPY", "curve_name": CURVE, "terminal": terminal,
            "native_copied_bones": copies, "cases": cases}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--request", action="store_true")
    if parser.parse_args().request:
        write_request()
    else:
        print(json.dumps(replay(), indent=2))
