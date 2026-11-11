import gzip
import json
from pathlib import Path


directory = Path(__file__).resolve().parent
root = directory.parents[3]
with gzip.open(directory / "../d149/locomotion.native.json.gz", "rt") as stream:
    source = json.load(stream)
machine, = [graph for graph in source["graphs"] if ":AnimGraph." in graph["object_path"]
            and graph["object_path"].endswith(".Jump States")]
nodes = {node["title"]: node for node in machine["nodes"]}
target = root / "assets/ALS/Character/AnimationInstances/AB_Als_Locomotion/AnimGraph/SM9_Locomotion_States/S2_Jump/SM0_Jump_States"
definition = target.with_suffix(".fsm.ron").read_text()
for index, label, operator in [(0, "Left Foot", "LessEqual_DoubleDouble"), (1, "Right Foot", "Greater_DoubleDouble")]:
    transition = nodes["Entry Conduit to " + label]
    assert transition["object_path"].endswith(f"AnimStateTransitionNode_{index}")
    outgoing = next(pin for pin in transition["pins"] if pin["name"] == "Out")
    assert outgoing["linked_to"][0]["node_guid"] == nodes[label]["node_guid"]
    rule, = [graph for graph in source["graphs"] if graph["object_path"] == transition["object_path"] + ".Transition"]
    comparison, = [node for node in rule["nodes"] if any(operator in prop["exported_text"] for prop in node["properties"])]
    threshold = next(pin for pin in comparison["pins"] if pin["name"] == "B")
    assert not threshold["linked_to"] and float(threshold["default_value"] or "0.0") == 0.0
    assert any(prop["exported_text"] == "GetParent.FeetState.FootPlantedAmount"
               for node in rule["nodes"] for prop in node["properties"])
    assert nodes[label]["node_guid"] in definition
    assert 'global_key: "feet_state.foot_planted_amount"' in (target / f"T{index}_Transition.condition.animgraph.ron").read_text()

cases = []
for valid in (False, True):
    for retained in (-0.8, 0.8):
        for current in (-0.8, 0.8):
            planted = current if valid else retained
            crossing = 0.7 if valid else 0.2
            cases.append({
                "valid": valid,
                "retained_planted": retained,
                "current_planted_curve": current,
                "effective_planted": planted,
                "effective_crossing": crossing,
                "source_jump_foot": "Left Foot" if planted <= 0.0 else "Right Foot",
                "jump_foot_from_current_curve": "Left Foot" if current <= 0.0 else "Right Foot",
            })
failures = [case for case in cases if case["source_jump_foot"] != case["jump_foot_from_current_curve"]]
assert len(failures) == 2 and all(not case["valid"] for case in failures)
print(json.dumps({"cases": cases, "jump_foot_failures_with_current_curve": len(failures)}, indent=2))
