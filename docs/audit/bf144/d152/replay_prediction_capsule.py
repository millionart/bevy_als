import gzip
import hashlib
import json
from pathlib import Path


directory = Path(__file__).resolve().parent
root = directory.parents[3]
raw = gzip.decompress((directory / "edge.native.json.gz").read_bytes())
assert hashlib.sha256(raw).hexdigest() == (directory / "edge.native.sha256").read_text().split()[0]
native = json.loads(raw)
subobjects = {obj["object_path"].rsplit(":", 1)[-1]: obj for obj in native["blueprint_cdo_subobjects"]}
capsule = {prop["name"]: prop["exported_text"] for prop in subobjects["CollisionCylinder"]["properties"]}
movement = {prop["name"]: prop["exported_text"] for prop in subobjects["CharMoveComp"]["properties"]}
radius = float(capsule["CapsuleRadius"])
half_height = float(capsule["CapsuleHalfHeight"])
floor_cosine = float(movement["WalkableFloorZ"])
assert (radius, half_height, floor_cosine) == (30.0, 90.0, 0.71)
samples = {sample["input"]["name"]: sample for sample in native["geometry_query_probe"]["samples"]}

with gzip.open(directory / "../d149/locomotion.native.json.gz", "rt") as stream:
    locomotion = json.load(stream)
binding, = [prop for obj in locomotion["asset_objects"]
            if ":AnimGraph." in obj["object_path"] and ".Fall.AnimGraphNode_TwoWayBlend_0." in obj["object_path"]
            for prop in obj["properties"] if prop["name"] == "PropertyBindings"]
assert 'PropertyPath=("GetParent","InAirState","GroundPredictionAmount")' in binding["exported_text"]
fall = (root / "assets/ALS/Character/AnimationInstances/AB_Als_Locomotion/AnimGraph/SM9_Locomotion_States/S1_Fall.animgraph.ron").read_text()
assert 'alpha_key: "in_air_state.ground_prediction_amount"' in fall

curve = json.loads((root / "assets/ALS/Data/AnimationInstance/Air/als_in_air_settings.json").read_text())["ground_prediction"]
assert curve["times"] == [0.0, 1.0] and curve["values"] == [1.0, 0.0]
assert curve["interps"][0] == "Cubic"
results = []
for name, sample in samples.items():
    sweep = sample["input"]["capsule_sweep"]
    assert sweep["radius_cm"] == (radius * 2 if name.startswith("physical") else radius)
    assert sweep["half_height_cm"] == half_height
    hit = sample["capsule_sweep"]
    ground_valid = hit["hit"] and hit["normal"]["z"] >= floor_cosine
    alpha = 0.0
    if ground_valid:
        t = hit["distance_cm"] / sweep["length_cm"]
        alpha = (2.0 * t**3 - 3.0 * t**2 + 1.0
                 + (t**3 - 2.0 * t**2 + t) * curve["leave_tangents"][0]
                 + (t**3 - t**2) * curve["arrive_tangents"][1])
    results.append({"case": name, "valid_ground": ground_valid, "fall_input_alpha": alpha})
assert [row["case"] for row in results if row["fall_input_alpha"] == 0.0] == ["query_edge"]
assert results[0]["fall_input_alpha"] == results[2]["fall_input_alpha"]
print(json.dumps({"capsule_radius_cm": radius, "capsule_half_height_cm": half_height, "cases": results}, indent=2))
