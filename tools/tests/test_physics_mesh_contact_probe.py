import json
import math
import sys
from pathlib import Path

path = Path(sys.argv[1])
report = json.loads(path.with_name(path.name + ".reader.report.json").read_text(encoding="utf-8-sig"))
assert report["success"], "Native reader did not complete successfully"
samples = json.loads(path.read_text(encoding="utf-8-sig"))["physics_contact_probe"]["samples"]
expected = {(mesh, bone, gap) for mesh in ["SM_Als_Barrel", "SM_Als_Rock", "SM_Als_Stairs_01"] for bone in ["head", "foot_l"] for gap in [-0.1, 30]}
assert len(samples) == len(expected)
assert {(s["input"]["mesh_asset_path"].rsplit("/", 1)[1], s["input"]["bone_name"], s["input"]["placement_gap_cm"]) for s in samples} == expected
for sample in samples:
    source = sample["input"]
    geometry = sample["other_geometry"]
    assert geometry["simulation_enabled"] and geometry["query_enabled"]
    assert geometry["kind"] == ("convex" if "Barrel" in source["mesh_asset_path"] else "trimesh")
    assert (geometry["margin_cm"] > 0) == (geometry["kind"] == "convex")
    assert bool(sample["points"]) == (source["placement_gap_cm"] < 0), source["label"]
    for point in sample["points"]:
        normal = point["world_normal"]
        assert abs(sum(normal[axis] ** 2 for axis in "xyz") - 1) < 1e-5
        projection = sum((point["world_point_0_cm"][axis] - point["world_point_1_cm"][axis]) * normal[axis] for axis in "xyz")
        assert math.isfinite(point["phi_cm"]) and abs(projection - point["phi_cm"]) < 1e-3
print("physics mesh contact probe PASS")
