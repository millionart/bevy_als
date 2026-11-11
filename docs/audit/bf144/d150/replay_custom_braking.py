import gzip
import hashlib
import json
import math
from pathlib import Path


directory = Path(__file__).resolve().parent
root = directory.parents[3]
montage = json.loads((directory / "mantle.native.json").read_text())
properties = {prop["name"]: prop["exported_text"] for prop in montage["asset_properties"]}
assert properties["BlendIn"] == "(BlendTime=0.200000,BlendOption=HermiteCubic)"
assert 'SlotName="PostLocomotion"' in properties["SlotAnimTracks"]
frames = montage["montage_clock_probe"]["samples"][0]["frames"]
assert math.isclose(frames[1]["weight"], 0.07407408, abs_tol=1e-8)
assert all(0.0 < frame["weight"] < 1.0 and frame["valid"] for frame in frames)

standing_bytes = gzip.decompress((directory / "standing.native.json.gz").read_bytes())
assert hashlib.sha256(standing_bytes).hexdigest() == (directory / "standing.native.sha256").read_text().split()[0]
standing = json.loads(standing_bytes)
node, = [node for graph in standing["graphs"] if graph["object_path"].endswith(":AnimGraph")
         for node in graph["nodes"] if node["title"] == "BS_Als_Lean\nBlendspace Evaluator"]
binding, = [obj for obj in standing["asset_objects"] if obj["object_path"].startswith(node["object_path"] + ".")]
assert any('PropertyPath=("GetParent","LeanState","ForwardAmount")' in prop["exported_text"] for prop in binding["properties"])
installed = (root / "assets/ALS/Character/AnimationInstances/Stances/AB_Als_Standing/AnimGraph.animgraph.ron").read_text()
assert node["node_guid"] in installed and 'key: "lean_state.forward_amount"' in installed

settings = json.loads((directory / "../d147/settings.native.json").read_text())
general = next(prop for prop in settings["asset_properties"] if prop["name"] == "General")
half_life = float(next(prop["exported_text"] for prop in general["fields"] if prop["name"] == "LeanInterpolationHalfLife"))
assert half_life == 0.2
results = []
for frame, acceleration in zip(frames, [4.0, -4.0]):
    x = math.log(2.0) / (half_life + 1e-8) * frame["delta_seconds"]
    alpha = 1.0 - 1.0 / (1.0 + 1.00746054 * x + 0.45053901 * x * x + 0.25724632 * x * x * x)
    previous = 0.025
    for limit in (0.0, 12.5):
        maximum = 8.0 if acceleration >= 0.0 else limit
        target = acceleration / maximum if maximum else 0.0
        results.append({
            "source_pose_weight": 1.0 - frame["weight"],
            "acceleration_dot_velocity": acceleration,
            "published_braking_limit": limit,
            "target_lean": target,
            "forward_lean": previous + (target - previous) * alpha,
        })
assert results[0]["forward_lean"] == results[1]["forward_lean"]
assert results[2]["target_lean"] == 0.0
assert results[2]["forward_lean"] != results[3]["forward_lean"]
print(json.dumps({"standing_lean_node": node["node_guid"], "cases": results}, indent=2))
