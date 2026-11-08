import json
import math
import sys
from pathlib import Path

path = Path(sys.argv[1])
report = json.loads(path.with_name(path.name + ".reader.report.json").read_text(encoding="utf-8-sig"))
assert report["success"], "Native reader did not complete successfully"
data = json.loads(path.read_text(encoding="utf-8-sig"))
samples = data["physics_contact_probe"]["samples"]
assert {sample["input"]["label"] for sample in samples} == {
    "bench_foot_penetrating", "bench_foot_near", "bench_foot_separated",
    "bench_foot_cull_edge", "bench_foot_core_overlap",
}
for sample in samples:
    result = sample["cold_gjk"]
    assert result["success"]
    assert 0 <= result["vertex_0"] < 8
    assert 0 <= result["vertex_1"] < len(sample["other_geometry"]["convex_core_vertices_cm"])
    assert 1 <= len(result["simplex"]) <= 4
    assert len(result["simplex"]) == len(sample["gjk_simplex"])
    for actual, expected in zip(result["simplex"], sample["gjk_simplex"]):
        assert math.isclose(actual["weight"], expected["weight"], abs_tol=1e-8)
        for key in ["shape_point_0_cm", "shape_point_1_cm"]:
            assert all(math.isclose(actual[key][axis], expected[key][axis], abs_tol=1e-6) for axis in "xyz")
    if sample["points"]:
        normal = sample["points"][0]["world_normal"]
        assert all(math.isclose(result["shape_normal_1"][axis], -normal[axis], abs_tol=1e-5) for axis in "xyz")
print("physics cold GJK probe PASS")
