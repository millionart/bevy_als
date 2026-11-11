import copy
import gzip
import json
import re
import struct
import subprocess
import sys
from pathlib import Path


directory = Path(__file__).resolve().parent
root = directory.parents[3]
sys.path.insert(0, str(root / "tools/import"))
from compile_als_event_tracks import ValidationError, clip_durations, f32, tracks

path = directory / "roll.native.json"
native = json.loads(path.read_bytes() if path.is_file() else gzip.decompress(path.with_suffix(".json.gz").read_bytes()))
probe = native["notify_clock_probe"]
assert probe["duration"] == 1.5
native_bounds = {(event["trigger_time"], event["end_trigger_time"]) for case in probe["cases"]
                 for event in case["events"] if event["event_index"] == 0}
assert native_bounds == {(9.999999747378752e-05, 1.500100016593933)}
start, end = native_bounds.pop()
scale_owner = next(value for value in native["asset_objects"] if value["object_path"].endswith(":AlsAnimNotifyState_RootMotionScale_0"))
translation_scale = next(value["numeric_value"] for value in scale_owner["properties"] if value["name"] == "TranslationScale")
assert translation_scale == f32(1.3)
asset = "ALS/Animations/Actions/Roll/A_Als_Roll.anim.ron"
manifest = json.loads((root / "assets/ALS/Animations/als_notify_manifest.json").read_bytes())
sequence = next(source for source in manifest["clips"] if source["source_asset"] == native["asset_path"])
event = next(event for event in sequence["events"] if event["name"] == "AlsRootMotionScale")
assert (event["start_time"], event["end_time"]) == (start, end)
assert event["parameters"]["TranslationScale"] == translation_scale
compiled = tracks(manifest)
roll = next(row for rows in compiled[asset].values() for row in rows if row["event_id"] == event["event_id"])
assert (roll["start_time"], roll["end_time"]) == (start, end)
text = (root / "assets" / asset).read_text()
pattern = rf'event: StringId\("{re.escape(event["event_id"])}"\),\s*start_time: ([^,]+),\s*end_time: ([^,]+),'
match = re.search(pattern, text)
assert match and tuple(f32(float(value)) for value in match.groups()) == (start, end)
assert start > 0.0 and end > 1.5
before = subprocess.check_output(["git", "show", f"04c792dc:assets/{asset}"], text=True)
original = re.search(pattern, before)
assert original and tuple(f32(float(value)) for value in original.groups()) == (0.0, 1.5)


def next_f32(value, direction):
    return struct.unpack("<f", struct.pack("<I", struct.unpack("<I", struct.pack("<f", value))[0] + direction))[0]


pre_start = next_f32(start, -1)
assert 0.0 <= pre_start < start < 1.5 < end

bad = copy.deepcopy(sequence)
invalid = next(item for item in bad["events"] if item["event_id"] == event["event_id"])
invalid["end_time"] = next_f32(end, 1)
try:
    tracks({"clips": [bad]})
except ValidationError as error:
    assert str(error) == "event_segment_mapping:" + event["event_id"]
else:
    raise AssertionError("Accepted an end beyond the native offset allowance")

durations = clip_durations(manifest)
compiled_events = [(clip, row) for clip, groups in compiled.items() for rows in groups.values() for row in rows]
print(json.dumps({"native_start": start, "native_end": end, "roll_event": event["event_id"],
                  "translation_scale": translation_scale, "pre_start": pre_start, "clip_end_is_in_state": True,
                  "compiled_clips": len(compiled), "compiled_events": len(compiled_events),
                  "starts_before_clip": sum(row["start_time"] < 0.0 for _, row in compiled_events),
                  "ends_after_clip": sum(row["end_time"] > durations[clip] for clip, row in compiled_events),
                  "end_above_native_offset_rejected": True}, indent=2))
