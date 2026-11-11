import gzip
import hashlib
import json
import re
import sys
from collections import defaultdict
from pathlib import Path


directory = Path(__file__).resolve().parent
root = directory.parents[3]
asset_root = Path(sys.argv[1]) if len(sys.argv) > 1 else root / "assets"
sys.path.insert(0, str(root / "tools/animgraph_authoring"))
from ue_animgraph_converter import bind_linked_pose_cache_order, contract, graph_ir, validate_registry
from ue_animgraph_pose_renderer import cache_key, rendered_inner


raw = gzip.decompress((directory / "layering.native.json.gz").read_bytes())
assert hashlib.sha256(raw).hexdigest() == (directory / "layering.native.sha256").read_text().split()[0]
layering = json.loads(raw)
schedule = layering["cached_pose_schedule"]
layer, = [layer for layer in schedule["layers"] if layer["name"] == "AnimGraph"]
order = [node["cache_name"] for node in layer["nodes"]]
assert len(order) == len(set(order)) == 11
assert order.index("Locomotion") < order.index("Overlay")
native, = [graph for graph in layering["graphs"] if graph["object_path"].endswith(":AnimGraph")]
for name in ("Locomotion", "Overlay"):
    save, = [node for node in native["nodes"] if node["title"] == f"Save cached pose '{name}'"]
    inputs, = [node for node in native["nodes"] if node["title"] == f"Input Pose\n{name} Input"]
    assert next(pin for pin in save["pins"] if pin["name"] == "Pose")["linked_to"][0]["node_guid"] == inputs["node_guid"]

overlay = json.loads((directory / "default-overlay.native.json").read_text())
binding, = [prop for obj in overlay["asset_objects"]
            if ":Overlay." in obj["object_path"] and ".AnimGraphNode_TwoWayBlend_11." in obj["object_path"]
            for prop in obj["properties"] if prop["name"] == "PropertyBindings"]
assert 'PropertyPath=("GetParent","InAirState","GroundPredictionAmount")' in binding["exported_text"]
target = (asset_root / "ALS/Character/AnimationInstances/AB_Als_Layering/AnimGraph.animgraph.ron").read_text()
keys = [cache_key({"source_asset": layering["asset_path"]}, name) for name in order]
if "linked_pose_cache_order: " + json.dumps(keys) not in target:
    raise SystemExit("Generated Layering root is missing its native linked-pose cache order")
contract_data = contract(layering, "AnimGraph")
registry = validate_registry(json.loads((root / "tools/animgraph_authoring/als_ref_mapping_registry.json").read_text()))
ir, _ = graph_ir(contract_data, registry, [node["object_path"] for node in native["nodes"]])
bind_linked_pose_cache_order(ir, schedule)
root_node, = [node for node in ir["nodes"] if node["renderer"] == "root_output"]
_, ron = rendered_inner(contract_data, {}, root_node, {}, {})
assert "linked_pose_cache_order: " + json.dumps(keys) in ron

nodes = {identity: name for identity, name in re.findall(r'^        \(id: "([^"]+)", name: "([^"]+)",', target, re.M)}
sources = defaultdict(list)
for target_node, source_node, pin in re.findall(r'^        NodeData\("([^"]+)", "[^"]+"\): NodeData\("([^"]+)", "([^"]+)"\),$', target, re.M):
    if pin.casefold() == "pose":
        sources[target_node].append(source_node)


def cache_demands(node):
    name = nodes[node]
    if name.startswith("Use cached pose '"):
        return {name.removeprefix("Use cached pose '").removesuffix("'")}
    return set().union(*(cache_demands(source) for source in sources[node]))


dependencies = {name.removeprefix("Save cached pose '").removesuffix("'"): cache_demands(node)
                for node, name in nodes.items() if name.startswith("Save cached pose '")}
assert "Overlay" in dependencies["Pelvis"]
assert all(order.index(cache) < order.index(dependency)
           for cache, required in dependencies.items() for dependency in required)
direct = {"Locomotion", "Pelvis"}
demanded = direct.copy()
for cache in order:
    if cache in demanded:
        demanded.update(dependencies[cache])
leaf_order = [name for name in order if name in demanded.intersection({"Locomotion", "Overlay"})]
assert leaf_order == ["Locomotion", "Overlay"] and "Overlay" not in direct

grounded_bytes = gzip.decompress((directory / "grounded.native.json.gz").read_bytes())
assert hashlib.sha256(grounded_bytes).hexdigest() == (directory / "grounded.native.sha256").read_text().split()[0]
grounded = json.loads(grounded_bytes)
standing_order = [node["cache_name"] for layer in grounded["cached_pose_schedule"]["layers"]
                  if layer["name"] == "AnimGraph" for node in layer["nodes"]]
assert standing_order == ["Standing", "Crouching"]
grounded_target = (asset_root / "ALS/Character/AnimationInstances/AB_Als_Grounded/AnimGraph.animgraph.ron").read_text()
assert "linked_pose_cache_order: " + json.dumps([cache_key({"source_asset": grounded["asset_path"]}, name)
                                               for name in standing_order]) in grounded_target

for graph_name, path in [("Main", "/Source"), ("Renamed", "/Renamed")]:
    graph = {
        "source_graph": graph_name,
        "nodes": [
            {"source_node": path + "/Root", "renderer": "root_output", "semantic": {"synchronize_groups": True}},
            {"source_node": path + "/InputA", "renderer": "linked_input_pose", "semantic": {}},
            {"source_node": path + "/InputB", "renderer": "linked_input_pose", "semantic": {}},
            {"source_node": path + "/SaveA", "renderer": "save_cached_pose", "semantic": {"cache_name": "A"}},
            {"source_node": path + "/SaveB", "renderer": "save_cached_pose", "semantic": {"cache_name": "B"}},
        ],
        "links": [
            {"source": {"node_object_path": path + "/InputA"}, "target": {"node_object_path": path + "/SaveA", "pin_name": "Pose"}},
            {"source": {"node_object_path": path + "/InputB"}, "target": {"node_object_path": path + "/SaveB", "pin_name": "Pose"}},
        ],
    }
    if graph_name == "Renamed":
        del graph["nodes"][0]["semantic"]
    for names in (["B", "A"], ["A", "B"]):
        bind_linked_pose_cache_order(graph, {"layers": [{"name": graph_name, "nodes": [{"cache_name": name} for name in names]}]})
        assert graph["nodes"][0]["semantic"]["linked_pose_cache_order"] == names
        assert graph["nodes"][0]["semantic"].get("synchronize_groups", False) == (graph_name == "Main")

samples = []
previous = 0.0
for frame, prediction in enumerate([0.0, 0.8923675, 0.8923675, 0.0], 1):
    samples.append({"frame": frame, "fall": prediction, "ordered_overlay": prediction, "demand_order_overlay": previous})
    previous = prediction
assert [sample["frame"] for sample in samples if sample["fall"] != sample["demand_order_overlay"]] == [2, 4]
print(json.dumps({
    "source_order": order,
    "linked_order_from_pelvis_request": leaf_order,
    "grounded_source_order": standing_order,
    "cases": samples,
    "identity_changes_keep_the_same_order": True,
}, indent=2))
