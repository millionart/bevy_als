import argparse
import gzip
import json
from pathlib import Path

from coordinate_basis import source_point_to_runtime
from import_trimesh_contact_topology import vector


def bounds(value):
    minimum = source_point_to_runtime(vector(value["min"]))
    maximum = source_point_to_runtime(vector(value["max"]))
    if any(first > second for first, second in zip(minimum, maximum)):
        raise ValueError("Source mesh BVH bounds are invalid")
    return minimum, maximum


def nodes_for(geometry, order):
    nodes = geometry.get("trimesh_core_bvh_nodes")
    if not isinstance(nodes, list) or not nodes:
        raise ValueError("Source mesh BVH nodes are missing")
    result = []
    for node in nodes:
        if not isinstance(node, list) or len(node) != 2:
            raise ValueError("Source mesh BVH node is invalid")
        children = []
        for child in node:
            if child is None:
                children.append(None)
                continue
            index, count = child["index"], child["face_count"]
            if type(index) is not int or type(count) is not int or index < 0 or count < 0:
                raise ValueError("Source mesh BVH child is invalid")
            if (count == 0 and index >= len(nodes)) or (count > 0 and index + count > len(order)):
                raise ValueError("Source mesh BVH child is out of range")
            minimum, maximum = bounds(child)
            children.append({"index": index, "face_count": count, "min_m": minimum, "max_m": maximum})
        result.append(children)
    stack, visited, observed = [0], set(), []
    while stack:
        index = stack.pop()
        if index in visited:
            raise ValueError("Source mesh BVH contains a repeated node")
        visited.add(index)
        for child in result[index]:
            if child is None:
                continue
            if child["face_count"]:
                observed.extend(range(child["index"], child["index"] + child["face_count"]))
            else:
                stack.append(child["index"])
    if observed != order:
        raise ValueError("Source mesh BVH traversal differs from native order")
    return result


def convert(reports, topology):
    result = {}
    for report in reports:
        for sample in report["physics_contact_probe"]["samples"]:
            request = sample["input"]
            geometry = sample["other_geometry"]
            name = request["mesh_asset_path"].rsplit("/", 1)[-1]
            if geometry["kind"] != "trimesh" or geometry["shape_index"] != request["mesh_shape_index"] or name not in topology:
                raise ValueError("Source mesh identity differs from target topology")
            expected = topology[name]
            vertices = [source_point_to_runtime(vector(point)) for point in geometry["trimesh_core_vertices_cm"]]
            if vertices != expected["vertices_m"] or geometry["trimesh_core_triangles"] != expected["triangles"]:
                raise ValueError("Source mesh geometry differs from target topology")
            order = geometry.get("trimesh_core_bvh_face_order")
            if not isinstance(order, list) or any(type(index) is not int for index in order) or sorted(order) != list(range(len(expected["triangles"]))):
                raise ValueError("Source mesh BVH order is missing or invalid")
            face_bounds = geometry.get("trimesh_core_bvh_face_bounds_cm")
            if not isinstance(face_bounds, list) or len(face_bounds) != len(order):
                raise ValueError("Source mesh BVH face bounds are missing or invalid")
            converted = []
            for bound in face_bounds:
                minimum, maximum = bounds(bound)
                converted.append([minimum, maximum])
            value = {"face_order": order, "face_bounds_m": converted, "nodes": nodes_for(geometry, order)}
            if name in result and result[name] != value:
                raise ValueError("Source mesh BVH data differs across samples")
            result[name] = value
    if set(result) != set(topology):
        raise ValueError("Source mesh BVH orders do not cover target topology")
    return dict(sorted(result.items()))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, nargs="+", required=True)
    parser.add_argument("--topology", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.output.resolve() in {path.resolve() for path in args.source + [args.topology]}:
        raise ValueError("Target output must differ from source")
    reports = []
    for path in args.source:
        raw = path.read_bytes()
        if path.suffix == ".gz":
            raw = gzip.decompress(raw)
        reports.append(json.loads(raw))
    result = convert(reports, json.loads(args.topology.read_text()))
    args.output.write_text(json.dumps(result, separators=(",", ":")) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
