import argparse
import base64
import gzip
import json
import re
from collections import Counter, defaultdict
from pathlib import Path


def sequence_inventory(asset_root):
    manifest = json.loads((asset_root / "ALS/Animations/als_notify_manifest.json").read_bytes())
    sequences = {}
    clips = set()
    for source in manifest["clips"]:
        if source["source_kind"] != "AnimSequence":
            continue
        asset = source["source_asset"]
        segments = source["runtime_segments"]
        if len(segments) != 1 or asset in sequences:
            raise ValueError(f"Ambiguous Sequence identity: {asset}")
        clip = segments[0]["graph_clip"]
        if clip in clips:
            raise ValueError(f"Multiple Sequence sources for {clip}")
        clips.add(clip)
        sequences[asset] = (clip, source["sequence_length"])
    installed = {path.relative_to(asset_root).as_posix() for path in (asset_root / "ALS").rglob("*.anim.ron")}
    if clips != installed:
        raise ValueError(f"Sequence inventory differs from installed clips: {sorted(clips ^ installed)}")
    return dict(sorted(sequences.items()))


def mesh_inventory(asset_root):
    meshes = defaultdict(dict)
    for asset, (clip, duration) in sequence_inventory(asset_root).items():
        descriptor = (asset_root / clip).read_text()
        skeleton = re.search(r'\bskeleton:\s*"([^"]+\.skn\.ron)"', descriptor)
        if skeleton is None:
            raise ValueError(f"Missing mesh skeleton for {clip}")
        path = skeleton[1].removesuffix(".skn.ron")
        mesh = f"/ALS/{path}.{Path(path).name}"
        meshes[mesh][asset] = (clip, duration)
    return dict(meshes)


def read_capture(path):
    content = gzip.decompress(path.read_bytes()) if path.suffix == ".gz" else path.read_bytes()
    return json.loads(content)


def validate_capture(capture, mesh, sequences):
    if capture["format"] != "ue_sequence_compression_v1" or not capture["engine_version"] or not capture["platform"]:
        raise ValueError("Missing prepared compression platform identity")
    if capture["mesh"] != mesh or not capture["skeleton"]:
        raise ValueError("Capture differs from the installed mesh inventory")
    rows = capture["sequences"]
    if [row["asset"] for row in rows] != list(sequences):
        raise ValueError("Native sequences differ from the ordered installed inventory")
    bones = capture["bones"]
    names = {bone["name"] for bone in bones}
    if not bones or len(names) != len(bones) or any(
        not -1 <= bone["parent_index"] < index or (bone["parent_index"] == -1) != (index == 0)
        for index, bone in enumerate(bones)
    ):
        raise ValueError("Invalid native mesh bone hierarchy")
    modes = Counter()
    codecs = Counter()
    additives = Counter()
    durations_from_platform = []
    bone_bytes = curve_bytes = 0
    for row in rows:
        asset = row["asset"]
        manifest_duration = sequences[asset][1]
        if (row["duration"] <= 0 or (manifest_duration and row["duration"] != manifest_duration)
                or type(row["step"]) is not bool):
            raise ValueError(f"Sequence duration or interpolation is invalid: {asset}")
        if not manifest_duration:
            durations_from_platform.append({"asset": asset, "duration": row["duration"]})
        mode = row["pose_mode"]
        modes[mode] += 1
        additives[row["additive_type"]] += 1
        if mode == "raw_model" and "compression" not in row:
            continue
        if mode != "compressed":
            raise ValueError(f"Invalid Sequence pose mode: {asset}")
        data = row["compression"]
        mapping = data["track_to_skeleton"]
        if len(mapping) != len(set(mapping)) or any(not 0 <= index < len(bones) for index in mapping):
            raise ValueError(f"Invalid track-to-skeleton mapping: {asset}")
        names = data["indexed_curve_names"]
        if len({item["name"] for item in names}) != len(names) or sorted(
            item["sorted_index"] for item in names
        ) != list(range(len(names))):
            raise ValueError(f"Invalid platform curve index: {asset}")
        bone = base64.b64decode(data["bone_stream_base64"], validate=True)
        curve = base64.b64decode(data["curve_stream_base64"], validate=True)
        if (len(curve) != data["curve_bytes"] or bool(curve) != bool(names)
                or bool(bone) != bool(mapping) or data["key_count"] < 1
                or min(data["sampling_rate_numerator"], data["sampling_rate_denominator"]) < 1):
            raise ValueError(f"Invalid platform compression streams or clock: {asset}")
        bone_bytes += len(bone)
        curve_bytes += len(curve)
        codecs[(data["bone_codec"], data["curve_codec"])] += 1
    return {
        "platform": capture["platform"],
        "engine_version": capture["engine_version"],
        "mesh": capture["mesh"],
        "skeleton": capture["skeleton"],
        "bones": len(bones),
        "installed_sequences": len(rows),
        "pose_modes": dict(modes),
        "additive_types": dict(additives),
        "durations_from_platform": durations_from_platform,
        "codec_pairs": [{"bone": bone, "curve": curve, "sequences": count}
                        for (bone, curve), count in sorted(codecs.items())],
        "bone_bytes": bone_bytes,
        "curve_bytes": curve_bytes,
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--asset-root", type=Path, default=Path(__file__).resolve().parents[2] / "assets")
    parser.add_argument("--mesh", required=True)
    parser.add_argument("--native", type=Path)
    args = parser.parse_args()
    sequences = mesh_inventory(args.asset_root)[args.mesh]
    result = validate_capture(read_capture(args.native), args.mesh, sequences) if args.native else {"assets": list(sequences)}
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
