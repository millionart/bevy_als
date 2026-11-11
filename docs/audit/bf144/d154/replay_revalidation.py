import json
from pathlib import Path


directory = Path(__file__).resolve().parent
root = directory.parents[3]
source = json.loads((directory / "../d149/ab-als.native.json").read_text())
node, = [node for graph in source["graphs"] if graph["object_path"].endswith(":AnimGraph")
         for node in graph["nodes"] if node["title"] == "Control Rig"]
binding, = [prop for obj in source["asset_objects"] if obj["object_path"].startswith(node["object_path"] + ".")
            for prop in obj["properties"] if prop["name"] == "PropertyBindings"]
assert 'PropertyPath=("GetControlRigInput")' in binding["exported_text"]
installed = (root / "assets/ALS/Character/AB_Als/AnimGraph.animgraph.ron").read_text()
assert node["node_guid"] in installed and 'property: "control_rig_input.foot_left_location"' in installed
settings = json.loads((directory / "../d147/settings.native.json").read_text())
foot = next(prop for prop in settings["asset_properties"] if prop["name"] == "FootLock")
assert any(field["name"] == "bAllowFootLock" and field["exported_text"] == "True" for field in foot["fields"])

cases = []
for history in (0.0, 0.8):
    for requested in (0.0, 0.5, 1.0):
        source_amount = requested if requested == 1.0 else min(requested, history)
        reset_amount = 1.0 if requested == 1.0 else 0.0
        for frame, target in enumerate((0.0, 0.08), 1):
            cases.append({
                "history": history,
                "requested": requested,
                "frame": frame,
                "source_weight": source_amount,
                "weight_if_reset": reset_amount,
                "source_rig_position_m": target * (1.0 - source_amount),
                "rig_position_if_reset_m": target * (1.0 - reset_amount),
            })
failures = [case for case in cases if case["source_weight"] != case["weight_if_reset"]]
assert len(failures) == 2
assert all(case["history"] == 0.8 and case["requested"] == 0.5 for case in failures)
assert failures[1]["rig_position_if_reset_m"] - failures[1]["source_rig_position_m"] == 0.04
print(json.dumps({"control_rig": node["node_guid"], "cases": cases, "reset_failures": len(failures)}, indent=2))
