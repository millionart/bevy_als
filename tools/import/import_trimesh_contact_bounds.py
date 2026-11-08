import argparse
import gzip
import json
import math
from pathlib import Path

from coordinate_basis import source_point_to_runtime


def convert(report):
    result = {}
    for sample in report["physics_contact_probe"]["samples"]:
        request = sample["input"]
        geometry = sample["other_geometry"]
        transform = geometry["shape_to_body"]
        vector = lambda value: [float(value[axis]) for axis in "xyz"]
        if (geometry["kind"] != "trimesh" or geometry["shape_index"] != request["mesh_shape_index"]
                or request["mesh_scale"] != [1, 1, 1] or vector(transform["translation"]) != [0, 0, 0]
                or vector(transform["scale"]) != [1, 1, 1]
                or [transform["rotation"][axis] for axis in "xyzw"] != [0, 0, 0, 1]):
            raise ValueError("Source mesh bounds must use an unscaled identity shape")
        minimum = source_point_to_runtime(vector(geometry["bounds_min_cm"]))
        maximum = source_point_to_runtime(vector(geometry["bounds_max_cm"]))
        if any(not math.isfinite(value) for value in minimum + maximum) or any(a > b for a, b in zip(minimum, maximum)):
            raise ValueError("Source mesh bounds are invalid")
        name = request["mesh_asset_path"].rsplit("/", 1)[-1]
        if name in result:
            raise ValueError("Source mesh bounds are duplicated")
        result[name] = [minimum, maximum]
    return dict(sorted(result.items()))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    raw = args.source.read_bytes()
    if args.source.suffix == ".gz":
        raw = gzip.decompress(raw)
    args.output.write_text(json.dumps(convert(json.loads(raw)), separators=(",", ":")) + "\n")


if __name__ == "__main__":
    main()
