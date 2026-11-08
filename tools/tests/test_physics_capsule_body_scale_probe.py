import json
import math
import sys
from pathlib import Path

path = Path(sys.argv[1])
report = json.loads(path.with_name(path.name + ".reader.report.json").read_text(encoding="utf-8-sig"))
assert report["success"], "Native reader did not complete successfully"
samples = json.loads(path.read_text(encoding="utf-8-sig"))["physics_contact_probe"]["samples"]
assert {sample["input"]["label"] for sample in samples} == {
    "head_unit", "head_uniform", "head_nonuniform", "head_reflected",
}
geometries = {}
for sample in samples:
    geometry = sample["geometry"]
    assert geometry["kind"] == "capsule"
    assert geometry["height_cm"] > 0.0 and geometry["radius_cm"] > 0.0
    assert math.isclose(sum(geometry["axis"][axis] ** 2 for axis in "xyz"), 1.0, abs_tol=1e-6)
    for axis in "xyz":
        endpoint = geometry["endpoint_0_cm"][axis] + geometry["height_cm"] * geometry["axis"][axis]
        assert math.isclose(endpoint, geometry["endpoint_1_cm"][axis], abs_tol=1e-4)
    geometries[sample["input"]["label"]] = geometry
assert geometries["head_uniform"]["radius_cm"] != geometries["head_unit"]["radius_cm"]
assert geometries["head_nonuniform"]["height_cm"] != geometries["head_unit"]["height_cm"]
print("physics capsule body scale probe PASS")
