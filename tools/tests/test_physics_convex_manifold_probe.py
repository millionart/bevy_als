import json
from pathlib import Path
import sys

path = Path(sys.argv[1])
watchdog = json.loads(path.with_name(path.name + ".reader.report.json").read_text(encoding="utf-8-sig"))
assert watchdog["success"], "Native reader did not complete successfully"
report = json.loads(path.read_text(encoding="utf-8-sig"))
samples = report["physics_contact_probe"]["samples"]
assert {sample["input"]["label"] for sample in samples} == {"bench_warm_separated", "bench_warm_rotated"}
for sample in samples:
    assert not sample["restored"]
    assert len(sample["next_points"]) == 4
    for key in ("cold_gjk", "warm_gjk"):
        gjk = sample[key]
        assert gjk["success"]
        assert 0 <= gjk["vertex_0"] < 8
        assert 0 <= gjk["vertex_1"] < len(sample["other_geometry"]["convex_core_vertices_cm"])
        assert 1 <= len(gjk["simplex"]) <= 4
print("physics convex manifold probe PASS")
