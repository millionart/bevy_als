import json
import math
from pathlib import Path
import sys

path = Path(sys.argv[1])
watchdog = json.loads(path.with_name(path.name + ".reader.report.json").read_text(encoding="utf-8-sig"))
assert watchdog["success"], "Native reader did not complete successfully"
report = json.loads(path.read_text(encoding="utf-8-sig"))
settings = report["physics_asset"]["constraint_console_variables"]
inverse = float(settings["p.Chaos.Collision.CullDistanceReferenceSize"])
minimum = float(settings["p.Chaos.Collision.MinCullDistanceScale"])
assert settings["p.Chaos.Collision.EnableBoundsChecks"] in ("true", "false")
expected = {"bench_aggregate_within_cull": [1, 2], "bench_aggregate_outside_cull": [1]}
for sample in report["physics_contact_probe"]["samples"]:
    contacts = sample["aggregate_contacts"]
    assert [contact["mesh_shape_index"] for contact in contacts] == expected[sample["input"]["label"]]
    shape = sample["geometry"]
    extent = max(shape["maximum_cm"][axis] - shape["minimum_cm"][axis] for axis in "xyz")
    cull = sample["input"]["cull_distance_cm"] * max(extent * inverse, 0, minimum)
    for contact in contacts:
        assert not contact["disabled"]
        assert math.isclose(contact["cull_distance_cm"], cull, abs_tol=1e-5)
    if sample["input"]["label"] == "bench_aggregate_within_cull":
        assert math.isclose(min(point["phi_cm"] for point in contacts[1]["points"]), 2.4, abs_tol=1e-4)
print("physics convex candidate probe PASS")
