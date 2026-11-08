import json
import math
from pathlib import Path
import sys

path = Path(sys.argv[1])
watchdog = json.loads(path.with_name(path.name + ".reader.report.json").read_text(encoding="utf-8-sig"))
assert watchdog["success"], "Native reader did not complete successfully"
report = json.loads(path.read_text(encoding="utf-8-sig"))
samples = report["physics_contact_probe"]["samples"]
assert {sample["input"]["label"] for sample in samples} == {"bench_velocity_cull_rest", "bench_velocity_cull_moving"}
for sample in samples:
    moving = sample["input"]["label"] == "bench_velocity_cull_moving"
    contacts = sample["aggregate_contacts"]
    assert [contact["mesh_shape_index"] for contact in contacts] == ([1, 2] if moving else [1])
    expected = 3.0 + (100.0 / 60.0 if moving else 0.0)
    assert all(math.isclose(contact["cull_distance_cm"], expected, abs_tol=1e-5) for contact in contacts)
print("physics convex velocity cull probe PASS")
