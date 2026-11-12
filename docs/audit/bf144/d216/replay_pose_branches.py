import gzip
import importlib.util
import json
from pathlib import Path


AUDIT = Path(__file__).resolve().parent


def read(name):
    path = AUDIT / name
    raw = path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix(".json.gz").read_bytes())
    return json.loads(raw)


def curve_sum(poses, weighted, f32):
    result = {}
    for index, weight in weighted:
        for name, value in poses[index]["curves"].items():
            result[name] = f32(result.get(name, 0.0) + f32(f32(value) * weight))
    return result


def curve_lerp(poses, first, second, alpha, f32):
    a, b = poses[first]["curves"], poses[second]["curves"]
    if alpha <= f32(1e-5):
        return {name: f32(value) for name, value in a.items()}
    if abs(f32(alpha - 1.0)) <= f32(1e-5):
        return {name: f32(value) for name, value in b.items()}
    result = {}
    for name in sorted(a.keys() | b.keys()):
        start = f32(a.get(name, 0.0))
        end = f32(b.get(name, 0.0))
        result[name] = f32(start + f32(f32(end - start) * alpha))
    return result


def transform(poses, weighted, field):
    return [sum(poses[index]["bone_transforms"][0][field][axis] * weight for index, weight in weighted)
            for axis in range(3)]


def main():
    spec = importlib.util.spec_from_file_location("source_blend_list", AUDIT.parent / "d214/replay_blend_list_clock.py")
    blend = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(blend)
    f32 = blend.f32
    cases = []
    branches = {"single": 0, "two": 0, "weighted": 0}
    ablations = {"scale_full_pose": 0, "use_raw_second_weight": 0, "sum_curves": 0}
    for name in ("two", "multi"):
        request = read(f"{name}.request.json")
        source = read(f"{name}.native.json")["blend_list_probe"]["samples"]
        for requested, observed in zip(request["samples"], source, strict=True):
            assert requested["name"] == observed["name"]
            poses = requested["poses"]
            rows = blend.simulate(requested["steps"], list(map(f32, request["blend_times"])), request["mode"])
            cases_for_fixture = []
            for index, (row, frame) in enumerate(zip(rows, observed["frames"], strict=True)):
                assert row["weights"] == frame["weights"]
                weights = frame["weights"]
                relevant = [(i, weight) for i, weight in enumerate(weights) if weight > f32(1e-5)]
                evaluation = relevant
                branch = "weighted"
                curves = curve_sum(poses, relevant, f32)
                if len(relevant) == 1 and relevant[0][1] >= f32(1.0 - f32(1e-5)):
                    branch = "single"
                    evaluation = [(relevant[0][0], 1.0)]
                    curves = curve_sum(poses, evaluation, f32)
                elif len(relevant) == 2 and abs(f32(f32(relevant[0][1] + relevant[1][1]) - 1.0)) <= f32(1e-8):
                    branch = "two"
                    first, second = relevant
                    alpha = f32(1.0 - first[1])
                    evaluation = [first, (second[0], alpha)]
                    curves = curve_lerp(poses, first[0], second[0], alpha, f32)
                branches[branch] += 1
                root = frame["pose"]["bones"][0]
                assert root["name"] == "root"
                root = root["local_transform"]
                positions = [f32(root["translation"][axis] / 100.0) for axis in "xyz"]
                scales = [f32(root["scale"][axis]) for axis in "xyz"]
                assert [f32(value / 100.0) for value in transform(poses, evaluation, "translation_cm")] == positions, (name, index)
                assert list(map(f32, transform(poses, evaluation, "scale"))) == scales, (name, index)
                assert curves == frame["pose"]["curves"], (name, index, curves, frame["pose"]["curves"])
                if branch == "single":
                    ablations["scale_full_pose"] += list(map(f32, transform(poses, relevant, "scale"))) != scales
                if branch == "two":
                    ablations["use_raw_second_weight"] += [f32(value / 100.0) for value in transform(poses, relevant, "translation_cm")] != positions
                    ablations["sum_curves"] += curve_sum(poses, evaluation, f32) != curves
                cases_for_fixture.append({"weights": weights, "branch": branch, "position": positions,
                                          "scale": scales, "curves": curves})
            cases.append({"name": observed["name"], "frames": cases_for_fixture})
    assert all(branches.values()) and all(ablations.values())
    print(json.dumps({"cases": cases, "branches": branches, "ablations": ablations}, indent=2))


if __name__ == "__main__":
    main()
