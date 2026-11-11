import difflib
import gzip
import json
import struct
import subprocess
import sys
import tempfile
from pathlib import Path


directory = Path(__file__).resolve().parent
root = directory.parents[3]
assets = root / "assets"
baseline = "5ca1d2c0"
sys.path.insert(0, str(root / "tools/import"))
from compile_als_clip_timelines import compile_timelines, load_gltf, sha256
from gen_ron import check_anim


def original(path):
    return subprocess.check_output(["git", "show", f"{baseline}:assets/{path}"], cwd=root)


def json_lines(document):
    return (json.dumps(document, indent=2, ensure_ascii=False) + "\n").splitlines(keepends=True)


def native_lengths():
    result = {}
    for case in json.loads((directory / "timing-sources.json").read_bytes()):
        name = case["source_asset"].rsplit(".", 1)[-1]
        with gzip.open(directory / f"{name}.native.json.gz") as stream:
            report = json.load(stream)
        assert report["asset_path"] == case["source_asset"]
        timing = report.get("notify_clock_probe", {}).get("duration")
        if timing is None:
            timing = next(prop["numeric_value"] for prop in report["asset_properties"] if prop["name"] == "SequenceLength")
        assert timing == case["manifest_duration"] or timing == case["previous_duration"], name
        result[case["graph_clip"]] = timing
    return result


lengths = native_lengths()
if len(sys.argv) > 1 and sys.argv[1] == "--source-only":
    print(json.dumps({"native_lengths": lengths}, indent=2))
    sys.exit(0)

report = compile_timelines(assets, write=False)
assert report["clips"] == 128
assert len(report["corrected"]) == 20
assert len(report["other_differences"]) == 1
metadata = report["other_differences"][0]
assert metadata["manifest"] == 0.0 and metadata["installed"] == lengths[metadata["clip"]] == 1.0
virtual_bones = json.loads((assets / "ALS/Data/AnimationInstance/als_raw_virtual_bones.json").read_bytes())
assert not set(virtual_bones["raw_gltf_paths"]).intersection(row["source_gltf"] for row in report["corrected"])
rows = []
with (directory / "timelines-full.diff").open("w", encoding="utf-8") as diff:
    for row in report["corrected"]:
        clip_name = row["clip"]
        before_path, after_path = row["source_gltf"], row["target_gltf"]
        assert lengths[clip_name] == row["native_duration"]
        before, before_buffer = load_gltf(assets / before_path)
        after, after_buffer = load_gltf(assets / after_path)
        errors, _ = check_anim(after, after_path, Path(before_path).stem)
        assert not errors, errors
        raw_buffer = (Path(before_path).parent / before["buffers"][0]["uri"]).as_posix()
        assert original(before_path) == (assets / before_path).read_bytes()
        assert original(raw_buffer) == before_buffer
        offset = row["last_sample_offset"]
        assert before_buffer[:offset] == after_buffer[:offset]
        assert before_buffer[offset + 4:] == after_buffer[offset + 4:]
        assert struct.unpack_from("<f", before_buffer, offset)[0] == row["previous_duration"]
        assert struct.unpack_from("<f", after_buffer, offset)[0] == row["native_duration"]
        patched = json.loads(json.dumps(before))
        changes = [i for i, pair in enumerate(zip(before["accessors"], after["accessors"])) if pair[0] != pair[1]]
        assert len(changes) == 1
        patched["accessors"][changes[0]]["max"] = [row["native_duration"]]
        patched["buffers"][0]["uri"] = after["buffers"][0]["uri"]
        assert patched == after
        clip = (assets / clip_name).read_bytes()
        assert clip.replace(after_path.encode(), before_path.encode(), 1) == original(clip_name)
        diff.writelines(difflib.unified_diff(
            original(clip_name).decode().splitlines(keepends=True),
            clip.decode().splitlines(keepends=True), fromfile="a/assets/" + clip_name, tofile="b/assets/" + clip_name,
        ))
        diff.writelines(difflib.unified_diff(
            json_lines(before), json_lines(after),
            fromfile="a/assets/" + before_path, tofile="b/assets/" + after_path,
        ))
        rows.append({"clip": clip_name, "binary_offset": offset, "before_hex": before_buffer[offset:offset + 4].hex(),
                     "after_hex": after_buffer[offset:offset + 4].hex(),
                     "previous_endpoint_remaining_seconds": row["native_duration"] - row["previous_duration"]})

sample = next(row for row in report["corrected"] if "Jump_Walk_Left" in row["clip"])
with tempfile.TemporaryDirectory() as temporary:
    fixture = Path(temporary)
    paths = [sample["clip"], sample["source_gltf"]]
    document = json.loads(original(sample["source_gltf"]))
    paths.append((Path(sample["source_gltf"]).parent / document["buffers"][0]["uri"]).as_posix())
    for path in paths:
        destination = fixture / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(original(path))
    manifest = json.loads((assets / "ALS/Animations/als_notify_manifest.json").read_bytes())
    manifest["clips"] = [clip for clip in manifest["clips"] if clip["source_kind"] == "AnimSequence"
                         and clip["runtime_segments"][0]["graph_clip"] == sample["clip"]]
    assert len(manifest["clips"]) == 1
    (fixture / "ALS/Animations/als_notify_manifest.json").write_text(json.dumps(manifest))
    generated = compile_timelines(fixture, write=True)
    assert compile_timelines(fixture, write=False) == generated
    buffer = (fixture / sample["target_gltf"]).with_suffix(".bin")
    blob = bytearray(buffer.read_bytes())
    struct.pack_into("<f", blob, sample["last_sample_offset"], sample["previous_duration"])
    buffer.write_bytes(blob)
    try:
        compile_timelines(fixture, write=False)
    except ValueError as error:
        assert str(error).startswith("Stale timeline asset:")
    else:
        raise AssertionError("Missing endpoint correction was accepted")
    assert compile_timelines(fixture, write=True) == generated
    assert sha256(buffer.read_bytes()) == sample["target_buffer_sha256"]

print(json.dumps({"native_lengths": lengths, "timeline_differences": rows, "metadata_control": metadata,
                  "ablation_rejected": True, "ablation_recovered": True}, indent=2))
