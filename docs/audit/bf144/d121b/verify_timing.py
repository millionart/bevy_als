import hashlib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[4]
sys.path[:0] = [str(ROOT / "tools/animgraph_authoring"), str(ROOT / "tools/import")]
from ue_animation_asset_resolver import sequence_timing_contract, source_sequence_timing, source_uasset_path
from ue_animation_timing import f32


def read(name):
    return json.loads((Path(__file__).resolve().parent / name).read_text(encoding="utf-8-sig"))


def fields(timing):
    return {key:timing[key] for key in ("duration","rate_scale","markers")}


def verify(content):
    assets = []
    rounded_durations = rounded_markers = 0
    for file in ("crouch", "stand-a", "stand-b"):
        document = read(file + ".native.json")["sync_group_clock_probe"]
        for player in document["players"]:
            asset = player["asset_path"]
            expected = fields({"markers":[], **player})
            actual = fields(source_sequence_timing(content, "/ALS", asset))
            assert actual == expected, asset
            source_file = source_uasset_path(content, "/ALS", asset)
            assets.append({"asset":asset, "sha256":hashlib.sha256(source_file.read_bytes()).hexdigest(), **actual})
            rounded_durations += f32(round(actual["duration"], 6)) != actual["duration"]
            rounded_markers += sum(f32(round(marker["time"], 6)) != marker["time"] for marker in actual["markers"])
    fallback = read("sequence-timing.native.json")
    assert fields(sequence_timing_contract(fallback)) == fields(assets[0])
    native = read("crouch.native.json")["sync_group_clock_probe"]
    frame = native["samples"][0]["frames"][27]
    right = next(marker["time"] for marker in assets[0]["markers"] if marker["name"] == "Right")
    position = frame["players"][0]["position"]
    rounded_right = f32(round(right, 6))
    assert right < position < rounded_right
    assert (frame["marker_end"]["previous"], frame["marker_end"]["next"]) == ("Right","Left")
    assert len(assets) == 26 and rounded_durations > 0 and rounded_markers > 0
    return {"assets":assets, "rounded_duration_failures":rounded_durations,
            "rounded_marker_failures":rounded_markers, "native_fallback_matches":True,
            "crouch_left_backward_frame_28":{"position":position,"native_right_marker":right,
              "rounded_right_marker":rounded_right, "native_group_end":frame["marker_end"]}}


if __name__ == "__main__":
    print(json.dumps(verify(Path(sys.argv[1]).resolve()),indent=2))
