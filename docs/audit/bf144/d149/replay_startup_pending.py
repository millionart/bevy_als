import gzip
import hashlib
import json
from pathlib import Path


directory = Path(__file__).resolve().parent
assets = directory.parents[3] / "assets/ALS/Character"
root = json.loads((directory / "ab-als.native.json").read_text())
linked, = [node for graph in root["graphs"] if graph["object_path"].endswith(":AnimGraph")
           for node in graph["nodes"] if node["title"] == "AB_Als_Locomotion\nLinked Anim Graph"]
assert any("AB_Als_Locomotion.AB_Als_Locomotion_C" in prop["exported_text"] for prop in linked["properties"])
assert linked["node_guid"] in (assets / "AB_Als/AnimGraph.animgraph.ron").read_text()

raw = gzip.decompress((directory / "locomotion.native.json.gz").read_bytes())
assert hashlib.sha256(raw).hexdigest() == (directory / "locomotion.native.sha256").read_text().split()[0]
source = json.loads(raw)
machine, = [graph for graph in source["graphs"]
            if ":AnimGraph." in graph["object_path"] and graph["object_path"].endswith(".Locomotion States")]
nodes = {node["title"]: node for node in machine["nodes"]}
installed = assets / "AnimationInstances/AB_Als_Locomotion/AnimGraph/SM9_Locomotion_States"
definition = installed.with_suffix(".fsm.ron").read_text()
routes = []
for index, label in [(19, "Jump"), (20, "Fall")]:
    transition = nodes[f"In Air Alias to {label}"]
    assert transition["object_path"].endswith(f"AnimStateTransitionNode_{index}")
    assert next(pin for pin in transition["pins"] if pin["name"] == "Out")["linked_to"][0]["node_guid"] == nodes[label]["node_guid"]
    rule, = [graph for graph in source["graphs"] if graph["object_path"] == transition["object_path"] + ".Transition"]
    default, = [pin["default_value"] for node in rule["nodes"] if node["title"] == "Equal (GameplayTag)"
                for pin in node["pins"] if pin["name"] == "B"]
    assert default == '(TagName="Als.LocomotionMode.InAir")'
    needs_jump = any(prop["exported_text"] == "GetParent.InAirState.bJumped" for node in rule["nodes"] for prop in node["properties"])
    assert needs_jump == (label == "Jump")
    if needs_jump:
        assert any(node["title"] == "AND Boolean" for node in rule["nodes"])
    priority = int(next(prop["exported_text"] for prop in transition["properties"] if prop["name"] == "PriorityOrder"))
    routes.append((priority, label, needs_jump))
    assert f'id: "{nodes[label]["node_guid"]}", label: "{label}"' in definition
    assert f'id: "{transition["node_guid"]}"' in definition
    generated_rule = (installed / f"T{index}_Transition.condition.animgraph.ron").read_text()
    assert "Als.LocomotionMode.InAir" in generated_rule
    assert ("in_air_state.b_jumped" in generated_rule) == needs_jump

results = []
for case, pending in [("first_ordinary", True), ("first_ordinary_without_registration_guard", False), ("second_ordinary", False)]:
    jumped = not pending
    state = next(label for _, label, needs_jump in sorted(routes) if jumped or not needs_jump)
    results.append({"case": case, "pending_during_jump_request": pending, "jumped": jumped, "state": state})
assert [result["state"] for result in results] == ["Fall", "Jump", "Jump"]
print(json.dumps({"linked_locomotion": linked["node_guid"], "cases": results}, indent=2))
