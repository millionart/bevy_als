import bisect
import json
import math
import sys
from collections import Counter
from pathlib import Path


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
sys.path.insert(0, str(HERE.parent / "d224d"))
from replay_pose_consumers import ASSETS, components, f32, project

from compile_als_per_track import compile_capture, read_capture


def relative_position(time, frame_rate, frame_count):
    numerator, denominator = frame_rate
    decimal = f32(time) * numerator / denominator
    frame = int(decimal)
    subframe = f32(decimal - frame)
    carry = int(subframe)
    limit = f32(0.99999994)
    fraction = min(max(f32(f32(min(subframe - carry, limit) + 0.5) - 0.5), 0.0), limit)
    return f32(min((frame + carry + fraction) / (frame_count - 1), 1.0))


def selection(track, position, frame_count, step, use_frame_table):
    count = len(track["keys"])
    if count == 1 or position <= 0:
        return 0, 0, 0.0
    if position >= 1:
        return count - 1, count - 1, 0.0
    frames = track["frames"]
    if frames and use_frame_table:
        sample = f32(position * (frame_count - 1))
        first = bisect.bisect_right(frames, min(int(sample), frame_count - 2)) - 1
        second = first + 1
        alpha = f32(f32(sample - frames[first]) / (frames[second] - frames[first]))
    else:
        sample = f32(position * (count - 1))
        first = int(sample)
        second = min(first + 1, count - 1)
        alpha = f32(sample - first)
    return first, second, 0.0 if step else alpha


def sampled_track(track, position, frame_count, step, rotation, use_frame_table=True, normalize=True):
    first, second, alpha = selection(track, position, frame_count, step, use_frame_table)
    start, end = track["keys"][first], track["keys"][second]
    if rotation:
        if first != second:
            dot = f32(start[0] * end[0])
            for a, b in zip(start[1:], end[1:]):
                dot = f32(dot + f32(a * b))
            weight = f32(1.0 - alpha) * (-1.0 if dot < 0.0 else 1.0)
            result = [f32(f32(b * alpha) + f32(a * weight)) for a, b in zip(start, end)]
        else:
            result = start
        inverse = 1.0 / math.sqrt(sum(value * value for value in result)) if normalize else 1.0
        return tuple(value * inverse for value in result)
    return tuple(f32(a + f32(f32(b - a) * alpha)) for a, b in zip(start, end)) if first != second else tuple(start)


def replay(compiled, prepared):
    counts = Counter()
    mismatches = []
    for frame in prepared["sequence_pose_probe"]["frames"]:
        assert frame["pose_evaluation_mode"] == "compressed" and not frame["curves"]
        position = relative_position(frame["time"], compiled["frame_rate"], compiled["frame_count"])
        for bone, native in zip(compiled["bones"], frame["bones"]):
            assert bone["path"][-1] == native["name"]
            tracks = [bone[key] for key in ("rotation", "translation", "scale")]
            args = (position, compiled["frame_count"], compiled["step"])
            actual = tuple(sampled_track(track, *args, rotation=index == 0) for index, track in enumerate(tracks))
            expected = components(native["local_transform"])
            if project(actual) != project(expected):
                mismatches.append((frame["time"], native["name"], project(actual), project(expected)))
            counts["projected_bones"] += 1
            ignored = sampled_track(tracks[0], *args, rotation=True, use_frame_table=False)
            counts["drop_frame_table"] += project((ignored, *actual[1:]))[0] != project(expected)[0]
            unnormalized = sampled_track(tracks[0], *args, rotation=True, normalize=False)
            counts["drop_rotation_normalization"] += project((unnormalized, *actual[1:]))[0] != project(expected)[0]
    assert not mismatches, (len(mismatches), mismatches[:5])
    assert counts["drop_frame_table"] > 0 and counts["drop_rotation_normalization"] > 0
    return dict(counts)


def main():
    capture = read_capture(HERE / "bow.native.json.gz")
    codec = read_capture(HERE / "codec.native.json.gz")
    expected = codec["sequence_pose_probe"]
    assert expected["compression"] == capture["sequences"][0]["compression"]
    assert all(bone["retarget_mode"] == 0 for bone in expected["frames"][0]["bones"])
    compiled = compile_capture(capture, ASSETS)
    path = ASSETS / "ALSRuntime/OverlayObjects/Bow/als_prepared_per_track.json"
    assert json.loads(path.read_bytes()) == compiled
    prepared = read_capture(HERE / "prepared.native.json.gz")
    assert prepared["asset_path"] == compiled["asset"]
    results = replay(compiled, prepared)
    original = read_capture(HERE.parent / "d224a/bow.native.json.gz")
    source = capture["sequences"][0]
    source.pop("forced_root_lock")
    layout = source["compression"].pop("per_track")
    assert capture == original
    formats = Counter(format_name for bone in compiled["bones"] for format_name in bone["formats"].values())
    tables = sum(bool(bone["rotation"]["frames"]) for bone in compiled["bones"])
    graph = ASSETS / "ALS/OverlayObjects/Bow/AB_Als_Bow/AnimGraph.animgraph.ron"
    assert '"als_ext::SourceSequenceEvaluatorNode"' in graph.read_text() and compiled["clip"] in graph.read_text()
    print(json.dumps({"result": "PASS_PREPARED_BOW_PER_TRACK", "layout": layout, "formats": dict(formats),
                      "rotation_frame_tables": tables, "sample_frames": len(prepared["sequence_pose_probe"]["frames"]),
                      "same_source_stream": True, **results}, indent=2))


if __name__ == "__main__":
    main()
