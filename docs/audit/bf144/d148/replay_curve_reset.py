import json
from pathlib import Path


directory = Path(__file__).resolve().parent
root = directory.parents[3]
report = json.loads((directory / "head.native.json").read_text())
node, = [node for graph in report["graphs"] for node in graph["nodes"] if node["title"] == "Apply Mesh Space Additive"]
binding, = [obj for obj in report["asset_objects"] if obj["object_path"].startswith(node["object_path"] + ".")]
properties = {prop["name"]: prop["exported_text"] for prop in binding["properties"]}
assert 'PropertyName="Alpha"' in properties["PropertyBindings"]
assert 'PropertyPath=("GetParent","ViewState","HeadBlendAmount")' in properties["PropertyBindings"]
graph = (root / "assets/ALS/Character/AnimationInstances/AB_Als_Head/AnimGraph.animgraph.ron").read_text()
assert f'id: "{node["node_guid"]}"' in graph and 'alpha_key: "view_state.head_blend_amount"' in graph

results = []
for action in ("overwrite", "remove_insert"):
    for pose, aiming in (("ordinary", 0.0), ("aiming", 0.994912)):
        old = {"PoseAiming": aiming, "FootLeftIk": 0.75}
        for policy, curves in (
            ("retain", old),
            ("reset_all", {}),
            ("reset_aiming_only", {"FootLeftIk": old["FootLeftIk"]}),
        ):
            alpha = (1.0 - curves.get("ViewBlock", 0.0)) * (1.0 - curves.get("PoseAiming", 0.0))
            results.append({
                "action": action,
                "pose": pose,
                "policy": policy,
                "head_alpha": alpha,
                "remaining_curves": sorted(curves),
            })
assert [row["action"] for row in results if row["head_alpha"] != 1.0] == ["overwrite", "remove_insert"]
assert all(not row["remaining_curves"] for row in results if row["policy"] == "reset_all")
assert all(row["remaining_curves"] == ["FootLeftIk"] for row in results if row["policy"] == "reset_aiming_only")
print(json.dumps({"source_additive_node": node["node_guid"], "cases": results}, indent=2))
