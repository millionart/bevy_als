import argparse
import hashlib
import json
import re
import struct
import tempfile
from pathlib import Path

from audit_animation_transform_conversion import accessor_values, load_gltf
from compile_als_event_tracks import clip_durations, f32


CLIP_SOURCE = re.compile(rb'\bpath:\s*"([^"]+)"')
ANIMATION_NAME = re.compile(rb'\banimation_name:\s*"([^"]+)"')


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def next_float(value):
    bits = struct.unpack("<I", struct.pack("<f", value))[0]
    return struct.unpack("<f", struct.pack("<I", bits + 1))[0]


def write_asset(path, content):
    if path.is_file() and path.read_bytes() == content:
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=path.parent, delete=False) as stream:
        staged = Path(stream.name)
        try:
            stream.write(content)
            stream.close()
            staged.chmod(path.stat().st_mode & 0o777 if path.exists() else 0o644)
            staged.replace(path)
        finally:
            staged.unlink(missing_ok=True)


def compile_timelines(asset_root, write):
    manifest = json.loads((asset_root / "ALS/Animations/als_notify_manifest.json").read_bytes())
    durations = clip_durations(manifest)
    corrected = []
    other_differences = []
    for clip_name, length in sorted(durations.items()):
        clip_path = asset_root / clip_name
        clip = clip_path.read_bytes()
        path_match = CLIP_SOURCE.search(clip)
        name_match = ANIMATION_NAME.search(clip)
        if path_match is None or name_match is None:
            raise ValueError(f"Missing glTF source in {clip_name}")
        installed = path_match[1].decode()
        original = "ALS/" + installed.removeprefix("ALSRuntime/") if installed.startswith("ALSRuntime/") else installed
        original_path = asset_root / original
        document, original_blob = load_gltf(original_path)
        name = name_match[1].decode()
        animation = next((item for item in document["animations"] if item.get("name") == name), None)
        if animation is None:
            raise ValueError(f"Missing animation {name} in {original}")
        inputs = {sampler["input"] for sampler in animation["samplers"]}
        timelines = {index: [value[0] for value in accessor_values(document, original_blob, index)] for index in inputs}
        for index, times in timelines.items():
            if not times or times[-1] != f32(document["accessors"][index]["max"][0]):
                raise ValueError(f"Inconsistent timeline max in {original}")
        previous_length = max(times[-1] for times in timelines.values())
        length = f32(length)
        if previous_length == length:
            continue
        if previous_length <= 0.0 or next_float(previous_length) != length:
            other_differences.append({"clip": clip_name, "installed": previous_length, "manifest": length})
            continue
        if len(timelines) != 1 or not original.startswith("ALS/") or document.get("images"):
            raise ValueError(f"Unsupported derived timeline in {original}")
        index, times = next(iter(timelines.items()))
        if len(times) < 2 or any(first >= second for first, second in zip(times, times[1:])):
            raise ValueError(f"Invalid timeline in {original}")
        accessor = document["accessors"][index]
        if accessor["type"] != "SCALAR":
            raise ValueError(f"Invalid timeline accessor in {original}")
        view = document["bufferViews"][accessor["bufferView"]]
        offset = view.get("byteOffset", 0) + accessor.get("byteOffset", 0) + (len(times) - 1) * view.get("byteStride", 4)
        blob = bytearray(original_blob)
        struct.pack_into("<f", blob, offset, length)
        accessor["max"] = [length]
        derived_path = asset_root / "ALSRuntime" / original.removeprefix("ALS/")
        derived_blob = derived_path.with_suffix(".bin")
        document["buffers"][0]["uri"] = derived_blob.name
        derived = (json.dumps(document, ensure_ascii=False, indent=2) + "\n").encode()
        relative = derived_path.relative_to(asset_root).as_posix()
        redirected = clip[:path_match.start(1)] + relative.encode() + clip[path_match.end(1):]
        for path, expected in [(derived_blob, blob), (derived_path, derived), (clip_path, redirected)]:
            if write:
                write_asset(path, expected)
            elif not path.is_file() or path.read_bytes() != expected:
                raise ValueError(f"Stale timeline asset: {path.relative_to(asset_root).as_posix()}")
        corrected.append({
            "clip": clip_name, "source_gltf": original, "source_gltf_sha256": sha256(original_path.read_bytes()),
            "source_buffer_sha256": sha256(original_blob), "target_gltf": relative,
            "target_gltf_sha256": sha256(derived), "target_buffer_sha256": sha256(blob),
            "sample_count": len(times), "last_sample_offset": offset, "previous_duration": previous_length,
            "native_duration": length,
        })
    return {"clips": len(durations), "corrected": corrected, "other_differences": other_differences}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--asset-root", type=Path, default=Path(__file__).resolve().parents[2] / "assets")
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args()
    print(json.dumps(compile_timelines(args.asset_root, args.write), indent=2))


if __name__ == "__main__":
    main()
