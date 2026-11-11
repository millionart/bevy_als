import argparse
import hashlib
import json
import math
from pathlib import Path

from import_playground_ray_culling import report_bytes


def point(value):
    result = [value[axis] for axis in "xzy"]
    if not all(math.isfinite(component) for component in result):
        raise ValueError("Cooked mesh vector is not finite")
    return result


def bounds(value):
    minimum, maximum = point(value["min"]), point(value["max"])
    if any(low > high for low, high in zip(minimum, maximum)):
        raise ValueError("Cooked mesh bounds are inverted")
    return [minimum, maximum]


def traversal(nodes, face_count):
    stack = [0]
    seen = set()
    faces = []
    while stack:
        index = stack.pop()
        if not 0 <= index < len(nodes) or index in seen:
            raise ValueError("Cooked mesh BVH node reference is invalid")
        seen.add(index)
        for child in nodes[index]:
            if child is None:
                continue
            index = child["index"]
            count = child["face_count"]
            if count:
                if not 0 <= index < face_count or index + count > face_count:
                    raise ValueError("Cooked mesh BVH face range is invalid")
                faces.extend(range(index, index + count))
            else:
                stack.append(index)
    if len(seen) != len(nodes) or sorted(faces) != list(range(face_count)):
        raise ValueError("Cooked mesh BVH does not cover every face once")
    return faces


def compile_sample(sample, rules):
    request = sample["input"]["mesh_raycast"]
    name = request["asset_path"].rsplit("/", 1)[-1].split(".", 1)[0]
    shape, = [shape for shape in sample["mesh_raycast"]["shapes"] if shape["complex"] and shape["triangle_mesh"]]
    if not request["capture_ray_geometry"] or not shape["query_enabled"] or shape["culls_back_face"] != rules[name]:
        raise ValueError("Cooked mesh query metadata differs")
    transform = shape["ray_geometry_shape_to_body"]
    rotation = transform["rotation"]
    if (point(shape["core_scale"]) != [1, 1, 1] or point(transform["scale"]) != [1, 1, 1]
            or point(transform["translation"]) != [0, 0, 0] or any(rotation[axis] for axis in "xyz")
            or abs(rotation["w"]) != 1.0):
        raise ValueError("Cooked mesh shape requires an unsupported local transform")
    vertices = [point(vertex) for vertex in shape["trimesh_core_vertices_cm"]]
    triangles = shape["trimesh_core_triangles"]
    if not triangles or any(len(face) != 3 or any(not 0 <= index < len(vertices) for index in face) for face in triangles):
        raise ValueError("Cooked mesh triangle indices are invalid")
    face_bounds = [bounds(value) for value in shape["trimesh_core_bvh_face_bounds_cm"]]
    if len(face_bounds) != len(triangles):
        raise ValueError("Cooked mesh face bounds are incomplete")
    nodes = []
    for children in shape["trimesh_core_bvh_nodes"]:
        if len(children) != 2:
            raise ValueError("Cooked mesh BVH must have binary nodes")
        nodes.append([None if child is None else {
            "index": child["index"],
            "face_count": child["face_count"],
            "bounds_cm": bounds(child),
        } for child in children])
    order = traversal(nodes, len(triangles))
    if order != shape["trimesh_core_bvh_face_order"]:
        raise ValueError("Cooked mesh BVH traversal order differs")
    return name, {"vertices_cm": vertices, "triangles": triangles, "face_bounds_cm": face_bounds, "nodes": nodes}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--rules", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--audit", type=Path, required=True)
    args = parser.parse_args()
    raw = report_bytes(args.source)
    rules = json.loads(args.rules.read_text())
    meshes = {}
    for sample in json.loads(raw)["geometry_query_probe"]["samples"]:
        name, data = compile_sample(sample, rules)
        if name in meshes:
            raise ValueError(f"Duplicate cooked mesh {name}")
        meshes[name] = data
    if meshes.keys() != rules.keys():
        raise ValueError("Cooked mesh coverage differs from source mesh rules")
    args.output.write_text(json.dumps(meshes, sort_keys=True, separators=(",", ":"), allow_nan=False) + "\n")
    audit = {
        "source_sha256": hashlib.sha256(raw).hexdigest(),
        "meshes": [{
            "mesh": name,
            "vertices": len(data["vertices_cm"]),
            "triangles": len(data["triangles"]),
            "bvh_nodes": len(data["nodes"]),
        } for name, data in sorted(meshes.items())],
    }
    args.audit.write_text(json.dumps(audit, indent=2) + "\n")


if __name__ == "__main__":
    main()
