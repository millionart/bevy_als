import json
import math
import sys
from pathlib import Path

samples=json.loads(Path(sys.argv[1]).read_text(encoding="utf-8-sig"))["physics_contact_probe"]["samples"]
expected={
    "foot_l_tilt_45_width_5.0_shift_0.0":(True,True),
    "foot_l_tilt_45_width_5.0_shift_0.2":(True,True),
    "foot_l_tilt_45_width_5.0_shift_-0.2":(True,True),
    "foot_l_tilt_45_width_5.0_shift_0.6":(True,False),
    "foot_l_tilt_45_width_500.0_shift_0.2":(True,True),
    "hand_l_width_5.0_shift_0.2":(True,True),
    "head_width_500.0_shift_0.2":(False,False),
    "root_width_500.0_shift_0.2":(False,False),
}
assert {sample["input"]["label"] for sample in samples}==set(expected)
for sample in samples:
    assert (sample["restore_allowed"],sample["restored"])==expected[sample["input"]["label"]]
    if sample["restored"]:
        assert len(sample["points"])==len(sample["next_points"])
        delta=[new-old for new,old in zip(sample["input"]["next"]["body_translation_cm"],sample["input"]["body_translation_cm"])]
        for old,new in zip(sample["points"],sample["next_points"]):
            assert old["shape_point_0_cm"]==new["shape_point_0_cm"]
            assert all(abs(new["world_point_0_cm"][axis]-old["world_point_0_cm"][axis]-delta[index])<1e-5 for index,axis in enumerate("xyz"))
    else:
        assert sample["next_points"]==sample["fresh_points"]
    for kind in ["points","next_points","fresh_points"]:
        for point in sample[kind]:
            normal=point["world_normal"]
            assert abs(sum(normal[axis]**2 for axis in "xyz")-1)<1e-5
            projection=sum((point["world_point_0_cm"][axis]-point["world_point_1_cm"][axis])*normal[axis] for axis in "xyz")
            assert math.isfinite(point["phi_cm"]) and abs(projection-point["phi_cm"])<1e-4
print("physics contact restore PASS")
