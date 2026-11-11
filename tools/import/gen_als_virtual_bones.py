import argparse
import json
from pathlib import Path

from audit_als_animation_events import manifest_targets


def generate(mesh, sequences, targets):
    source = mesh["skeletal_mesh"]
    bindings = source["virtual_bones"]
    bones = source["bones"]
    virtual = {bone["name"] for bone in bones if bone["virtual"]}
    available = {bone["name"] for bone in bones if not bone["virtual"]}
    for binding in bindings:
        name = binding["virtual_bone"]
        bone, = [bone for bone in bones if bone["name"] == name]
        parent = bone["parent_index"]
        if (name not in virtual or name in available or parent < 0
            or bones[parent]["name"] != binding["source_bone"]
            or binding["source_bone"] not in available or binding["target_bone"] not in available):
            raise ValueError("source virtual bone order or parent is invalid")
        available.add(name)
    if len(bindings) != len(virtual) or not bindings:
        raise ValueError("source virtual bone bindings are incomplete")
    paths = []
    for sequence in sequences:
        probe = sequence["sequence_pose_probe"]
        if (probe["required_bones_mesh"] != mesh["asset_path"] or not probe["frames"]
            or any(frame["pose_evaluation_mode"] != "raw_model" for frame in probe["frames"])):
            raise ValueError("sequence probe does not use the source raw mesh pose")
        asset = targets[sequence["asset_path"]].removeprefix("assets/")
        if asset in paths or not Path("assets", asset).is_file():
            raise ValueError("runtime raw sequence path is invalid")
        paths.append(asset)
    if not paths:
        raise ValueError("no source raw sequence has a virtual bone binding")
    return {"raw_gltf_paths":sorted(paths),"virtual_bones":bindings}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--mesh", required=True, type=Path)
    parser.add_argument("--sequence", required=True, action="append", type=Path)
    parser.add_argument("--manifest", default=Path("assets/docs/export-manifest.md"), type=Path)
    parser.add_argument("--output", default=Path("assets/ALS/Data/AnimationInstance/als_raw_virtual_bones.json"), type=Path)
    args = parser.parse_args()
    def read(path):
        return json.loads(path.read_text(encoding="utf-8-sig"))
    data = generate(read(args.mesh), [read(path) for path in args.sequence], manifest_targets(args.manifest))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
    print(f"Generated {len(data['virtual_bones'])} bindings for {len(data['raw_gltf_paths'])} raw sequences")


if __name__ == "__main__":
    main()
