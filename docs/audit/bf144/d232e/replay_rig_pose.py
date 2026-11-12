import argparse
import json
import re
import sys
from collections import Counter
from functools import lru_cache
from pathlib import Path


HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "d232d"))
from replay_source_additive import FMA, ROOT, components, early_projection, f32, inverse, product, read_capture, source_rotations
from replay_pose_chain import input_transform, request_transform


def requests():
    head = read_capture(HERE.parent / "d232d/mesh.native.json.gz")["mesh_additive_probe"]["samples"]
    common = json.loads((HERE.parent / "d165c/pelvis-history.request.json").read_text())
    transfer = {key: common[key] for key in ("node_guid", "mesh_asset", "bone_names")}
    transfer["samples"] = [
        {"name": row["name"], "pose": {
            "bone_transforms": [request_transform(bone["local_transform"]) for bone in row["pose"]["bones"]],
            "curves": row["pose"]["curves"],
        }}
        for row in head
    ]
    for case in common["cases"]:
        case["frames"] = case["frames"][:12]
    history = {**common, "unit_boundary": {"function": "FRigUnit_SetRotation::Execute", "items": ["Item"]}}
    for name, request in (("transfer", transfer), ("history-before", common), ("history", history)):
        (HERE / f"{name}.request.json").write_text(json.dumps(request, separators=(",", ":")) + "\n")


def read(name):
    path = HERE / f"{name}.native.json"
    return json.loads(path.read_bytes()) if path.exists() else read_capture(path.with_suffix(".json.gz"))


def cross(a, b):
    return tuple(FMA(-a[k], b[j], a[j] * b[k]) for j, k in ((1, 2), (2, 0), (0, 1)))


def rotate(q, v):
    c = cross(q[:3], v)
    twice = tuple(x + x for x in c)
    intermediate = [FMA(q[3], b, a) for a, b in zip(v, twice)]
    return tuple(a + b for a, b in zip(intermediate, cross(q[:3], twice)))


def hierarchy(rows, normalize, length_scale):
    cache = {row["name"]: row for row in rows if row["type"] == "Bone"}

    def read_cached(row, space):
        q, t, s = components(row[f"current_{space}_cache"])
        return q, tuple(value * length_scale for value in t), s

    @lru_cache(maxsize=None)
    def global_transform(name):
        row = cache[name]
        if not row["current_global_dirty"]:
            return read_cached(row, "global")
        q, t, s = read_cached(row, "local")
        if row["parent"] == "None":
            return normalize(q), t, s
        pq, pt, ps = global_transform(row["parent"])
        translation = rotate(pq, tuple(a * b for a, b in zip(ps, t)))
        return normalize(product(pq, q)), tuple(a + b for a, b in zip(translation, pt)), tuple(a * b for a, b in zip(s, ps))

    def local_transform(name):
        row = cache[name]
        if not row["current_local_dirty"]:
            return read_cached(row, "local")
        q, t, s = read_cached(row, "global")
        pq, pt, ps = global_transform(row["parent"])
        inverse_rotation = inverse(pq)
        position = rotate(inverse_rotation, tuple(a - b for a, b in zip(t, pt)))
        reciprocal = tuple(1.0 / scale for scale in ps)
        return normalize(product(inverse_rotation, q)), tuple(a * b for a, b in zip(position, reciprocal)), tuple(a * b for a, b in zip(s, reciprocal))

    return local_transform, global_transform


def stable_work(value):
    if isinstance(value, list):
        return [stable_work(item) for item in value]
    if isinstance(value, dict):
        return {key: stable_work(item) for key, item in value.items()
                if not (value.get("name") == "ContainerVersion" and key in ("exported_text", "numeric_value"))}
    if isinstance(value, str):
        return re.sub(r"ContainerVersion=-?\d+", "ContainerVersion", value)
    return value


def replay(work):
    normalize, _ = source_rotations(work)
    counts = Counter()
    witness = None
    request = json.loads((HERE / "transfer.request.json").read_bytes())
    for case, row in zip(request["samples"], read("transfer")["rig_pose_transfer_probe"]["samples"], strict=True):
        assert case["name"] == row["name"] and row["adapter_enabled"] and row["adapter_ready"]
        assert row["transfer_flags"] == {"input_pose": True, "input_curves": True, "global_space": False, "reset_pose": False}
        assert row["before"] == row["after"]
        transferred = {bone["name"]: components(bone["local"]) for bone in row["transferred"]}
        assert transferred.keys() == set(request["bone_names"])
        for name, source in zip(request["bone_names"], case["pose"]["bone_transforms"], strict=True):
            assert input_transform(source) == transferred[name]
            counts["transfer_bones"] += 1
            counts["rounded_transfer_bones"] += early_projection(input_transform(source)) != input_transform(source)

    baseline = read("history-before")["rig_node_history_probe"]
    observed = read("history")["rig_node_history_probe"]
    for original, case in zip(baseline["cases"], observed["cases"], strict=True):
        assert case["name"] == original["name"] and case["unit_boundary_binding_restored"]
        for before, frame in zip(original["frames"], case["frames"], strict=True):
            assert {key: value for key, value in frame.items() if key not in ("unit_boundaries", "work_after")} == {
                key: value for key, value in before.items() if key != "work_after"}
            assert stable_work(frame["work_after"]) == stable_work(before["work_after"])
            boundaries = frame["unit_boundaries"]
            if not boundaries:
                continue
            assert [row["instruction"] for row in boundaries] == [100, 130]
            raw = boundaries[-1]["hierarchy_cache_before_item_reads"]
            late_local, late_global = hierarchy(raw, normalize, 1.0)
            early_local, _ = hierarchy(raw, normalize, 0.01)
            final_global = {row["name"]: components(row["current_global_transform"]) for row in frame["hierarchy_after"]["bones"]}
            for row in frame["after"]["bones"]:
                name = row["name"]
                native = components(row["local_transform"])
                value = late_local(name)
                assert value == native, (case["name"], frame["name"], name, value, native)
                assert late_global(name) == final_global[name], (case["name"], frame["name"], name, late_global(name), final_global[name])
                counts["local_and_global_bones"] += 1
                early = early_local(name)[1]
                late = tuple(component * 0.01 for component in value[1])
                counts["early_meters_double"] += early != late
                if tuple(map(f32, early)) != tuple(map(f32, late)):
                    counts["early_meters_f32"] += 1
                    witness = witness or {"case": case["name"], "frame": frame["name"], "bone": name, "early": early, "late": late}
            counts["history_frames"] += 1
    assert witness and counts["rounded_transfer_bones"] > 0
    return {"result": "PASS_SOURCE_RIG_LOCAL_TRANSFER", **counts, "witness": witness}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--requests", action="store_true")
    parser.add_argument("--work", type=Path, default=ROOT / "target/d232e")
    args = parser.parse_args()
    if args.requests:
        requests()
    else:
        print(json.dumps(replay(args.work), indent=2))
