import gzip
import json
from pathlib import Path


directory = Path(__file__).resolve().parent
root = directory.parents[3]
with gzip.open(directory / "../d150/standing.native.json.gz", "rt") as stream:
    source = json.load(stream)
callback, = [node for graph in source["graphs"] if ":AnimGraph." in graph["object_path"]
             and graph["object_path"].endswith(".Idle") for node in graph["nodes"]
             if node["title"] == "RefreshDynamicTransitions\nOn Update"]
target = root / "assets/ALS/Character/AnimationInstances/Stances/AB_Als_Standing/AnimGraph/SM6_Standing_States/S0_Idle.animgraph.ron"
assert callback["node_guid"] in target.read_text()
settings = json.loads((directory / "../d147/settings.native.json").read_text())
dynamic, = [prop for prop in settings["asset_properties"] if prop["name"] == "DynamicTransitions"]
fields = {prop["name"]: prop["exported_text"] for prop in dynamic["fields"]}
threshold = float(fields["FootLockDistanceThreshold"]) * 0.01
assert threshold == 0.08 and "A_Als_Stand_DynamicTransition_Left" in fields["StandingLeftSequence"]

cases = []
for pending, shift in [(True, 0.0), (True, 0.2), (False, 0.2)]:
    for retain_current_target in (False, True):
        distance_squared = (shift if retain_current_target or not pending else 0.0) ** 2
        cases.append({
            "pending": pending,
            "component_shift_m": shift,
            "current_targets_sampled": retain_current_target or not pending,
            "left_lock_amount": 1.0,
            "rig_valid": not pending,
            "left_distance_squared_m2": distance_squared,
            "queued_sequence": "StandingLeft" if distance_squared > threshold**2 else None,
        })
failures = [case for case in cases if case["component_shift_m"] == 0.2 and case["queued_sequence"] is None]
assert len(failures) == 1 and failures[0]["pending"] and not failures[0]["current_targets_sampled"]
print(json.dumps({"idle_callback": callback["node_guid"], "threshold_m2": threshold**2,
                  "cases": cases, "skipped_sampling_failures": len(failures)}, indent=2))
