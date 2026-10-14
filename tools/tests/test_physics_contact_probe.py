import json
import math
import sys
from pathlib import Path

samples=json.loads(Path(sys.argv[1]).read_text(encoding="utf-8-sig"))["physics_contact_probe"]["samples"]
expected={
    "head_0_-0.1_wide":(2,-0.1),
    "head_0_4_wide":(0,None),
    "root_0_-0.1_wide":(1,-0.1),
    "root_0_0.5_wide":(1,0.5),
    "root_0_4_wide":(0,None),
    "foot_l_0_-0.1_wide":(4,-0.1),
    "hand_l_0_-0.1_wide":(4,-0.1),
}
assert {sample["input"]["label"] for sample in samples}==set(expected)
for sample in samples:
    count,minimum=expected[sample["input"]["label"]]
    assert len(sample["points"])==count
    if minimum is not None:
        assert abs(min(point["phi_cm"] for point in sample["points"])-minimum)<1e-4
    for point in sample["points"]:
        normal=point["world_normal"]
        assert abs(sum(normal[axis]**2 for axis in "xyz")-1)<1e-5
        projection=sum((point["world_point_0_cm"][axis]-point["world_point_1_cm"][axis])*normal[axis] for axis in "xyz")
        assert math.isfinite(point["phi_cm"]) and abs(projection-point["phi_cm"])<1e-4
print("physics contact probe PASS")
