import json
import math
import sys
from pathlib import Path

path = Path(sys.argv[1])
read = lambda file: json.loads(file.read_text(encoding="utf-8-sig"))
assert read(path.with_name(path.name + ".reader.report.json"))["success"]
samples = read(path)["physics_contact_probe"]["samples"]
types = set()
for sample in samples:
    constraints = [sample, *sample.get("aggregate_contacts", [])]
    for constraint in constraints:
        for index in [0, 1]:
            margin = constraint[f"collision_margin_{index}_cm"]
            assert math.isfinite(margin) and margin >= 0
        for point in constraint["points"]:
            assert isinstance(point["contact_type"], int) and point["contact_type"] in range(5)
            types.add(point["contact_type"])
    assert {point["contact_type"] for point in sample["points"]} == set(sample["input"]["expected_contact_types"])
    if sample["geometry"]["kind"] == "capsule":
        assert sample["collision_margin_0_cm"] == sample["geometry"]["radius_cm"]
    else:
        assert math.isclose(sample["collision_margin_0_cm"], 0.6, abs_tol=1e-6)
    assert sample["collision_margin_1_cm"] == 0
assert {1, 2, 3}.issubset(types)
print("physics contact classification probe PASS: contact types and effective margins")
