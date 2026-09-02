from __future__ import annotations

import argparse
import json
import math
import struct
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools" / "uasset_root_export"))

from coordinate_basis import (
    source_direction_to_runtime,
    source_point_to_runtime,
    source_rotation_to_runtime,
)
from extract_root_track import ExtractError, decode_anim_data_model
from uasset import UAsset


COMPONENT_COUNTS = {
    "SCALAR": 1,
    "VEC2": 2,
    "VEC3": 3,
    "VEC4": 4,
}


def load_gltf(path: Path) -> tuple[dict, bytes]:
    document = json.loads(path.read_text(encoding="utf-8"))
    buffers = document.get("buffers")
    if not isinstance(buffers, list) or len(buffers) != 1:
        raise ValueError("expected exactly one gltf buffer")
    uri = buffers[0].get("uri")
    if not isinstance(uri, str) or not uri:
        raise ValueError("gltf buffer uri is invalid")
    return document, (path.parent / uri).read_bytes()


def accessor_values(document: dict, blob: bytes, index: int) -> list[tuple[float, ...]]:
    accessors = document.get("accessors", [])
    buffer_views = document.get("bufferViews", [])
    if not isinstance(index, int) or index < 0 or index >= len(accessors):
        raise ValueError(f"accessor index is invalid: {index}")
    accessor = accessors[index]
    if accessor.get("componentType") != 5126:
        raise ValueError(f"accessor component type is unsupported: {accessor.get('componentType')}")
    component_count = COMPONENT_COUNTS.get(accessor.get("type"))
    if component_count is None:
        raise ValueError(f"accessor type is unsupported: {accessor.get('type')}")
    view_index = accessor.get("bufferView")
    if not isinstance(view_index, int) or view_index < 0 or view_index >= len(buffer_views):
        raise ValueError(f"buffer view index is invalid: {view_index}")
    view = buffer_views[view_index]
    stride = int(view.get("byteStride", component_count * 4))
    item_size = component_count * 4
    if stride < item_size:
        raise ValueError("buffer view stride is invalid")
    start = int(view.get("byteOffset", 0)) + int(accessor.get("byteOffset", 0))
    count = int(accessor.get("count", 0))
    end = start + (count - 1) * stride + item_size if count else start
    if start < 0 or end > len(blob):
        raise ValueError("accessor range exceeds buffer")
    return [struct.unpack_from("<" + "f" * component_count, blob, start + item * stride) for item in range(count)]


def target_node_index(document: dict, bone: str) -> int:
    matches = [index for index, node in enumerate(document.get("nodes", [])) if node.get("name") == bone]
    if len(matches) != 1:
        raise ValueError(f"target bone node count is {len(matches)} for {bone}")
    return matches[0]


def target_channels(document: dict, bone: str) -> dict[str, tuple[list[float], list[tuple[float, ...]]]]:
    animations = document.get("animations", [])
    if len(animations) != 1:
        raise ValueError(f"expected exactly one animation, found {len(animations)}")
    animation = animations[0]
    node_index = target_node_index(document, bone)
    channels: dict[str, tuple[list[float], list[tuple[float, ...]]]] = {}
    return node_index, animation, channels


def animation_channel_values(
    document: dict,
    blob: bytes,
    bone: str,
) -> dict[str, tuple[list[float], list[tuple[float, ...]]]]:
    node_index, animation, channels = target_channels(document, bone)
    samplers = animation.get("samplers", [])
    for channel in animation.get("channels", []):
        target = channel.get("target", {})
        path = target.get("path")
        if target.get("node") != node_index or path not in {"translation", "rotation", "scale"}:
            continue
        sampler_index = channel.get("sampler")
        if not isinstance(sampler_index, int) or sampler_index < 0 or sampler_index >= len(samplers):
            raise ValueError(f"gltf sampler is invalid for {bone}.{path}")
        if path in channels:
            raise ValueError(f"gltf channel is duplicated for {bone}.{path}")
        sampler = samplers[sampler_index]
        times = [value[0] for value in accessor_values(document, blob, sampler.get("input"))]
        values = accessor_values(document, blob, sampler.get("output"))
        if len(times) != len(values):
            raise ValueError(f"gltf sampler count mismatch for {bone}.{path}")
        channels[path] = (times, values)
    return channels


def source_times(count: int, frames_per_second: float) -> list[float]:
    if count < 1:
        return []
    if count == 1:
        return [0.0]
    return [index / frames_per_second for index in range(count)]


def normalized_quaternion(value: tuple[float, ...] | list[float]) -> tuple[float, float, float, float]:
    if len(value) != 4:
        raise ValueError("quaternion component count is invalid")
    length = math.sqrt(sum(component * component for component in value))
    if not math.isfinite(length) or length <= 1e-12:
        raise ValueError("quaternion is invalid")
    return tuple(component / length for component in value)


def vector_error(left: list[float] | tuple[float, ...], right: list[float] | tuple[float, ...]) -> float:
    if len(left) != len(right):
        raise ValueError("vector component count mismatch")
    return math.sqrt(sum((left_value - right_value) ** 2 for left_value, right_value in zip(left, right)))


def quaternion_error(left: list[float] | tuple[float, ...], right: list[float] | tuple[float, ...]) -> float:
    left_normalized = normalized_quaternion(left)
    right_normalized = normalized_quaternion(right)
    alignment = abs(sum(left_value * right_value for left_value, right_value in zip(left_normalized, right_normalized)))
    return 1.0 - min(1.0, alignment)


