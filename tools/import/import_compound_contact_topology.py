import argparse
import gzip
import json
import math
from pathlib import Path

from coordinate_basis import source_direction_to_runtime, source_point_to_runtime


def vector(value):
    result = [float(value[axis]) for axis in "xyz"]
    if not all(math.isfinite(component) for component in result):
        raise ValueError("Source topology vector is not finite")
    return result


def compile_topology(native, collision):
    groups = {}
    for sample in native["physics_contact_probe"]["samples"]:
        request = sample["input"]
        geometry = sample["other_geometry"]
        name = request["mesh_asset_path"].rsplit("/", 1)[1]
        index = request["mesh_shape_index"]
        transform = geometry["shape_to_body"]
        if (geometry["kind"] != "convex" or geometry["shape_index"] != index
                or vector(geometry["convex_core_scale"]) != [1, 1, 1]
                or vector(transform["translation"]) != [0, 0, 0]
                or vector(transform["scale"]) != [1, 1, 1]
                or [transform["rotation"][axis] for axis in "xyzw"] != [0, 0, 0, 1]):
            raise ValueError("Source topology must be an unscaled convex shape")
        vertices = [source_point_to_runtime(vector(point)) for point in geometry["convex_core_vertices_cm"]]
        if len(vertices) < 4:
            raise ValueError("Source convex vertices are missing")
        faces = []
        for plane in geometry["convex_core_planes"]:
            indices = plane["vertex_indices"]
            if len(indices) < 3 or any(not isinstance(value, int) or value < 0 or value >= len(vertices) for value in indices):
                raise ValueError("Source convex face indices are invalid")
            faces.append({"normal": source_direction_to_runtime(vector(plane["normal"])),
                          "origin_m": source_point_to_runtime(vector(plane["origin_cm"])),
                          "vertices": indices})
        margin = float(geometry["margin_cm"]) * 0.01
        if not faces or not math.isfinite(margin) or margin < 0:
            raise ValueError("Source convex planes or margin are invalid")
        group = groups.setdefault(name, {})
        if index in group:
            raise ValueError("Source convex shape index is duplicated")
        group[index] = {"vertices_m": vertices, "faces": faces, "margin_m": margin, "winding": -1}
    if set(groups) != set(collision):
        raise ValueError("Source and target convex assets differ")
    result = {}
    for name, shapes in sorted(groups.items()):
        if sorted(shapes) != list(range(len(collision[name]))):
            raise ValueError("Source and target convex shape indices differ")
        result[name] = [shapes[index] for index in range(len(shapes))]
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--collision", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    raw = args.source.read_bytes()
    if args.source.suffix == ".gz":
        raw = gzip.decompress(raw)
    topology = compile_topology(json.loads(raw), json.loads(args.collision.read_text()))
    args.output.write_text(json.dumps(topology, separators=(",", ":")) + "\n")


if __name__ == "__main__":
    main()
