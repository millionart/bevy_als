import json
import math
import sys
from pathlib import Path

path = Path(sys.argv[1])
report = json.loads(path.with_name(path.name + ".reader.report.json").read_text(encoding="utf-8-sig"))
assert report["success"], "Native reader did not complete successfully"
samples = json.loads(path.read_text(encoding="utf-8-sig"))["physics_contact_probe"]["samples"]
assert {sample["input"]["label"] for sample in samples} == {
    "bench_warm_same", "bench_warm_small", "bench_warm_separated", "bench_warm_overlap", "bench_warm_rotated",
}
for sample in samples:
    for key in ["cold_gjk", "warm_gjk"]:
        result = sample[key]
        assert result["success"] and 0 <= result["vertex_0"] < 8
        assert 0 <= result["vertex_1"] < len(sample["other_geometry"]["convex_core_vertices_cm"])
        assert 1 <= len(result["simplex"]) <= 4
        assert math.isclose(sum(point["weight"] for point in result["simplex"]), 1.0, abs_tol=1e-8)
    if not sample["restored"]:
        actual = sample["warm_gjk"]["simplex"]
        expected = sample["next_gjk_simplex"]
        assert len(actual) == len(expected)
        for first, second in zip(actual, expected):
            assert math.isclose(first["weight"], second["weight"], abs_tol=1e-8)
            for key in ["shape_point_0_cm", "shape_point_1_cm"]:
                assert all(math.isclose(first[key][axis], second[key][axis], abs_tol=1e-6) for axis in "xyz")
print("physics warm GJK probe PASS")
