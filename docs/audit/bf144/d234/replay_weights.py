import argparse
import json
import sys
from pathlib import Path


HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "d232b"))
from replay_pose_chain import ROOT, accumulate, components, f32, input_transform, project, read_capture, source_rotations


def write_request():
    original = json.loads((HERE.parent / "d232b/operators.request.json").read_bytes())
    samples = []
    for base in original["samples"]:
        if base["pose_mode"] != "two":
            continue
        for label, alpha in (("sprint59", 0.4052223861217499), ("quarter", 0.25)):
            first, second = base["weighted_inputs"]
            samples.append({**base, "name": f"{base['name']}:{label}", "duration": 1.0, "deltas": [alpha],
                            "weighted_inputs": [{**first, "weight": f32(1.0 - alpha)}, {**second, "weight": alpha}]})
    (HERE / "weights.request.json").write_text(json.dumps({"samples": samples}, separators=(",", ":")) + "\n")


def replay(work):
    normalize, _ = source_rotations(work)
    request = json.loads((HERE / "weights.request.json").read_bytes())
    path = HERE / "weights.native.json"
    native = json.loads(path.read_bytes()) if path.exists() else read_capture(path.with_suffix(".json.gz"))
    exact = differences = projected_differences = 0
    witness = None
    for sample, row in zip(request["samples"], native["blend_profile_probe"]["samples"], strict=True):
        assert sample["name"] == row["name"]
        frame, = row["frames"]
        raw = f32(sample["deltas"][0])
        first = f32(1.0 - raw)
        effective = f32(1.0 - first)
        assert frame["alpha"] == raw and frame["two_way_effective_alpha"] == effective
        assert raw == effective if row["name"].endswith(":quarter") else effective == 0.40522241592407227
        poses = [input_transform(item["transform"]) for item in sample["weighted_inputs"]]
        correct = accumulate(poses, [first, effective], normalize, 1)
        assert correct == components(frame["weighted_pose"])
        old = accumulate(poses, [first, raw], normalize, 1)
        exact += 1
        differences += correct != old
        if project(correct) != project(old):
            projected_differences += 1
            witness = witness or {"case": row["name"], "update_alpha": raw, "evaluation_alpha": effective,
                                  "correct": project(correct), "old": project(old)}
    assert exact == 158 and differences > 0 and projected_differences > 0
    return {"result": "PASS_TWO_WAY_COMPLEMENT", "bones": exact,
            "old_double_differences": differences, "old_f32_differences": projected_differences,
            "witness": witness}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--request", action="store_true")
    parser.add_argument("--work", type=Path, default=ROOT / "target/d234")
    args = parser.parse_args()
    if args.request:
        write_request()
    else:
        print(json.dumps(replay(args.work), indent=2))
