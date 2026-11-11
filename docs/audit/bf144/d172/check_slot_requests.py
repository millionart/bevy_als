import gzip
import json
import sys
from pathlib import Path


def load(path):
    data = path.read_bytes() if path.is_file() else gzip.decompress(path.with_suffix(".json.gz").read_bytes())
    return json.loads(data)


directory = Path(__file__).resolve().parent
assets = directory.parents[3] / "assets"
source_only = sys.argv[1:] == ["--source-only"]
rows = []
for stance, slot_name, idle_path in (
    ("standing", "TurnInPlaceStanding", "SM6_Standing_States/S0_Idle.animgraph.ron"),
    ("crouching", "TurnInPlaceCrouching", "SM2_Crouching_States/S8_Idle.animgraph.ron"),
):
    source = load(directory / f"{stance}.native.json")
    parent = next(graph for graph in source["graphs"] if graph["name"] == "AnimGraph")
    inertial = next(node for node in parent["nodes"] if node["class_path"].endswith(".AnimGraphNode_Inertialization")
                    and any("RotationYawSpeed" in prop["exported_text"] for prop in node["properties"]))
    links = next(pin for pin in inertial["pins"] if pin["name"] == "Source")["linked_to"]
    assert len(links) == 1 and links[0]["pin_name"] == "Pose"
    machine = links[0]["node_object_path"]
    idle = next(graph for graph in source["graphs"] if graph["name"] == "Idle" and graph["object_path"].startswith(machine + "."))
    moving = next(graph for graph in source["graphs"] if graph["name"] == "Move" and graph["object_path"].startswith(machine + "."))
    slot = next(node for node in idle["nodes"] if node["class_path"].endswith(".AnimGraphNode_Slot") and slot_name in node["title"])
    assert all(not node["class_path"].endswith(".AnimGraphNode_Slot") for node in moving["nodes"])
    rows.append({"stance": stance, "idle_slot": slot["node_guid"], "parent_inertialization": inertial["node_guid"],
                 "source_state_machine": machine, "idle": idle["object_path"], "move": moving["object_path"]})
    if not source_only:
        root = assets / f"ALS/Character/AnimationInstances/Stances/AB_Als_{stance.title()}"
        parent_line = next(line for line in (root / "AnimGraph.animgraph.ron").read_text().splitlines()
                           if inertial["node_guid"] in line and "InertializationPassthroughNode" in line)
        slot_line = next(line for line in (root / "AnimGraph" / idle_path).read_text().splitlines()
                         if slot["node_guid"] in line and "SourceSlotNode" in line)
        for key in ("epoch", "duration"):
            assert f'request_{key}_key: ""' in parent_line
            assert f'request_{key}_key: "als_{stance}_turn_inertialization_{key}"' in slot_line

print(json.dumps({"stances": rows, "installed_slot_bindings_checked": not source_only}, indent=2))
