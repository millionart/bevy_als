import argparse
import json
import sys
from collections import Counter
from pathlib import Path


HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "d232b"))
from replay_pose_chain import FMA, ROOT, components, early_projection, f32, input_transform, project, read_capture, request_transform, source_rotations, walk_run_poses
from replay_animation_relative import PACK, clip_data


NODES = {
    "pelvis": ("f420d0ed-44b6-fc69-c441-4bb723fc7f14", True, [[("pelvis", 0), ("thigh_l", -1), ("thigh_r", -1)]]),
    "spine": ("c118bb3b-4e38-ad76-0cbc-cf865ad26c84", True, [[("spine_01", 3)]]),
    "neck": ("7043759a-4796-027b-b8b3-f9bd1551bdd6", True, [[("neck_01", 0)]]),
    "left_arm": ("ba336ddc-4d17-60ed-8ed2-6eb20b10a92d", True, [[("clavicle_l", 1)]]),
    "right_arm": ("6973a284-4b89-a761-ea84-1da1a08e71f8", True, [[("clavicle_r", 1)]]),
    "arms": ("f686986a-4182-0bc5-6261-a6b8554660cf", False, [[("clavicle_l", 1)], [("clavicle_r", 1)]]),
    "fingers": ("d650094f-41a3-2706-67fb-b5a5994b2139", False, [
        [(f"{digit}_01_{side}", 0) for digit in ("index", "middle", "ring", "pinky", "thumb")]
        + [(f"VB hand_{side}_to_{target}", 0) for target in ("hand_r", "ik_hand_gun")]
        for side in ("l", "r")]),
    "stop": ("65c98345-4e47-077e-f1a4-45ab3dea2923", True,
             [[(f"thigh_{side}", 0), (f"ik_foot_{side}", 0), (f"VB foot_{side}", 0)] for side in ("l", "r")]),
}


def write_requests():
    source = walk_run_poses()
    names = [bone["name"] for bone in source[0]]
    poses = [{"bone_transforms": [request_transform(bone["local_transform"]) for bone in pose], "curves": {}}
             for pose in source]
    for name, (guid, _, filters) in NODES.items():
        weights = [None, [0.0], [0.27], [0.63], [0.999995], [1.0]] if len(filters) == 1 else [
            None, [0.0, 0.0], [0.27, 0.63], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]]
        samples = []
        for index, weight in enumerate(weights):
            sample = {"name": f"{name}:{index}", "delta_seconds": 1.0 / 30.0,
                      "base": poses[index % 3], "layers": [poses[(index + offset + 1) % 3] for offset in range(len(filters))]}
            if weight is not None:
                sample["blend_weights"] = weight
            samples.append(sample)
        request = {"node_guid": guid, "bone_names": names, "samples": samples}
        (HERE / f"{name}.request.json").write_text(json.dumps(request, separators=(",", ":")) + "\n")


def capture(name):
    path = HERE / f"{name}.native.json"
    return json.loads(path.read_bytes()) if path.exists() else read_capture(path.with_suffix(".json.gz"))


def product(a, b):
    x, y, z, w = a
    i, j, k, l = b
    return (w * i + x * l + y * k - z * j,
            w * j - x * k + y * l + z * i,
            w * k + x * j - y * i + z * l,
            w * l - x * i - y * j - z * k)


def effective_mask(paths, filters, layer_weights):
    values = [(0, 0.0)] * len(paths)
    for layer, branches in enumerate(filters):
        for name, depth in branches:
            root = next((path for path in paths if path[-1] == name), None)
            if root is None:
                continue
            increase = f32(1.0 / depth) if depth else 1.0
            for index, path in enumerate(paths):
                if path[:len(root)] == root:
                    increment = f32(increase * (len(path) - len(root) + 1))
                    values[index] = (layer, min(max(f32(values[index][1] + increment), 0.0), 1.0))
    output = []
    for layer, weight in values:
        effective = f32(weight * layer_weights[layer])
        output.append((layer, effective) if effective > f32(1e-5) else (0, 0.0))
    return output


def vector_lerp(a, b, weight):
    return tuple(FMA(end, weight, start * (1.0 - weight)) for start, end in zip(a, b))


def layered(base, layers, parents, mask, mesh_space, normalize, nlerp, full_norms=0):
    result, mesh = [], []
    identity = (0.0, 0.0, 0.0, 1.0)
    for index, (layer, weight) in enumerate(mask):
        parent = parents[index]
        source, target = base[index], layers[layer][index]
        if weight <= f32(1e-5):
            value = source
        elif weight >= f32(1.0 - f32(1e-5)):
            value = target
        else:
            value = (nlerp(source[0], target[0], weight), vector_lerp(source[1], target[1], weight),
                     vector_lerp(source[2], target[2], weight))
        if mesh_space:
            source_parent, target_parent, output_parent = mesh[parent] if parent >= 0 else (identity,) * 3
            source_mesh, target_mesh = product(source_parent, source[0]), product(target_parent, target[0])
            if weight <= f32(1e-5):
                component = source_mesh
            elif weight >= f32(1.0 - f32(1e-5)):
                component = target_mesh
                for _ in range(full_norms):
                    component = normalize(component)
            else:
                sign = -1.0 if sum(a * b for a, b in zip(source_mesh, target_mesh)) < 0.0 else 1.0
                component = normalize(tuple(b * weight + a * (sign * (1.0 - weight))
                                            for a, b in zip(source_mesh, target_mesh)))
            if parent >= 0:
                inverse = tuple(-value for value in output_parent[:3]) + output_parent[3:]
                value = (normalize(product(inverse, component)), value[1], value[2])
            mesh.append((source_mesh, target_mesh, component))
        result.append(value)
    return result


