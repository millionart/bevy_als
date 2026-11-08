import json
import math
from pathlib import Path
import sys

path = Path(sys.argv[1])
watchdog = json.loads(path.with_name(path.name + ".reader.report.json").read_text(encoding="utf-8-sig"))
assert watchdog["success"], "Native reader did not complete successfully"
report = json.loads(path.read_text(encoding="utf-8-sig"))
settings = report["physics_asset"]["constraint_console_variables"]
assert math.isfinite(float(settings["p.Chaos.Collision.Manifold.PlaneContactNormalEpsilon"]))
assert settings["p.Chaos.Collision.Manifold.ForceOneShotManifoldEdgeEdgeCaseZeroCullDistance"] in ("true", "false")
expected = {"SM_Als_Bench_shape_1": 1, "SM_Als_Bench_shape_2": 3,
            "SM_Als_Arch_02_shape_0_foot_l_scale_1_gap_-0.1_against_2": 2}
samples = report["physics_contact_probe"]["samples"]
assert {sample["input"]["label"] for sample in samples} == set(expected)
for sample in samples:
    assert {point["contact_type"] for point in sample["points"]} == {expected[sample["input"]["label"]]}
    gjk = sample["cold_gjk"]
    assert gjk["success"]
    assert 0 <= gjk["vertex_0"] < 8
    assert 0 <= gjk["vertex_1"] < len(sample["other_geometry"]["convex_core_vertices_cm"])
    assert math.isclose(gjk["support_delta_cm"], (math.sqrt(3) - 1) * sample["collision_margin_0_cm"], abs_tol=1e-7)
print("physics face classification probe PASS")
