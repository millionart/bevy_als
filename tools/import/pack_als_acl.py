"""Pack native ACL streams from a UE sequence-compression export for ALS."""

import argparse
import base64
import json
from pathlib import Path
import re
import struct


def glb_document(path):
    payload = path.read_bytes()
    return json.loads(payload[20:20 + struct.unpack_from("<I", payload, 12)[0]])


def bone_records(assets, bones):
    model = glb_document(assets / "ALS/Character/SKM_Als.glb")
    joints = {joint for skin in model["skins"] for joint in skin["joints"]}
    paths = {}
    pending = [(index, ()) for index in model["scenes"][0]["nodes"]]
    while pending:
        index, parent = pending.pop()
        node = model["nodes"][index]
        name = node.get("name", f"GltfNode{index}")
        path = (*parent, name)
        if index in joints:
            if name in paths:
                raise ValueError(f"Ambiguous ALS joint: {name}")
            paths[name] = path
        pending.extend((child, path) for child in node.get("children", ()))
    if paths.keys() != {bone["name"] for bone in bones}:
        raise ValueError("Installed ALS bone names differ from the source mesh")

    def transform(value):
        return {
            field: [value[field][axis] for axis in axes]
            for field, axes in (("translation", "xyz"), ("rotation", "xyzw"), ("scale", "xyz"))
        }

    ordered = [paths[bone["name"]] for bone in bones]
    records = []
    for path, bone in zip(ordered, bones):
        parent = bone["parent_index"]
        if parent >= 0 and path[:-1] != ordered[parent]:
            raise ValueError(f"Installed hierarchy differs for {bone['name']}")
        reference = transform(bone["skeleton_pose"])
        mesh_pose = transform(bone["mesh_pose"])
        row = {"path": path, "reference": reference}
        mode = bone["retarget_mode"]
        if mode == 3:
            row["relative_target"] = mesh_pose
        elif mode == 4:
            if any(abs(a - b) > 0.0001 for a, b in zip(
                reference["translation"], mesh_pose["translation"]
            )):
                raise ValueError(f"OrientAndScale needs authored transforms for {bone['name']}")
        elif mode != 0:
            raise ValueError(f"Unsupported bundled bone retarget mode {mode} for {bone['name']}")
        records.append(row)
    return records


def additive_reference_paths(probes):
    references = {}
    for probe in probes:
        properties = {item["name"]: item["exported_text"] for item in probe["asset_properties"]}
        if (properties["AdditiveAnimType"], properties["RefPoseType"], properties["RefFrameIndex"]) != (
            "AAT_LocalSpaceBase", "ABPT_AnimFrame", "0"
        ):
            raise ValueError(f"Expected a local additive clip with a frame-zero reference: {probe['asset_path']}")
        match = re.fullmatch(r"/Script/Engine\.AnimSequence'/ALS/ALS/Animations/(.+)\.[^.]+'",
                             properties["RefPoseSeq"])
        if match is None:
            raise ValueError(f"Reference clip does not map to ALS animations: {properties['RefPoseSeq']}")
        references[probe["asset_path"]] = f"ALS/Animations/{match[1]}.anim.ron"
    return references


def pack(assets, source, additive_references):
    if source["format"] != "ue_sequence_compression_v1":
        raise ValueError("Expected a UE sequence compression export")
    bones = source["bones"]
    result = {"bones": bone_records(assets, bones), "sequences": []}
    expected_relative = [
        {"bone": index, "authored_pose": bone["skeleton_pose"]}
        for index, bone in enumerate(bones)
        if bone["retarget_mode"] == 3
    ]
    payload = bytearray()
    for row in source["sequences"]:
        package, animation = row["asset"].rsplit(".", 1)
        relative = package.removeprefix("/ALS/ALS/Animations/")
        if relative == package or row["pose_mode"] != "compressed":
            raise ValueError(f"Expected a compressed ALS sequence: {row['asset']}")
        additive = additive_references.get(row["asset"])
        if row["additive_type"] not in (0, 1) or row["forced_root_lock"] is not None:
            raise ValueError(f"Unsupported ALS pose type or forced root lock: {package}")
        if (row["additive_type"] == 1) != (additive is not None):
            raise ValueError(f"Expected a native frame-zero base binding for local additive {package}")
        if row["animation_relative"] != expected_relative:
            raise ValueError(f"{package} requires per-sequence authored reference poses")

        clip = assets / f"ALS/Animations/{relative}.anim.ron"
        descriptor = clip.read_text()
        path = re.search(r'\bpath:\s*"([^"]+)"', descriptor)[1]
        name = re.search(r'\banimation_name:\s*"([^"]+)"', descriptor)[1]
        gltf_root = "ALS" if additive is not None else "ALSRuntime"
        if path != f"{gltf_root}/Animations/{relative}.glb" or name != f"{animation}_0":
            raise ValueError(f"Installed clip does not map to the prepared ALS runtime: {clip}")
        sequence = {"path": path, "animation": name, "step": row["step"]}
        if additive is not None:
            if not (assets / additive).is_file():
                raise ValueError(f"Installed additive reference is missing: {additive}")
            sequence["additive_reference"] = additive
        data = row["compression"]
        for stream, codec in (("bone", "AnimBoneCompressionCodec_ACL_0"),
                              ("curve", "AnimCurveCompressionCodec_ACL_0")):
            if not data[f"{stream}_codec"].endswith(f":{codec}"):
                raise ValueError(f"{package} uses a different {stream} codec")
            contents = base64.b64decode(data[f"{stream}_stream_base64"], validate=True)
            if contents:
                sequence[f"{stream}_range"] = [len(payload), len(payload) + len(contents)]
                payload.extend(contents)
        sequence["bone_tracks"] = data["track_to_skeleton"]
        sequence["curve_tracks"] = [entry["name"] for entry in data["indexed_curve_names"]]
        result["sequences"].append(sequence)
    metadata = json.dumps(result, ensure_ascii=False, separators=(",", ":")).encode()
    return struct.pack("<I", len(metadata)) + metadata + payload


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--assets", type=Path, default=Path("assets"))
    parser.add_argument("--additive-base-probe", action="append", type=Path, default=[])
    args = parser.parse_args()
    references = additive_reference_paths(json.loads(probe.read_text()) for probe in args.additive_base_probe)
    result = pack(args.assets, json.loads(args.capture.read_text()), references)
    with args.output.open("xb") as output:
        output.write(result)
    print(f"Packed {len(result):,} bytes of ACL tracks and bindings")


if __name__ == "__main__":
    main()