def replay(work, names):
    metadata, _ = clip_data(ROOT / PACK)
    paths = metadata["bone_paths"]
    reference = [components(bone["skeleton_pose"]) for bone in metadata["bones"]]
    parents = [bone["parent_index"] for bone in metadata["bones"]]
    normalize, nlerp = source_rotations(work)
    if "spine" in names:
        before, after = capture("layering-graph"), capture("spine")
        for key in ("asset_path", "asset_properties", "blueprint_cdo_properties"):
            assert before[key] == after[key]
        assert [graph for graph in before["graphs"] if graph["name"] == "AnimGraph"] == [
            graph for graph in after["graphs"] if graph["name"] == "AnimGraph"]
    counts = Counter()
    witness = {}
    for name in names:
        guid, mesh, filters = NODES[name]
        request = json.loads((HERE / f"{name}.request.json").read_bytes())
        observed = capture(name)
        data = observed["layered_bone_blend_probe"]
        assert guid == request["node_guid"] == data["node_guid"]
        assert request["bone_names"] == [path[-1] for path in paths]
        prop = next(prop for prop in observed["blueprint_cdo_properties"] if prop["name"] == data["node_property"])
        fields = {field["name"]: field["exported_text"] for field in prop["fields"]}
        assert (fields["bMeshSpaceRotationBlend"] == "True") == mesh
        assert fields["bMeshSpaceScaleBlend"] == fields["bRootSpaceRotationBlend"] == "False"
        for case, row in zip(request["samples"], data["samples"], strict=True):
            assert case["name"] == row["name"]
            weights = list(map(f32, case.get("blend_weights", [1.0] * len(filters))))
            assert row["update_weights"] == [1.0, *weights]
            mask = effective_mask(paths, filters, weights)
            assert mask == [(bone["source"], bone["weight"]) for bone in row["bone_weights"]], case["name"]
            base = [input_transform(bone) for bone in case["base"]["bone_transforms"]]
            layers = [[input_transform(bone) for bone in pose["bone_transforms"]] if weight > f32(1e-5) else reference
                      for weight, pose in zip(weights, case["layers"])]
            if all(weight <= f32(1e-5) for weight in weights):
                result = [base] * 4
            else:
                result = [
                    layered(base, layers, parents, mask, mesh, normalize, nlerp),
                    layered(list(map(early_projection, base)), [list(map(early_projection, layer)) for layer in layers],
                            parents, mask, mesh, normalize, nlerp),
                    layered(base, layers, parents, mask, mesh, normalize, nlerp, full_norms=2),
                    layered(base, layers, parents, [(0, weight) for _, weight in mask], mesh, normalize, nlerp),
                ]
            for index, (value, early, endpoint, wrong_layer) in enumerate(zip(*result)):
                expected = components(row["bone_transforms"][index])
                assert value == expected, (case["name"], paths[index][-1], value, expected)
                counts["source_bones"] += 1
                counts["mesh_bones" if mesh else "local_bones"] += 1
                counts["partial_weights"] += f32(1e-5) < mask[index][1] < f32(1.0 - f32(1e-5))
                counts["full_endpoint_double_differences"] += endpoint != value
                counts["wrong_layer_double_differences"] += wrong_layer != value
                if project(early) != project(value):
                    key = "mesh" if mesh else "local"
                    counts[f"{key}_early_projection"] += 1
                    witness.setdefault(key, {"case": case["name"], "bone": paths[index][-1],
                                            "source": project(value), "early": project(early)})
                if project(endpoint) != project(value):
                    counts["full_endpoint_projected_differences"] += 1
                    witness.setdefault("full_endpoint", {"case": case["name"], "bone": paths[index][-1],
                                                         "source": project(value), "extra_normalization": project(endpoint)})
    assert counts["source_bones"] > 0 and counts["partial_weights"] > 0
    if len(names) == len(NODES):
        assert all(counts[key] for key in ("local_early_projection", "mesh_early_projection",
                                           "full_endpoint_projected_differences", "wrong_layer_double_differences"))
    return {"result": "PASS_SOURCE_LAYERED_POSE", **counts, "witness": witness}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--requests", action="store_true")
    parser.add_argument("--work", type=Path, default=ROOT / "target/d232c")
    parser.add_argument("--nodes", nargs="+", choices=NODES, default=list(NODES))
    args = parser.parse_args()
    if args.requests:
        write_requests()
    else:
        print(json.dumps(replay(args.work, args.nodes), indent=2))
