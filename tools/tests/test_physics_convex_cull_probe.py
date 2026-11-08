import json
import math
from pathlib import Path
import sys

path = Path(sys.argv[1])
watchdog = json.loads(path.with_name(path.name + ".reader.report.json").read_text(encoding="utf-8-sig"))
assert watchdog["success"], "Native reader did not complete successfully"
report = json.loads(path.read_text(encoding="utf-8-sig"))
samples = report["physics_contact_probe"]["samples"]
assert {sample["input"]["label"] for sample in samples} == {"bench_cull_inside", "bench_cull_outside"}
for sample in samples:
    gjk = sample["cold_gjk"]
    assert gjk["success"]
    expected_delta = (math.sqrt(3) - 1) * sample["collision_margin_0_cm"]
    assert math.isclose(gjk["support_delta_cm"], expected_delta, abs_tol=1e-7)
    culled = -gjk["penetration_cm"] > sample["input"]["cull_distance_cm"] + gjk["support_delta_cm"]
    assert culled == (not sample["points"])
print("physics convex cull probe PASS")