def compare_channel(
    source_values: list[tuple[float, ...]],
    target: tuple[list[float], list[tuple[float, ...]]] | None,
    frames_per_second: float,
    converter,
    error_fn,
    tolerance: float,
) -> dict:
    source_times_values = source_times(len(source_values), frames_per_second)
    expected = [tuple(converter(value)) for value in source_values]
    issues: list[str] = []
    maximum_error = 0.0
    if target is None:
        issues.append("target channel is missing")
        return {
            "source_keys": len(source_values),
            "target_keys": 0,
            "maximum_error": math.inf,
            "issues": issues,
        }
    target_times_values, target_values = target
    if len(target_times_values) != len(source_times_values):
        issues.append("key count differs")
    shared = min(len(target_times_values), len(source_times_values), len(target_values), len(expected))
    for index in range(shared):
        if abs(target_times_values[index] - source_times_values[index]) > tolerance:
            issues.append(f"time differs at key {index}")
            break
    for index in range(shared):
        error = error_fn(expected[index], target_values[index])
        maximum_error = max(maximum_error, error)
        if error > tolerance:
            issues.append(f"value differs at key {index}")
            break
    if len(target_values) != len(target_times_values):
        issues.append("target value count differs")
    return {
        "source_keys": len(source_values),
        "target_keys": len(target_values),
        "maximum_error": maximum_error,
        "issues": issues,
    }


def frame_rate(model: dict) -> float:
    rate = model.get("frame_rate", {})
    numerator = rate.get("Numerator")
    denominator = rate.get("Denominator", 1)
    if not isinstance(numerator, int) or not isinstance(denominator, int) or numerator <= 0 or denominator <= 0:
        raise ValueError(f"source frame rate is invalid: {rate}")
    return numerator / denominator


def audit(source_uasset: Path, target_gltf: Path, bones: list[str], tolerance: float) -> dict:
    model = decode_anim_data_model(UAsset(str(source_uasset)))
    document, blob = load_gltf(target_gltf)
    fps = frame_rate(model)
    source_tracks = model.get("tracks", {})
    rows = []
    for bone in bones:
        track = source_tracks.get(bone)
        if track is None:
            rows.append({"bone": bone, "issues": ["source track is missing"]})
            continue
        channels = animation_channel_values(document, blob, bone)
        translation = compare_channel(
            track.get("pos", []),
            channels.get("translation"),
            fps,
            source_point_to_runtime,
            vector_error,
            tolerance,
        )
        rotation = compare_channel(
            track.get("rot", []),
            channels.get("rotation"),
            fps,
            source_rotation_to_runtime,
            quaternion_error,
            tolerance,
        )
        scale = compare_channel(
            track.get("scale", []),
            channels.get("scale"),
            fps,
            source_direction_to_runtime,
            vector_error,
            tolerance,
        )
        rows.append(
            {
                "bone": bone,
                "translation": translation,
                "rotation": rotation,
                "scale": scale,
                "issues": translation["issues"] + rotation["issues"] + scale["issues"],
            }
        )
    issues = [f"{row['bone']}: {issue}" for row in rows for issue in row["issues"]]
    return {
        "format": "als_animation_transform_audit_v1",
        "source_uasset": source_uasset.as_posix(),
        "target_gltf": target_gltf.as_posix(),
        "frames_per_second": fps,
        "source_track_count": model.get("track_count"),
        "bones": rows,
        "issues": issues,
        "pass": not issues,
    }


def self_test() -> None:
    assert source_point_to_runtime([100.0, 200.0, 300.0]) == [1.0, 3.0, 2.0]
    assert source_direction_to_runtime([1.0, 2.0, 3.0]) == [1.0, 3.0, 2.0]
    assert source_rotation_to_runtime([0.2, -0.3, 0.4, 0.5]) == [-0.2, -0.4, 0.3, 0.5]
    translation = compare_channel(
        [(100.0, 200.0, 300.0)],
        ([0.0], [(1.0, 3.0, 2.0)]),
        30.0,
        source_point_to_runtime,
        vector_error,
        1e-6,
    )
    assert not translation["issues"]
    rotation = compare_channel(
        [(0.0, 0.0, 0.0, 1.0)],
        ([0.0], [(0.0, 0.0, 0.0, -1.0)]),
        30.0,
        source_rotation_to_runtime,
        quaternion_error,
        1e-6,
    )
    assert not rotation["issues"]
    mismatch = compare_channel(
        [(0.0, 0.0, 0.0)],
        ([0.1], [(0.0, 0.0, 0.0)]),
        30.0,
        source_direction_to_runtime,
        vector_error,
        1e-6,
    )
    assert mismatch["issues"]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source-uasset", type=Path)
    parser.add_argument("--target-gltf", type=Path)
    parser.add_argument("--bone", action="append", dest="bones")
    parser.add_argument("--tolerance", type=float, default=3e-5)
    parser.add_argument("--report", type=Path)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    self_test()
    if args.self_test:
        print("animation transform conversion self-test PASS")
        return 0
    if args.source_uasset is None or args.target_gltf is None or not args.bones:
        raise ValueError("source-uasset, target-gltf, and at least one bone are required")
    if not math.isfinite(args.tolerance) or args.tolerance <= 0.0:
        raise ValueError("tolerance must be positive")
    report = audit(args.source_uasset, args.target_gltf, args.bones, args.tolerance)
    if args.report is not None:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(
        "animation transform conversion "
        f"bones={len(report['bones'])} "
        f"issues={len(report['issues'])} "
        f"-> {'PASS' if report['pass'] else 'FAIL'}"
    )
    for issue in report["issues"]:
        print(f"DIFF {issue}")
    return 0 if report["pass"] or not args.check else 1


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ExtractError, ValueError) as error:
        raise SystemExit(f"FAIL {error}") from error
