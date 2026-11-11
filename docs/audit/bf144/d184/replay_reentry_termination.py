import gzip
import json
import re
from pathlib import Path


directory = Path(__file__).resolve().parent
with gzip.open(directory / "reentry.native.json.gz") as stream:
    native = json.load(stream)
properties = {item["name"]: item["exported_text"] for item in native["asset_properties"]}
assert properties["BranchingPointStateNotifyIndices"] == "(0)"
state = next(item for item in native["asset_objects"]
             if item["class_path"].endswith("AlsAnimNotifyState_SetLocomotionAction"))
tag_text = next(prop["exported_text"] for prop in state["properties"] if prop["name"] == "LocomotionAction")
tag = re.fullmatch(r'\(TagName="([^"]+)"\)', tag_text)[1]
assert tag == "Als.LocomotionAction.Rolling"

samples = {sample["name"]: sample["frames"] for sample in native["montage_clock_probe"]["samples"]}
old = samples["old"]
new = samples["new"]
stop = next(index for index, frame in enumerate(old) if frame["stopped"])
termination = next(index for index, frame in enumerate(old) if not frame["valid"])
assert 0 < stop < termination
assert old[stop]["weight"] > 0.0
start, end = map(float, re.findall(r"TriggerTime=([\d.-]+)", properties["BranchingPointMarkers"]))
assert start < old[0]["position"] < end
assert all(start < frame["position"] < end and frame["valid"] for frame in new)


def replay(override, superseded_guard):
    action = tag
    frames = []
    for index, old_frame in enumerate(old):
        if index == stop:
            action = tag
        if override is not None and index == termination:
            action = override
        if index == termination and action == tag and not superseded_guard:
            action = ""
        frames.append({"old_valid": old_frame["valid"], "new_began": index == stop,
                       "old_ended": index == termination, "action": action})
    return frames


cases = []
for external in [None, "Als.LocomotionAction.Ragdolling"]:
    restored = replay(external, False)
    old_guard = replay(external, True)
    assert restored[termination]["action"] == (external or "")
    assert all(frame["action"] == restored[termination]["action"] for frame in restored[termination:])
    mismatches = sum(old_row != row for old_row, row in zip(old_guard, restored))
    assert bool(mismatches) == (external is None)
    cases.append({"external": external, "legacy_mismatches": mismatches, "frames": restored})
print(json.dumps({"stop_frame": stop, "termination_frame": termination, "cases": cases}, indent=2))
