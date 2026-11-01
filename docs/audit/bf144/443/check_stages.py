import copy
import json
import math
import sys
from pathlib import Path

r = Path(__file__).resolve().parent


def validate(data):
    rows = data["rows"]
    motors = data["after_motors"]
    posts = data["camera_after_post_update"]
    assert len(rows) == len(motors) == len(posts) == 240
    for i, (row, motor, post) in enumerate(zip(rows, motors, posts)):
        assert row["ordinal"] == motor["ordinal"] == post["ordinal"] == i
        assert all(motor["position"][axis] == row["position"][axis] for axis in (0, 2))
        for obj, keys in ((row, ("position", "physical_velocity", "planned_delta", "planned_velocity")), (motor, ("position", "velocity")), (post, ("position", "velocity", "gravity", "center_of_mass"))):
            for key in keys:
                assert len(obj[key]) == 3 and all(type(v) in (int, float) and math.isfinite(v) for v in obj[key])
        for key in ("frame_delta_ns", "physics_delta_ns", "substep_delta_ns", "substeps"):
            assert type(post[key]) is int and post[key] > 0
        assert row["desired_motion"] == row["expected_direction"]
        if i:
            assert row["position"] == posts[i-1]["position"]
    return dict(rows=240, stages=["after dynamics before motors", "after motors", "Last after physics"], frame_durations_ns=sorted(set(x["frame_delta_ns"] for x in posts)), physics_durations_ns=sorted(set(x["physics_delta_ns"] for x in posts)), substep_counts=sorted(set(x["substeps"] for x in posts)))


if __name__ == "__main__":
    label = sys.argv[1]
    if label == "self-test":
        data = dict(rows=[], after_motors=[], camera_after_post_update=[])
        for i in range(240):
            data["rows"].append(dict(ordinal=i, position=[0,0,0], physical_velocity=[0,0,0], planned_delta=[0,0,0], planned_velocity=[0,0,0], desired_motion=[0,0,0], expected_direction=[0,0,0]))
            data["after_motors"].append(dict(ordinal=i, position=[0,0,0], velocity=[0,0,0]))
            data["camera_after_post_update"].append(dict(ordinal=i, position=[0,0,0], velocity=[0,0,0], gravity=[0,-9.81,0], center_of_mass=[0,0,0], frame_delta_ns=33333335, physics_delta_ns=33333333, substep_delta_ns=5555555, substeps=6))
        validate(data)
        for kind in range(4):
            bad = copy.deepcopy(data)
            if kind == 0:bad["after_motors"] = []
            elif kind == 1:del bad["camera_after_post_update"][0]["frame_delta_ns"]
            elif kind == 2:bad["camera_after_post_update"][1]["ordinal"] = 0
            else:bad["camera_after_post_update"][1]["velocity"][0] = float("nan")
            try:validate(bad)
            except (AssertionError, KeyError):continue
            raise AssertionError(kind)
        result = dict(rejected=4)
    else:
        result = validate(json.loads((r / ("runtime-" + label) / "parameters.json").read_text()))
        result["gate"] = "PASS_ACTUAL_THREE_STAGE_FIELDS_AND_CLOCKS240"
    (r / (label + "-stages.json")).write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result), flush=True)
