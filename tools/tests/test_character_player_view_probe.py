import argparse
import copy
import gzip
import json
import math
from pathlib import Path


def validate(data):
    cold = 0
    updated = 0
    distinct = 0
    snapshots = 0
    for case in data["character_animation_probe"]["cases"]:
        rows = [case["initialization"]["controller_input"]]
        for frame in case["frames"]:
            rows.extend([frame["controller_before_tick"], frame["controller_input"]])
        for row in rows:
            view = row["player_view"]
            assert view["manager_present"] is True
            assert view["view_target_is_character"] is True
            assert view["view_target_is_controller"] is False
            timestamp = view["cache_time"]
            assert type(timestamp) in (int, float) and math.isfinite(timestamp)
            expected = view["cache_rotation_degrees"] if timestamp > 0 else view["view_target_actor_rotation_degrees"]
            actual = view["returned_rotation_degrees"]
            for name in ("pitch", "yaw", "roll"):
                assert type(actual[name]) in (int, float) and math.isfinite(actual[name])
                assert actual[name] == expected[name], (snapshots, name, actual, expected)
            assert all(math.isfinite(view["returned_location_cm"][axis]) for axis in ("x", "y", "z"))
            if timestamp <= 0:
                assert view["returned_location_cm"] == view["view_target_actor_location_cm"]
            cold += timestamp <= 0
            updated += timestamp > 0
            distinct += timestamp <= 0 and actual != row["control_rotation_degrees"]
            snapshots += 1
    assert cold > 0 and distinct > 0
    return dict(snapshots=snapshots, cold=cold, updated=updated, cold_distinct_from_control=distinct)


def self_test():
    rotation = lambda yaw: dict(pitch=0, yaw=yaw, roll=0)
    cold = dict(control_rotation_degrees=rotation(60), player_view=dict(
        manager_present=True, view_target_is_character=True, view_target_is_controller=False,
        cache_time=0, cache_rotation_degrees=rotation(0),
        view_target_actor_rotation_degrees=rotation(15), returned_rotation_degrees=rotation(15),
        returned_location_cm=dict(x=0, y=0, z=92.15), view_target_actor_location_cm=dict(x=0, y=0, z=92.15)))
    warm = copy.deepcopy(cold)
    warm["player_view"].update(cache_time=0.03, cache_rotation_degrees=rotation(45), returned_rotation_degrees=rotation(45))
    data = dict(character_animation_probe=dict(cases=[dict(initialization=dict(controller_input=cold), frames=[dict(controller_before_tick=cold, controller_input=warm)])]))
    assert validate(data)["snapshots"] == 3
    for mutation in ("missing", "control", "zero", "location"):
        broken = copy.deepcopy(data)
        row = broken["character_animation_probe"]["cases"][0]["frames"][0]["controller_input"]
        if mutation == "location":
            row = broken["character_animation_probe"]["cases"][0]["initialization"]["controller_input"]
            row["player_view"]["returned_location_cm"] = dict(x=0, y=0, z=0)
        elif mutation == "missing":
            del row["player_view"]
        else:
            row["player_view"]["returned_rotation_degrees"] = rotation(60 if mutation == "control" else 0)
        try:
            validate(broken)
        except (AssertionError, KeyError):
            continue
        raise AssertionError(mutation)
    return dict(rejected=4)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        result = self_test()
    else:
        with gzip.open(args.input, "rt") if args.input.suffix == ".gz" else args.input.open() as stream:
            result = validate(json.load(stream))
    print(json.dumps(result, sort_keys=True))
