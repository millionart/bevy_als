from bisect import bisect_right
import json
import math
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[4]
EVIDENCE = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT / "tools/import"))
from audit_animation_transform_conversion import accessor_values, load_gltf
from audit_als_animation_events import manifest_targets
from coordinate_basis import quaternion_multiply, source_point_to_runtime, source_rotation_to_runtime, source_direction_to_runtime
from gen_als_virtual_bones import generate
from ue_animation_timing import f32


def read(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def channels(transform):
    return tuple(tuple(transform[group][axis] for axis in ("xyzw" if group == "rotation" else "xyz"))
                 for group in ("translation", "rotation", "scale"))


def add(a, b):
    return tuple(x + y for x, y in zip(a, b))


def subtract(a, b):
    return tuple(x - y for x, y in zip(a, b))


def multiply(a, b):
    return tuple(x * y for x, y in zip(a, b))


def inverse_rotation(q):
    return (-q[0], -q[1], -q[2], q[3])


def rotate(q, point):
    return quaternion_multiply(quaternion_multiply(q, (*point, 0.0)), inverse_rotation(q))[:3]


def compose(parent, local):
    pt, pq, ps = parent
    lt, lq, ls = local
    return add(pt, rotate(pq, multiply(ps, lt))), quaternion_multiply(pq, lq), multiply(ps, ls)


def relative(source, target):
    st, sq, ss = source
    tt, tq, ts = target
    inverse = inverse_rotation(sq)
    reciprocal = tuple(1.0 / value for value in ss)
    return multiply(rotate(inverse, subtract(tt, st)), reciprocal), quaternion_multiply(inverse, tq), multiply(ts, reciprocal)


def components(pose, parents):
    result = {}
    for name, parent in parents.items():
        result[name] = pose[name] if parent is None else compose(result[parent], pose[name])
    return result


def reconstruct(pose, parents, bindings, update_components):
    result = pose.copy()
    current = components(pose, parents)
    for binding in bindings:
        name, source, target = (binding[key] for key in ("virtual_bone", "source_bone", "target_bone"))
        result[name] = relative(current[source], current[target])
        if update_components:
            current[name] = compose(current[source], result[name])
    return result


def project(transform):
    t, q, s = transform
    return source_point_to_runtime(t), source_rotation_to_runtime(q), source_direction_to_runtime(s)


def error(actual, expected, names):
    distances = []
    for name in names:
        a, b = actual[name], expected[name]
        position = max(abs(x - y) for x, y in zip(a[0], b[0]))
        rotation = min(max(abs(x - y) for x, y in zip(a[1], b[1])), max(abs(x + y) for x, y in zip(a[1], b[1])))
        scale = max(abs(x - y) for x, y in zip(a[2], b[2]))
        distances.append((position, rotation, scale))
    return [max(values) for values in zip(*distances)]


def slerp(a, b, factor):
    dot = sum(x * y for x, y in zip(a, b))
    if dot < 0.0:
        dot, b = -dot, tuple(-x for x in b)
    if dot > 0.9995:
        result = tuple(x + factor * (y - x) for x, y in zip(a, b))
        inverse = 1.0 / math.sqrt(sum(value * value for value in result))
        return tuple(value * inverse for value in result)
    angle = math.acos(min(dot, 1.0))
    inverse = 1.0 / math.sin(angle)
    wa = math.sin((1.0 - factor) * angle) * inverse
    wb = math.sin(factor * angle) * inverse
    return tuple(x * wa + y * wb for x, y in zip(a, b))


def parent_names(doc, names):
    nodes = doc["nodes"]
    parents = {child:node.get("name") for node in nodes for child in node.get("children", [])}
    return {node["name"]:parents.get(index) for index,node in enumerate(nodes) if node.get("name") in names}


def tracks(path, names):
    doc, blob = load_gltf(path)
    nodes = doc["nodes"]
    animation, = doc["animations"]
    result = {name:{} for name in names}
    for channel in animation["channels"]:
        sampler = animation["samplers"][channel["sampler"]]
        assert sampler.get("interpolation", "LINEAR") == "LINEAR"
        target = channel["target"]
        name = nodes[target["node"]]["name"]
        result[name][target["path"]] = (
            [time[0] for time in accessor_values(doc, blob, sampler["input"])],
            accessor_values(doc, blob, sampler["output"]),
        )
    return result, parent_names(doc, names)


def sample(tracks, time):
    result = {}
    for name, track in tracks.items():
        values = []
        for kind in ("translation", "rotation", "scale"):
            times, samples = track[kind]
            if time <= times[0]:
                value = samples[0]
            elif time >= times[-1]:
                value = samples[-1]
            else:
                hi = bisect_right(times, time)
                lo = hi - 1
                alpha = f32(f32(time - times[lo]) / f32(times[hi] - times[lo]))
                a, b = samples[lo], samples[hi]
                value = slerp(a, b, alpha) if kind == "rotation" else tuple(x + alpha * (y - x) for x, y in zip(a, b))
            values.append(tuple(f32(v) for v in value))
        result[name] = tuple(values)
    return result


def verify():
    mesh_document = read(EVIDENCE / "mesh.native.json")
    mesh = mesh_document["skeletal_mesh"]
    names = [bone["name"] for bone in mesh["bones"]]
    parents = {bone["name"]:names[bone["parent_index"]] if bone["parent_index"] >= 0 else None for bone in mesh["bones"]}
    rest = {bone["name"]:channels(bone["local_transform"]) for bone in mesh["bones"]}
    bindings = mesh["virtual_bones"]
    virtual = [binding["virtual_bone"] for binding in bindings]
    target = read(ROOT / "assets/ALS/Data/AnimationInstance/als_raw_virtual_bones.json")
    assert target["virtual_bones"] == bindings
    skeleton_parents = parent_names(read(ROOT / "assets/ALS/Animations/Base/A_Als_Idle.gltf"), names)
    for binding in bindings:
        assert skeleton_parents[binding["virtual_bone"]] == binding["source_bone"]
    targets = manifest_targets(ROOT / "assets/docs/export-manifest.md")
    raw_documents = [read(EVIDENCE / (name + ".native.json")) for name in ("front", "back", "sprint")]
    assert generate(mesh_document, raw_documents, targets) == target
    for control_mesh, controls in [
        (mesh_document, [read(EVIDENCE / "sprint-compressed.native.json")]),
        ({**mesh_document, "skeletal_mesh":{**mesh, "virtual_bones":list(reversed(bindings))}}, raw_documents),
    ]:
        try:
            generate(control_mesh, controls, targets)
        except ValueError:
            pass
        else:
            raise AssertionError("invalid virtual bone input was accepted")
    result = {}
    for name in ("front", "back", "sprint", "sprint-compressed"):
        doc = read(EVIDENCE / (name + ".native.json"))
        raw = name != "sprint-compressed"
        clip, = [path for path in target["raw_gltf_paths"] if Path(path).stem == doc["asset_path"].split(".")[-1]]
        if raw:
            target_tracks, target_parents = tracks(ROOT / "assets" / clip, names)
            for binding in bindings:
                assert target_parents[binding["virtual_bone"]] == binding["source_bone"]
        modes = ["source_replay", "source_frozen"]
        if raw:
            modes.extend(("target_baked", "target_replay"))
        maximum = {mode:[0.0,0.0,0.0] for mode in modes}
        positions = []
        frames = doc["sequence_pose_probe"]["frames"]
        for frame in frames:
            assert frame["pose_evaluation_mode"] == ("raw_model" if raw else "compressed")
            native = {bone["name"]:channels(bone["local_transform"]) for bone in frame["bones"]}
            base = {bone:rest[bone] if bone in virtual else native[bone] for bone in native}
            probes = {
                "source_replay":error(reconstruct(base, parents, bindings, True), native, virtual),
                "source_frozen":error(reconstruct(base, parents, bindings, False), native, virtual),
            }
            if raw:
                target_pose = sample(target_tracks, f32(frame["time"]))
                projected = {bone:project(value) for bone,value in native.items()}
                probes["target_baked"] = error(target_pose, projected, virtual)
                probes["target_replay"] = error(reconstruct(target_pose, parents, bindings, True), projected, virtual)
            for mode, errors in probes.items():
                maximum[mode] = [max(a,b) for a,b in zip(maximum[mode], errors)]
            positions.append({"time":frame["time"],"errors":probes})
        if raw:
            assert maximum["source_replay"][0] < 1e-6 and maximum["source_replay"][1] < 1e-8
            assert maximum["source_frozen"][0] > 1.0
            assert maximum["target_baked"][0] > maximum["target_replay"][0]
        else:
            assert maximum["source_replay"][0] > 1.0
        result[name] = {"frames":len(frames), "maximum_errors":maximum, "positions":positions}
    return result


if __name__ == "__main__":
    print(json.dumps(verify(), indent=2))
