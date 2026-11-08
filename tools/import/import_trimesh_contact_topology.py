import argparse
import gzip
import json
import math
from pathlib import Path

from coordinate_basis import source_point_to_runtime


def vector(value):
    result = [value[axis] for axis in "xyz"]
    if any(isinstance(component, bool) or not isinstance(component, (int, float)) or not math.isfinite(component)
           for component in result):
        raise ValueError("Source mesh vector is invalid")
    return [float(component) for component in result]


def convert(report):
    result = {}
    for sample in report["physics_contact_probe"]["samples"]:
        request = sample["input"]
        geometry = sample["other_geometry"]
        transform = geometry["shape_to_body"]
        if (geometry["kind"] != "trimesh" or geometry["shape_index"] != request["mesh_shape_index"]
                or request["mesh_scale"] != [1, 1, 1] or vector(geometry["trimesh_core_scale"]) != [1, 1, 1]
                or vector(transform["translation"]) != [0, 0, 0] or vector(transform["scale"]) != [1, 1, 1]
                or [transform["rotation"][axis] for axis in "xyzw"] != [0, 0, 0, 1]):
            raise ValueError("Source mesh topology must use an unscaled identity shape")
        vertices = [source_point_to_runtime(vector(point)) for point in geometry["trimesh_core_vertices_cm"]]
        triangles = geometry["trimesh_core_triangles"]
        if not vertices or not triangles or any(len(face) != 3 or any(type(index) is not int or index < 0 or index >= len(vertices)
                for index in face) for face in triangles):
            raise ValueError("Source mesh triangles are invalid")
        name = request["mesh_asset_path"].rsplit("/", 1)[-1]
        if name in result:
            raise ValueError("Source mesh topology is duplicated")
        result[name] = {"vertices_m": vertices, "triangles": triangles, "winding": -1}
    return dict(sorted(result.items()))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--bounds", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.output.resolve() in (args.source.resolve(), args.bounds.resolve()):
        raise ValueError("Target output must differ from source")
    raw = args.source.read_bytes()
    if args.source.suffix == ".gz":
        raw = gzip.decompress(raw)
    result = convert(json.loads(raw))
    if set(result) != set(json.loads(args.bounds.read_text())):
        raise ValueError("Source mesh assets differ from target bounds")
    args.output.write_text(json.dumps(result, separators=(",", ":")) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
