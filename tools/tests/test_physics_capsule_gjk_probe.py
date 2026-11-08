import json
import math
import sys
from pathlib import Path

path = Path(sys.argv[1])
report = json.loads(path.with_name(path.name + ".reader.report.json").read_text(encoding="utf-8-sig"))
assert report["success"], "Native reader did not complete successfully"
samples = json.loads(path.read_text(encoding="utf-8-sig"))["physics_contact_probe"]["samples"]
assert {sample["input"]["label"] for sample in samples} == {
    "arch_head_cylinder", "arch_head_endcap", "arch_head_raised", "arch_head_separated",
}
for sample in samples:
    geometry = sample["geometry"]
    axis = geometry["axis"]
    assert math.isclose(sum(axis[key] ** 2 for key in "xyz"), 1.0, abs_tol=1e-6)
    assert geometry["height_cm"] > 0.0
    result = sample["capsule_gjk"]
    assert result["success"]
    assert 0 <= result["vertex_0"] < len(sample["other_geometry"]["convex_core_vertices_cm"])
    assert result["vertex_1"] in (0, 1)
    assert result["support_delta_cm"] == 0.0
    for key in ("convex_point_cm", "segment_point_cm", "normal"):
        assert all(math.isfinite(result[key][axis]) for axis in "xyz")
    assert math.isclose(sum(result["normal"][axis] ** 2 for axis in "xyz"), 1.0, abs_tol=1e-5)
    if sample["input"]["label"] == "arch_head_separated":
        assert not sample["points"]
        assert -result["penetration_cm"] - geometry["radius_cm"] > sample["input"]["cull_distance_cm"]
    if sample["input"]["label"] == "arch_head_endcap":
        point = sample["points"][-1]
        radius = geometry["radius_cm"]
        body = dict(zip("xyz", sample["input"]["body_translation_cm"]))
        for axis in "xyz":
            assert math.isclose(result["normal"][axis], point["world_normal"][axis], abs_tol=1e-5)
            assert math.isclose(result["convex_point_cm"][axis], point["shape_point_1_cm"][axis], abs_tol=1e-4)
            expected = body[axis] + point["shape_point_0_cm"][axis] + radius * result["normal"][axis]
            assert math.isclose(result["segment_point_cm"][axis], expected, abs_tol=1e-4)
        assert math.isclose(result["penetration_cm"], -point["phi_cm"] - radius, abs_tol=1e-4)
print("physics capsule GJK probe PASS")
