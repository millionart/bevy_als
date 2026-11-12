import argparse
import base64
import hashlib
import json
import math
import platform
import re
import struct
import subprocess
import tempfile
import time
from pathlib import Path

from capture_als_sequence_compression import mesh_inventory, read_capture, validate_capture
from coordinate_basis import SOURCE_LENGTH_TO_RUNTIME_SCALE, source_heading_yaw_degrees_to_runtime


ROOT = Path(__file__).resolve().parents[2]
NO_PROGRESS_SECONDS = 30
HEADER = struct.Struct("<IIffB")


def write_json(path, document):
    path.write_text(json.dumps(document, indent=2) + "\n")


def run_tool(command, report, condition, timeout=NO_PROGRESS_SECONDS):
    started = time.monotonic()
    last_progress = json.loads(report.read_bytes())["last_progress"] if report.is_file() else "process_started"
    result = {"current_condition": condition, "last_progress": last_progress, "no_progress_limit_seconds": timeout}
    try:
        status = subprocess.run(command, capture_output=True, timeout=timeout)
    except subprocess.TimeoutExpired:
        result.update(exit_reason="no_progress_limit", process_exit_code=None)
        raise RuntimeError(f"No progress while {condition}") from None
    except OSError:
        result.update(exit_reason="start_failed", process_exit_code=None)
        raise
    else:
        result.update(exit_reason="completed" if status.returncode == 0 else "process_failed",
                      process_exit_code=status.returncode)
        if status.returncode != 0:
            raise RuntimeError(f"{condition}: {status.stderr.decode(errors='replace').strip()}")
        result["last_progress"] = condition
    finally:
        result["no_progress_seconds"] = round(time.monotonic() - started, 6)
        write_json(report, result)


def compile_tool(source, output, sdk, report):
    run_tool([
        "clang++", "-std=c++20", "-O2", "-ffp-contract=off", "-Wall", "-Wextra", "-Werror",
        "-isystem", str(sdk / "includes"), "-isystem", str(sdk / "external/rtm/includes"),
        str(source), "-o", str(output),
    ], report, f"building {source.name}")


