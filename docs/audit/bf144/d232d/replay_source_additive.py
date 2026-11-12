import argparse
import json
import sys
from collections import Counter
from pathlib import Path


HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "d232c"))
from replay_layered_pose import FMA, PACK, ROOT, clip_data, components, early_projection, f32, input_transform, product, project, read_capture, source_rotations
from replay_pose_chain import accumulate
from replay_source_blend import source_pose


THRESHOLD = f32(1e-5)
FULL = f32(1.0 - THRESHOLD)
IDENTITY = (0.0, 0.0, 0.0, 1.0)


def read(stem):
    path = HERE / f"{stem}.native.json"
    return json.loads(path.read_bytes()) if path.exists() else read_capture(path.with_suffix(".json.gz"))


def bones(row):
    return [components(bone["local_transform"]) for bone in row["bones"]]


def inverse(q):
    return tuple(-value for value in q[:3]) + q[3:]


def mesh_rotations(values, parents):
    output = []
    for (rotation, translation, scale), parent in zip(values, parents):
        output.append((product(output[parent][0], rotation) if parent >= 0 else rotation, translation, scale))
    return output


def difference(reference, target, parents, normalize):
    if parents is not None:
        reference, target = (mesh_rotations(pose, parents) for pose in (reference, target))
    return [
        (normalize(product(q, inverse(rq))), tuple(a - b for a, b in zip(t, rt)),
         tuple(a * (0.0 if abs(b) <= f32(1e-8) else 1.0 / b) - 1.0 for a, b in zip(s, rs)))
        for (rq, rt, rs), (q, t, s) in zip(reference, target)
    ]


def add(base, delta, weight, parents, normalize, nlerp, passes):
    if weight <= THRESHOLD:
        return base
    if parents is not None:
        base = mesh_rotations(base, parents)
    output = []
    for (q, t, s), (dq, dt, ds) in zip(base, delta):
        if weight >= FULL:
            dq = tuple(component * weight for component in dq)
            t = tuple(FMA(b, weight, a) for a, b in zip(t, dt))
            s = tuple(a * FMA(b, weight, 1.0) for a, b in zip(s, ds))
        else:
            dq = nlerp(IDENTITY, dq, weight)
            t = tuple(a + b * weight for a, b in zip(t, dt))
            s = tuple(a * (1.0 + b * weight) for a, b in zip(s, ds))
        output.append((product(dq, q), t, s))
    if parents is not None:
        for index in range(len(output) - 1, -1, -1):
            parent = parents[index]
            if parent >= 0:
                q, t, s = output[index]
                output[index] = (product(inverse(output[parent][0]), q), t, s)
    for _ in range(passes):
        output = [(normalize(q), t, s) for q, t, s in output]
    return output


