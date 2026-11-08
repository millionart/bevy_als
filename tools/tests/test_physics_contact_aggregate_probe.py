import json
import math
import sys
from pathlib import Path

path = Path(sys.argv[1])
read = lambda file: json.loads(file.read_text(encoding="utf-8-sig"))
assert read(path.with_name(path.name + ".reader.report.json"))["success"]
samples = read(path)["physics_contact_probe"]["samples"]
counts = set()
for sample in samples:
    expected = sample["input"]["expected_contact_shapes"]
    contacts = sample["aggregate_contacts"]
    actual = [contact["mesh_shape_index"] for contact in contacts]
    assert sorted(actual) == sorted(expected) and len(actual) == len(set(actual))
    assert sample["aggregate_shape_count"] == sample["other_geometry"]["shape_count"]
    assert sample["smooth_edge_collisions"] is False
    counts.add(len(contacts))
    for contact in contacts:
        assert not contact["disabled"] and contact["points"]
        assert math.isfinite(contact["cull_distance_cm"]) and contact["cull_distance_cm"] > 0
        assert 0 <= contact["mesh_shape_index"] < sample["aggregate_shape_count"]
        for point in contact["points"]:
            assert math.isfinite(point["phi_cm"])
            normal = point["world_normal"]
            assert abs(sum(normal[axis] ** 2 for axis in "xyz") - 1) < 1e-5
            separation = sum((point["world_point_0_cm"][axis] - point["world_point_1_cm"][axis]) * normal[axis] for axis in "xyz")
            assert abs(separation - point["phi_cm"]) < 1e-3
        if contact["mesh_shape_index"] == sample["other_geometry"]["shape_index"]:
            assert len(contact["points"]) == len(sample["points"])
            for actual_point, direct_point in zip(contact["points"], sample["points"]):
                assert abs(actual_point["phi_cm"] - direct_point["phi_cm"]) < 1e-4
                for field in ["world_point_0_cm", "world_point_1_cm", "world_normal"]:
                    limit = 1e-6 if field == "world_normal" else 1e-4
                    assert all(abs(actual_point[field][axis] - direct_point[field][axis]) < limit for axis in "xyz")
assert {0, 1, 2}.issubset(counts)
print("physics contact aggregate probe PASS: separated, single and multiple hulls")
