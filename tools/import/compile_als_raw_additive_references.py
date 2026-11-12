import argparse
import json
import math
import re
import struct
from pathlib import Path

from capture_als_sequence_compression import read_capture, sequence_inventory
from compile_als_prepared_acl import ROOT, runtime_bone_paths


def compile_references(assets, captures):
    sequences = sequence_inventory(assets)
    with (assets / "ALSRuntime/Animations/als_prepared_acl.bin").open("rb") as stream:
        length, = struct.unpack("<I", stream.read(4))
        prepared = json.loads(stream.read(length))
    output = {}
    for path in captures:
        native = read_capture(path)
        clip, _ = sequences[native["asset_path"]]
        descriptor = (assets / clip).read_text()
        source = re.search(r'\bpath:\s*"([^"]+)".*?\banimation_name:\s*"([^"]+)"', descriptor, re.S)
        skeleton = re.search(r'\bskeleton:\s*"([^"]+\.skn\.ron)"', descriptor)
        if source is None or skeleton is None:
            raise ValueError(f"Missing installed animation and skeleton: {clip}")
        mesh_path = skeleton[1].removesuffix(".skn.ron")
        mesh = f"/ALS/{mesh_path}.{Path(mesh_path).name}"
        probe = native["sequence_pose_probe"]
        if probe["sampling_mode"] != "raw_absolute_pose" or probe["required_bones_mesh"] != mesh:
            raise ValueError(f"Raw absolute mesh pose required: {clip}")
        for frame in probe["frames"]:
            time = frame["time"]
            bones = frame["bones"]
            if frame["pose_evaluation_mode"] != "raw_model" or not math.isfinite(time) or time < 0:
                raise ValueError(f"Invalid raw reference frame: {clip}")
            paths = runtime_bone_paths(assets, skeleton[1], bones)
            if [list(path) for path in paths] != prepared["bone_paths"]:
                raise ValueError(f"Raw reference bone order differs from the prepared mesh: {clip}")
            transforms = [row["local_transform"] for row in bones]
            for transform in transforms:
                for field, axes in (("translation", "xyz"), ("rotation", "xyzw"), ("scale", "xyz")):
                    if set(transform[field]) != set(axes) or not all(math.isfinite(value) for value in transform[field].values()):
                        raise ValueError(f"Invalid raw reference transform: {clip}")
            key = (*source.groups(), time)
            if key in output:
                raise ValueError(f"Duplicate raw reference sample: {clip}")
            output[key] = {"gltf": source[1], "animation": source[2], "time": time, "bones": transforms}
    return [output[key] for key in sorted(output)]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--assets", type=Path, default=ROOT / "assets")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("captures", type=Path, nargs="+")
    args = parser.parse_args()
    rows = compile_references(args.assets, args.captures)
    args.output.write_text(json.dumps(rows, separators=(",", ":"), allow_nan=False) + "\n")
    print(json.dumps({"references": len(rows), "bones": sum(len(row["bones"]) for row in rows)}))


if __name__ == "__main__":
    main()
