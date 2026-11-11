import gzip
import json
from pathlib import Path


directory = Path(__file__).resolve().parent
root = directory.parents[3]


def native(name):
    with gzip.open(directory / f"{name}.native.json.gz") as stream:
        return json.load(stream)


roll = native("roll")
montage = native("montage")
manifest = json.loads((root / "assets/ALS/Animations/als_notify_manifest.json").read_bytes())
sequence, action = (next(clip for clip in manifest["clips"] if clip["source_asset"] == data["asset_path"])
                    for data in [roll, montage])
scale_event = next(event for event in sequence["events"] if event["class"] == "AlsAnimNotifyState_SetRootMotionScale")
action_event = next(event for event in action["events"] if event["class"] == "AlsAnimNotifyState_SetLocomotionAction")
scale_object = next(item for item in roll["asset_objects"] if item["class_path"].endswith("AlsAnimNotifyState_SetRootMotionScale"))
scale = next(prop["numeric_value"] for prop in scale_object["properties"] if prop["name"] == "TranslationScale")
assert scale_event["parameters"]["TranslationScale"] == scale
interval = next(event for event in roll["notify_clock_probe"]["cases"][0]["events"] if event["event_index"] == 0)
assert (scale_event["start_time"], scale_event["end_time"]) == (interval["trigger_time"], interval["end_trigger_time"])


def replay(frames, external, scale_end, action_end):
    value = 1.0
    rows = []
    for index, frame in enumerate(frames):
        previous = frames[index - 1]["position"] if index else 0.0
        assert not frame["valid"] or previous <= frame["position"] < scale_event["end_time"]
        phase = "end" if index == scale_end else "active" if index < scale_end and not frame["stopped"] else None
        if index == 0:
            assert frame["position"] > scale_event["start_time"] and frame["weight"] >= scale_event["weight_threshold"]
            assert action_event["start_time"] < frame["position"] <= action_event["end_time"]
            phase = "begin"
            value = scale
        if index == 2 and external is not None:
            value = external
        if phase == "end" and abs(value - scale) <= 1e-8:
            value = 1.0
        rows.append((phase, value, index == action_end))
    return rows


samples = {sample["name"]: sample["frames"] for sample in montage["montage_clock_probe"]["samples"]}
frames = samples["interrupted"]
stop = next(index for index, frame in enumerate(frames) if frame["stopped"])
termination = next(index for index, frame in enumerate(frames) if not frame["valid"])
assert stop == 2 < termination
assert frames[stop]["weight"] > scale_event["weight_threshold"]
assert all(frame["valid"] and not frame["stopped"] for frame in samples["playing"])

results = []
for external in [None, 2.0]:
    source = replay(frames, external, stop, termination)
    old = replay(frames, external, termination, termination)
    queued_end = next(index for index, frame in enumerate(frames) if frame["stopped"] or not frame["valid"])
    fixed = replay(frames, external, queued_end, termination)
    reset_all = replay(frames, external, stop, stop)
    assert source == fixed
    assert source[stop] == ("end", 1.0 if external is None else external, False)
    assert source[termination] == (None, 1.0 if external is None else external, True)
    assert source[stop] != old[stop] and source[stop] != reset_all[stop]
    results.append({
        "external_scale": external,
        "legacy_mismatches": sum(a != b for a, b in zip(source, old)),
        "reset_all_mismatches": sum(a != b for a, b in zip(source, reset_all)),
        "restored": fixed,
    })
control = replay(samples["playing"], None, 15, 15)
assert all(scale == value and not branch_end for _, value, branch_end in control)
print(json.dumps({"stop_frame": stop, "termination_frame": termination, "stop_weight": frames[stop]["weight"],
                  "uninterrupted_control_frames": len(control), "cases": results}, indent=2))
