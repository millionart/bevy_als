import argparse
import gzip
import json
import math
from pathlib import Path

from coordinate_basis import source_point_to_runtime


def compare(native, target):
    result = []
    for sample in native["physics_contact_probe"]["samples"]:
        request = sample["input"]
        name = request["mesh_asset_path"].rsplit("/", 1)[1]
        index = request["mesh_shape_index"]
        geometry = sample["other_geometry"]
        source = [source_point_to_runtime([point[axis] for axis in "xyz"])
                  for point in geometry["convex_core_vertices_cm"]]
        runtime = target[name][index]
        def maximum_distance(first, second):
            return max(min(math.dist(point, candidate) for candidate in second) for point in first)
        result.append({"asset": name, "shape_index": index, "source_vertices": len(source),
                       "runtime_vertices": len(runtime), "source_to_runtime_m": maximum_distance(source, runtime),
                       "runtime_to_source_m": maximum_distance(runtime, source)})
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--target", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    raw = args.source.read_bytes()
    if args.source.suffix == ".gz":
        raw = gzip.decompress(raw)
    result = compare(json.loads(raw), json.loads(args.target.read_text()))
    args.output.write_text(json.dumps(result, indent=2) + "\n")


if __name__ == "__main__":
    main()
