import json
import math
import sys
from pathlib import Path

path = Path(sys.argv[1])
reader = json.loads(path.with_name(path.name + ".reader.report.json").read_text(encoding="utf-8-sig"))
assert reader["success"], "Native reader did not complete successfully"
samples = json.loads(path.read_text(encoding="utf-8-sig"))["physics_contact_probe"]["samples"]
labels = {sample["input"]["label"]: sample for sample in samples}
assert set(labels) == {"crossing", "parallel", "clamp_start", "clamp_end", "first_point",
                       "second_point", "nearly_point", "skew"}
for sample in samples:
    result = sample["segment_nearest"]
    for key in ("first_parameter", "second_parameter"):
        assert math.isfinite(result[key]) and 0 <= result[key] <= 1
    for key in ("first_point_cm", "second_point_cm"):
        assert all(math.isfinite(result[key][axis]) for axis in "xyz")
assert labels["crossing"]["segment_nearest"]["first_parameter"] == 0.5
assert labels["crossing"]["segment_nearest"]["second_parameter"] == 0.5
assert labels["first_point"]["segment_nearest"]["second_parameter"] == 0
assert labels["second_point"]["segment_nearest"]["first_parameter"] == 0.75
print("physics nearest segment probe PASS")
