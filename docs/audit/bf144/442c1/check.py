import copy
import json
import sys
from pathlib import Path

r = Path(__file__).resolve().parent


def validate(data):
    rows = data["rows"]
    late = data["camera_after_post_update"]
    assert len(rows) == len(late) == 240
    missing = []
    for i, row in enumerate(rows):
        assert row["ordinal"] == late[i]["ordinal"] == i
        assert row["desired_motion"] == row["expected_direction"]
        camera = row["camera_at_update"]
        if row["player_view_yaw"] is None:
            missing.append(i)
            assert camera is None
            assert row["consumed_yaw"] == row["view_yaw"]
            assert row["desired_motion"] == row["raw_control_direction"]
        else:
            assert row["player_view_yaw"] == row["consumed_yaw"] == camera["yaw"]
        if i in (20, 21):
            assert not row["input_blocked"] and row["input_move"] == [0, 1]
            assert any(value != 0 for value in row["desired_motion"])
            assert late[i]["camera"] is None
    assert missing == [20, 21]
    cold = data["natural_cold"]
    assert cold
    for row in cold:
        if row["input"] == [0, 0] and row["desired"] is not None:
            assert row["desired"] == [0, 0, 0]
    return dict(ready_rows=240, controlled_fallback_ordinals=missing, natural_snapshots=len(cold),
        natural_without_published=sum(v["player_yaw"] is None for v in cold),
        natural_without_view_with_nonzero_input=sum(v["player_yaw"] is None and v["input"] is not None and any(v["input"]) for v in cold),
        missing_view_with_later_sampler_eligible_camera=sum(v["player_yaw"] is None and any(c["active"] and not c["added"] for c in v["cameras"]) for v in cold))


def self_test():
    data = dict(rows=[], camera_after_post_update=[], natural_cold=[dict(player_yaw=None, cameras=[dict(active=True, added=True, yaw=0)], input=[0, 0], desired=[0, 0, 0])])
    for i in range(240):
        off = i in (20, 21)
        data["rows"].append(dict(ordinal=i, desired_motion=[1, 0, 0], expected_direction=[1, 0, 0], raw_control_direction=[1, 0, 0], player_view_yaw=None if off else 0, camera_at_update=None if off else dict(yaw=0), consumed_yaw=0, view_yaw=0, input_move=[0, 1], input_blocked=False))
        data["camera_after_post_update"].append(dict(ordinal=i, camera=None if off else dict(yaw=0)))
    validate(data)
    for kind in range(5):
        broken = copy.deepcopy(data)
        if kind == 0:
            broken["rows"][20]["player_view_yaw"] = 0
        elif kind == 1:
            broken["rows"][20]["consumed_yaw"] = 1
        elif kind == 2:
            broken["rows"][20]["desired_motion"] = [0, 0, 1]
        elif kind == 3:
            broken["rows"][20]["input_blocked"] = True
        else:
            broken["natural_cold"][0]["desired"] = [1, 0, 0]
        try:
            validate(broken)
        except (AssertionError, KeyError, TypeError):
            continue
        raise AssertionError(kind)
    return dict(rejected=5)


if __name__ == "__main__":
    if sys.argv[1] == "self-test":
        result = self_test()
    else:
        label = sys.argv[1]
        result = validate(json.loads((r / ("runtime-" + label) / "parameters.json").read_text()))
        result["gate"] = "PASS_TARGET_NATURAL_AND_CONTROLLED_VIEW_PUBLICATION_LIFECYCLE"
    (r / (sys.argv[1] + "-check.json")).write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result), flush=True)
