import argparse
import gzip
import hashlib
import json
import struct
import sys
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "animgraph_authoring"))
from ue_animation_asset_resolver import asset_property, object_property, source_object_path


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def report_bytes(path):
    raw = path.read_bytes()
    if path.suffix == ".gz":
        raw = gzip.decompress(raw)
        sha = path.with_suffix("").with_suffix(".sha256").read_text().split()[0]
        if hashlib.sha256(raw).hexdigest() != sha:
            raise ValueError("Uncompressed source report does not match its identity")
    return raw


def accessor(document, blob, index):
    entry = document["accessors"][index]
    view = document["bufferViews"][entry["bufferView"]]
    code = {5123: "H", 5125: "I", 5126: "f"}[entry["componentType"]]
    width = {"SCALAR": 1, "VEC3": 3}[entry["type"]]
    layout = struct.Struct("<" + code * width)
    stride = view.get("byteStride", layout.size)
    offset = view.get("byteOffset", 0) + entry.get("byteOffset", 0)
    return [layout.unpack_from(blob, offset + index * stride) for index in range(entry["count"])]


def oriented_faces(points, indices):
    faces = Counter()
    for a, b, c in indices:
        face = (points[a], points[b], points[c])
        if len(set(face)) == 3:
            faces[min(face, (face[1], face[2], face[0]), (face[2], face[0], face[1]))] += 1
    return faces


def gltf_faces(path):
    document = json.loads(path.read_text())
    node, = document["nodes"]
    if any(key in node for key in ("matrix", "rotation", "scale", "translation", "children")):
        raise ValueError("Static mesh must use its exported local geometry")
    buffer, = document["buffers"]
    binary = (path.parent / buffer["uri"]).read_bytes()
    faces = Counter()
    for primitive in document["meshes"][node["mesh"]]["primitives"]:
        if primitive.get("mode", 4) != 4:
            raise ValueError("Static mesh primitive must be a triangle list")
        points = accessor(document, binary, primitive["attributes"]["POSITION"])
        indices = [value for value, in accessor(document, binary, primitive["indices"])]
        faces.update(oriented_faces(points, zip(indices[::3], indices[1::3], indices[2::3], strict=True)))
    return faces


def compile_mesh(source, mesh_root):
    name = source["asset_path"].rsplit(".", 1)[1]
    setup_path = source_object_path(asset_property(source, "BodySetup"))
    setup, = [obj for obj in source["asset_objects"] if obj["object_path"] == setup_path]
    double_sided = object_property(setup, "bDoubleSidedGeometry")
    if double_sided not in ("True", "False"):
        raise ValueError("BodySetup double-sided flag is invalid")
    collision = source["static_mesh_collision"]
    if not collision["available"]:
        raise ValueError("Source mesh collision geometry is missing")
    points = [tuple(f32(point[axis] * f32(0.01)) for axis in "xzy") for point in collision["vertices_source"]]
    triangles = collision["triangle_indices"]
    if not collision["flip_normals"]:
        triangles = [(a, c, b) for a, b, c in triangles]
    expected = oriented_faces(points, triangles)
    actual = gltf_faces(mesh_root / f"{name}.gltf")
    if not expected or actual != expected:
        raise ValueError(f"Source collision winding differs for {name}")
    return name, double_sided == "False", sum(expected.values())


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, nargs="+", required=True)
    parser.add_argument("--mesh-root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--audit", type=Path, required=True)
    args = parser.parse_args()
    culling = {}
    audit = []
    for path in args.source:
        raw = report_bytes(path)
        name, cull_back_faces, triangles = compile_mesh(json.loads(raw), args.mesh_root)
        if name in culling:
            raise ValueError(f"Duplicate source mesh {name}")
        culling[name] = cull_back_faces
        audit.append({"mesh": name, "cull_back_faces": cull_back_faces, "matching_oriented_triangles": triangles,
                      "native_report": path.name, "native_sha256": hashlib.sha256(raw).hexdigest()})
    args.output.write_text(json.dumps(culling, indent=2, sort_keys=True) + "\n")
    args.audit.write_text(json.dumps(sorted(audit, key=lambda mesh: mesh["mesh"]), indent=2) + "\n")


if __name__ == "__main__":
    main()