def unpack_stream(data, width, expected_tracks, values):
    track_count, frame_count, rate, duration, wrap = HEADER.unpack_from(data)
    if (track_count != expected_tracks or frame_count == 0 or not math.isfinite(rate)
            or rate <= 0 or not math.isfinite(duration) or duration < 0 or wrap not in (0, 1)):
        raise ValueError("Invalid decoded ACL stream clock or track count")
    clock = {"sample_count": frame_count, "sample_rate": rate, "duration": duration, "wrap": bool(wrap)}
    position = HEADER.size
    if len(width) == 3:
        scale, default_scale, stripped = struct.unpack_from("<BBB", data, position)
        position += 3
        if any(flag not in (0, 1) for flag in (scale, default_scale, stripped)):
            raise ValueError("Invalid ACL transform metadata")
        clock.update(has_scale=bool(scale), default_scale=default_scale, stripped_step=bool(stripped))

    tracks = []
    for _ in range(track_count):
        channels = []
        for components in width:
            mode = data[position]
            position += 1
            if mode not in (0, 1, 2) or (len(width) == 1 and mode == 0):
                raise ValueError("Invalid decoded ACL channel mode")
            length = (0, 1, frame_count)[mode] * components * 4
            channel = data[position:position + length]
            if len(channel) != length or not all(math.isfinite(value) for value, in struct.iter_unpack("<f", channel)):
                raise ValueError("Invalid decoded ACL channel values")
            channels.append((len(values) // 4, mode))
            values.extend(channel)
            position += length
        tracks.append(channels)

    probe_count, = struct.unpack_from("<I", data, position)
    probes = data[position + 4:]
    if len(probes) != probe_count * (1 + track_count * sum(width)) * 4:
        raise ValueError("Invalid ACL sample readback")
    return clock, tracks, probes


def runtime_bone_paths(assets, skeleton_asset, bones):
    descriptor = (assets / skeleton_asset).read_text()
    scene = re.search(r'\bGltf\(source: "([^"]+)", label: "Scene(\d+)"\)', descriptor)
    if scene is None:
        raise ValueError(f"Missing installed skeleton scene: {skeleton_asset}")
    document = json.loads((assets / scene[1]).read_bytes())
    joints = {joint for skin in document["skins"] for joint in skin["joints"]}
    paths = {}
    pending = [(index, ()) for index in document["scenes"][int(scene[2])]["nodes"]]
    while pending:
        index, parent = pending.pop()
        node = document["nodes"][index]
        name = node.get("name", f"GltfNode{index}")
        path = (*parent, name)
        if index in joints:
            if name in paths:
                raise ValueError(f"Ambiguous installed bone name: {name}")
            paths[name] = path
        pending.extend((child, path) for child in node.get("children", ()))
    if paths.keys() != {bone["name"] for bone in bones}:
        raise ValueError(f"Installed bone names differ from the source mesh: {skeleton_asset}")
    ordered = [paths[bone["name"]] for bone in bones]
    for path, bone in zip(ordered, bones):
        if bone["parent_index"] >= 0 and path[:-1] != ordered[bone["parent_index"]]:
            raise ValueError(f"Installed bone hierarchy differs for {bone['name']}")
    return ordered


def compile_assets(capture, sequences, assets, sdk, output, work):
    report = work / "watchdog.json"
    work.mkdir(parents=True, exist_ok=True)
    report.unlink(missing_ok=True)
    binary = work / "decode_als_acl"
    compile_tool(Path(__file__).with_name("decode_als_acl.cpp"), binary, sdk, report)

    input_stream = work / "compressed.bin"
    readback = work / "readback.bin"
    values = bytearray()
    records = []
    oracles = {}
    sources = set()
    skeletons = set()
    for row in capture["sequences"]:
        asset = row["asset"]
        clip, _ = sequences[asset]
        descriptor = (assets / clip).read_text()
        path = re.search(r'\bpath:\s*"([^"]+)"', descriptor)[1]
        name = re.search(r'\banimation_name:\s*"([^"]+)"', descriptor)[1]
        skeletons.add(re.search(r'\bskeleton:\s*"([^"]+)"', descriptor)[1])
        if (path, name) in sources:
            raise ValueError(f"Ambiguous installed animation source: {clip}")
        sources.add((path, name))
        data = row["compression"]
        if not data["bone_codec"].endswith(":AnimBoneCompressionCodec_ACL_0") or not data[
            "curve_codec"
        ].endswith(":AnimCurveCompressionCodec_ACL_0"):
            raise ValueError(f"Sequence uses a different codec: {asset}")
        mapping = data["track_to_skeleton"]
        single_root = mapping[0] == 0
        probe = {}
        record = {key: row[key] for key in ("asset", "duration", "additive_type", "step", "forced_root_lock")}
        if record["forced_root_lock"] not in (None, "RefPose", "AnimFirstFrame", "Zero"):
            raise ValueError(f"Unknown source root lock for {asset}")
        record.update(clip=clip, gltf=path, animation=name, single_root=single_root)
        record["frame_rate"] = [data["sampling_rate_numerator"], data["sampling_rate_denominator"]]
        indexed = data["indexed_curve_names"]
        curve_names = [item["name"] for item in indexed]
        for kind, names, width in (("bone", mapping, (4, 3, 3)), ("curve", curve_names, (1,))):
            stream = base64.b64decode(data[f"{kind}_stream_base64"], validate=True)
            if not stream:
                record[kind] = None
                continue
            input_stream.write_bytes(stream)
            run_tool([str(binary.resolve()), str(input_stream), str(readback), str(int(single_root)), str(int(row["step"]))],
                     report, f"decoding {kind} tracks for {asset}")
            clock, tracks, probes = unpack_stream(readback.read_bytes(), width, len(names), values)
            if kind == "bone" and clock["default_scale"] != int(row["additive_type"] == 0):
                raise ValueError(f"ACL additive defaults disagree with source mode: {asset}")
            clock["tracks"] = [(key, *channels) for key, channels in zip(names, tracks)]
            record[kind] = clock
            probe[kind] = base64.b64encode(probes).decode("ascii")
        records.append(record)
        oracles[asset] = probe

    if len(skeletons) != 1:
        raise ValueError("A prepared ACL corpus requires one installed skeleton")
    relative_bones = [index for index, bone in enumerate(capture["bones"]) if bone["retarget_mode"] == 3]
    authored = [{"bone": index, "authored_pose": capture["bones"][index]["skeleton_pose"]}
                for index in relative_bones]
    if any(row["animation_relative"] != authored for row in capture["sequences"]):
        raise ValueError("Sequence authored relative poses differ from the shared skeleton")
    bones = [{key: value for key, value in bone.items() if key != "retarget_mode"} for bone in capture["bones"]]
    metadata = {
        "format": "als_prepared_acl_v1",
        "platform": capture["platform"],
        "sdk_architecture": platform.machine(),
        "source_length_to_runtime_scale": SOURCE_LENGTH_TO_RUNTIME_SCALE,
        "curve_value_scales": {"RotationYawSpeed": source_heading_yaw_degrees_to_runtime(1.0)},
        "bones": bones,
        "bone_paths": runtime_bone_paths(assets, skeletons.pop(), bones),
        "animation_relative_bones": relative_bones,
        "sequences": records,
    }
    header = json.dumps(metadata, separators=(",", ":")).encode()
    compiled = struct.pack("<I", len(header)) + header + values
    write_json(work / "acl-probes.json", oracles)
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=output.parent, delete=False) as stream:
        temporary = Path(stream.name)
        try:
            stream.write(compiled)
            stream.close()
            temporary.chmod(0o644)
            temporary.replace(output)
        finally:
            temporary.unlink(missing_ok=True)
    write_json(work / "compile-report.json", {
        "status": "completed", "sequences": len(records), "key_values": len(values) // 4,
        "compiled_bytes": len(compiled), "sha256": hashlib.sha256(compiled).hexdigest(),
    })


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--ue-root", type=Path, required=True)
    parser.add_argument("--capture", type=Path, default=ROOT / "docs/audit/bf144/d230/character.native.json.gz")
    parser.add_argument("--assets", type=Path, default=ROOT / "assets")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--work", type=Path, required=True)
    args = parser.parse_args()
    capture = read_capture(args.capture)
    sequences = mesh_inventory(args.assets)[capture["mesh"]]
    validate_capture(capture, capture["mesh"], sequences)
    sdk = args.ue_root / "Engine/Plugins/Animation/ACLPlugin/Source/ThirdParty/acl"
    compile_assets(capture, sequences, args.assets, sdk, args.output, args.work)


if __name__ == "__main__":
    main()
