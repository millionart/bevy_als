import gzip
import json
import struct
from pathlib import Path


directory = Path(__file__).resolve().parent
root = directory.parents[3]


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def native(name):
    with gzip.open(directory / f"{name}.native.json.gz") as stream:
        return json.load(stream)


slot = native("slot")["slot_probe"]
roll = native("roll")
manifest = json.loads((root / "assets/ALS/Animations/als_notify_manifest.json").read_bytes())
montage = next(clip for clip in manifest["clips"] if clip["source_asset"] == roll["asset_path"])
sequence = next(clip for clip in manifest["clips"] if clip["source_asset"].endswith("/A_Als_Roll.A_Als_Roll"))
scale = next(event for event in sequence["events"] if event["class"] == "AlsAnimNotifyState_SetRootMotionScale")
threshold = f32(1e-5)
assert scale["weight_threshold"] == threshold
assert montage["runtime_segments"][0]["slot"] == slot["slot_name"]
assert montage["runtime_segments"][0]["graph_clip"] == sequence["runtime_segments"][0]["graph_clip"]
tracks = next(item["exported_text"] for item in roll["asset_properties"] if item["name"] == "SlotAnimTracks")
assert f'SlotName="{slot["slot_name"]}"' in tracks and sequence["source_asset"] in tracks
steps = json.loads((directory / "slot.request.json").read_bytes())["notify_relevance_steps"]
source = [row["relevant"] for row in slot["notify_relevance"]]
assert len(source) == len(steps)
assert not any(row["unregistered_relevant"] for row in slot["notify_relevance"])


def replay(*, history=1, advance_frozen=False, inclusive=False, require_global=False):
    graph_frame = 0
    last_relevant = None
    result = []
    for step in steps:
        if step["update_graph"] or advance_frozen:
            graph_frame += 1
        if step["update_graph"] and "slot_weight" in step:
            weight = f32(step["slot_weight"])
            eligible = weight >= threshold if inclusive else weight > threshold
            if eligible and (not require_global or step["global_weight"] > 0.0):
                last_relevant = graph_frame
        result.append(last_relevant is not None and graph_frame - last_relevant <= history)
    return result


assert replay() == source
ablations = {}
for name, args in [
    ("no_previous_graph_frame", {"history": 0}),
    ("age_on_hidden_tick", {"advance_frozen": True}),
    ("inclusive_slot_threshold", {"inclusive": True}),
    ("global_weight_gate", {"require_global": True}),
]:
    count = sum(first != second for first, second in zip(source, replay(**args)))
    assert count > 0, name
    ablations[name] = count
previous_weight = f32(9.999999e-6)
assert previous_weight < threshold
weights = [previous_weight, threshold, 1.0]
dispatch = [[related and instance >= threshold for instance in weights] for related in source]
assert all(row[0] is False and row[1] == row[2] for row in dispatch)
print(json.dumps({"slot": slot["slot_name"], "source_relevance": source,
                  "legacy_unconditional_mismatches": source.count(False),
                  "instance_weights": weights, "dispatch": dispatch, "ablations": ablations}, indent=2))