def replay(work):
    metadata, keys = clip_data(ROOT / PACK)
    parents = [bone["parent_index"] for bone in metadata["bones"]]
    normalize, nlerp = source_rotations(work)
    counts = Counter()
    witness = {}

    def check(stage, actual, expected, early=None):
        for index, (value, oracle) in enumerate(zip(actual, expected, strict=True)):
            assert value == oracle, (stage, metadata["bones"][index]["name"], value, oracle)
            counts[stage] += 1
            if early is not None and project(value) != project(early[index]):
                counts[f"{stage}_early_projection"] += 1
                witness.setdefault(stage, {"bone": metadata["bones"][index]["name"],
                                           "source": project(value), "early": project(early[index])})

    local_request = json.loads((HERE.parent / "d170/additive.request.json").read_text())
    for request, frame in zip(local_request["samples"], read("local")["geometry_query_probe"]["samples"], strict=True):
        controls = request["additive_accumulation"]
        layer, = controls["layers"]
        base = [(request["rotation_xyzw"], controls["translation_cm"], controls["scale"])]
        delta = [(layer["rotation_xyzw"], layer["translation_cm"], layer["scale_delta"])]
        weight = f32(layer["weight"])
        value = add(base, delta, weight, None, normalize, nlerp, 1)
        check("local_api", value, [components(frame["additive_accumulation"])],
              add(list(map(early_projection, base)), list(map(early_projection, delta)),
                  weight, None, normalize, nlerp, 1))

    mesh_request = json.loads((HERE.parent / "d231/head.request.json").read_text())
    for request, frame in zip(mesh_request["samples"], read("mesh")["mesh_additive_probe"]["samples"], strict=True):
        assert request["name"] == frame["name"] and not frame["root_space_additive"]
        base = [input_transform(row) for row in request["base"]["bone_transforms"]]
        delta = [input_transform(row) for row in request["additive"]["bone_transforms"]]
        weight = frame["alpha"]
        for passes, stage in ((1, "api_pose"), (2, "pose")):
            value = add(base, delta, weight, parents, normalize, nlerp, passes)
            early = add(list(map(early_projection, base)), list(map(early_projection, delta)),
                        weight, parents, normalize, nlerp, passes)
            check(f"mesh_{stage}", value, bones(frame[stage]), early)

    for stem, base, mesh in (("lean", "lean", None), ("look", "stand", parents)):
        reference = bones(read(f"{base}-reference")["sequence_pose_probe"]["frames"][0])
        absolute = read(f"{stem}-absolute")["sequence_pose_probe"]["frames"]
        delta = read(f"{stem}-delta")["sequence_pose_probe"]["frames"]
        for a, b in zip(absolute, delta, strict=True):
            assert a["time"] == b["time"]
            target = bones(a)
            check(f"{stem}_difference", difference(reference, target, mesh, normalize), bones(b),
                  difference(list(map(early_projection, reference)), list(map(early_projection, target)),
                             mesh, normalize))

    for stem, request_stem in (("turn", "standing-turn"), ("transition", "transition")):
        native = read(stem)["slot_probe"]
        assert native == read_capture(HERE.parent / f"d169/{request_stem}.native.json.gz")["slot_probe"]
        request = json.loads((HERE.parent / f"d169/{request_stem}.request.json").read_text())
        sequence = next(row for row in metadata["sequences"] if row["asset"] == request["dynamic_montage"]["sequence"])
        for step, frame in zip(request["steps"], native["frames"], strict=True):
            weights = [f32(weight) for weight in frame["evaluation_weights"] if f32(weight) > THRESHOLD]
            total = 0.0
            for weight in weights:
                total = f32(total + weight)
            assert total == frame["total_weight"]
            if total > f32(1.0 + THRESHOLD):
                weights = [f32(weight / total) for weight in weights]
            base = [input_transform(row) for row in step["bone_transforms"]]
            if stem == "turn":
                sample = bones(native["frames"][0])
                assert frame["montage_position"] == native["frames"][0]["montage_position"]
                poses = [sample] * len(weights)
                regrouped = [accumulate([bone] * len(weights), [f32(weight / frame["slot_weight"]) for weight in weights],
                                        normalize, 0) for bone in sample]
                outer_weights = [frame["slot_weight"]]
                if frame["source_weight"] > THRESHOLD:
                    poses.append(base)
                    weights.append(frame["source_weight"])
                    outer_weights.append(frame["source_weight"])
                final = [accumulate(list(transforms), weights, normalize, int(len(weights) > 1))
                         for transforms in zip(*poses)]
                early = [accumulate([early_projection(bone) for bone in transforms], weights, normalize, int(len(weights) > 1))
                         for transforms in zip(*poses)]
                regrouped = [accumulate([bone, source][:len(outer_weights)], outer_weights, normalize, int(len(poses) > 1))
                             for bone, source in zip(regrouped, base)]
            else:
                sample = source_pose(sequence, metadata, keys, frame["montage_position"], normalize)
                final = base
                early = list(map(early_projection, base))
                early_delta = list(map(early_projection, sample))
                combined = [(IDENTITY, (0.0, 0.0, 0.0), (1.0, 1.0, 1.0))] * len(parents)
                for weight in weights:
                    final = add(final, sample, weight, parents, normalize, nlerp, 1)
                    early = add(early, early_delta, weight, parents, normalize, nlerp, 1)
                    combined = add(combined, sample, weight, None, normalize, nlerp, 1)
                combined = [(q, t, tuple(value - 1.0 for value in s)) for q, t, s in combined]
                regrouped = add(base, combined, 1.0, parents, normalize, nlerp, 1)
            check(f"{stem}_slot", final, bones(frame), early)
            counts[f"{stem}_slot_regrouped_double"] += sum(a != b for a, b in zip(final, regrouped))
            counts[f"{stem}_slot_regrouped_f32"] += sum(project(a) != project(b) for a, b in zip(final, regrouped))

    assert counts["mesh_pose_early_projection"] and counts["look_difference_early_projection"]
    assert counts["turn_slot_regrouped_f32"] and counts["transition_slot_regrouped_f32"]
    return {"result": "PASS_SOURCE_ADDITIVE_POSE", **counts, "witness": witness}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--work", type=Path, default=ROOT / "target/d232d")
    print(json.dumps(replay(parser.parse_args().work), indent=2))
