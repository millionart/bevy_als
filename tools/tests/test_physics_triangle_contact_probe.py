import json
import math
import sys
from pathlib import Path

path = Path(sys.argv[1])
report = json.loads(path.with_name(path.name + ".reader.report.json").read_text(encoding="utf-8-sig"))
assert report["success"], "Native reader did not complete successfully"
data = json.loads(path.read_text(encoding="utf-8-sig"))
assert data["physics_asset"]["constraint_console_variables"]["p.Chaos.Collision.UseCapsuleTriMesh2"].lower() in ["1", "true"]
samples = data["physics_contact_probe"]["samples"]
assert samples
for sample in samples:
    assert sample["geometry"]["kind"] == "capsule"
    assert sample["other_geometry"]["kind"] == "trimesh"
    groups = sample["triangle_contacts"]
    assert [group["triangle_index"] for group in groups] == sample["input"]["triangle_indices"]
    assert groups and any(group["points"] for group in groups)
    for group in groups:
        assert 0 <= group["triangle_index"] < len(sample["other_geometry"]["trimesh_core_triangles"])
        for point in group["points"]:
            assert point["face_index"] == -1
            normal = point["world_normal"]
            assert abs(sum(normal[axis] ** 2 for axis in "xyz") - 1) < 1e-5
            projection = sum((point["world_point_0_cm"][axis] - point["world_point_1_cm"][axis]) * normal[axis] for axis in "xyz")
            assert math.isfinite(point["phi_cm"]) and abs(projection - point["phi_cm"]) < 1e-5
    for point in sample["points"]:
        group = next(group for group in groups if group["triangle_index"] == point["face_index"])
        assert any(max(abs(raw[field][axis] - point[field][axis]) for field in ["world_point_0_cm", "world_point_1_cm", "world_normal"] for axis in "xyz") < 1e-3 for raw in group["points"]), sample["input"]["label"]
print("physics triangle contact probe PASS")
