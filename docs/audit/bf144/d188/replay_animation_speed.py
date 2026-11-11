import gzip
import json
from pathlib import Path


directory = Path(__file__).resolve().parent


def read_native(path):
    with gzip.open(path) as stream:
        return json.load(stream)


character = read_native(directory / "character.native.json.gz")
movement = next(item for item in character["blueprint_cdo_subobjects"]
                if item["class_path"] == "/Script/ALS.AlsCharacterMovementComponent")
assert next(prop["exported_text"] for prop in movement["properties"] if prop["name"] == "bTickBeforeOwner") == "True"
roll = read_native(directory.parent / "d187/roll.native.json.gz")
assert next(prop["exported_text"] for prop in roll["asset_properties"] if prop["name"] == "bEnableRootMotion") == "True"
crouching = read_native(directory / "crouching.native.json.gz")
path = "GetParent.LocomotionState.bMovingSmooth"
assert any(prop.get("exported_text") == path
           for graph in crouching["graphs"] for node in graph["nodes"]
           for prop in node.get("properties", []))


def moving_smooth(speed, has_input):
    return (has_input and speed >= 0.01) or speed > 1.5


cases = [
    ("roll_30_hz", 1 / 30, True, False, False, (0.471937, False), (1.582839, False)),
    ("roll_60_hz", 1 / 60, True, False, False, (1.928295, False), (2.249697, False)),
    ("previous_input", 1 / 60, True, False, False, (0.5, False), (0.5, True)),
    ("end_of_root_window", 1 / 60, False, False, False, (0.5, False), (2.0, False)),
    ("interrupted", 1 / 60, True, True, False, (0.5, False), (2.0, False)),
    ("ragdoll", 1 / 60, True, False, True, (0.5, False), (2.0, False)),
    ("zero_delta", 0.0, True, False, False, (0.5, False), (2.0, False)),
    ("subminimum", 0.5e-6, True, False, False, (0.5, False), (2.0, False)),
    ("minimum", 1e-6, True, False, False, (0.5, False), (2.0, False)),
    ("ordinary_walk", 1 / 60, False, False, False, (1.0, True), (2.0, True)),
]
rows = []
for name, delta, root_motion, interrupted, ragdoll, previous, current in cases:
    early = root_motion and not (interrupted or ragdoll) and delta >= 1e-6
    selected = previous if early else current
    rows.append({"case": name, "autonomous_pose_tick": early, "actual_speed": current[0],
                 "animation_speed": selected[0], "moving_smooth": moving_smooth(*selected),
                 "legacy_moving_smooth": moving_smooth(*current)})
assert not rows[0]["moving_smooth"] and rows[0]["legacy_moving_smooth"]
assert not rows[2]["moving_smooth"] and rows[2]["legacy_moving_smooth"]
assert [row["autonomous_pose_tick"] for row in rows] == [True, True, True, False, False, False, False, False, True, False]
previous, current = cases[1][-2:]
assert abs(previous[0] / 1.5 - 1.285530) < 1e-6
assert abs(current[0] / 1.5 - 1.499798) < 1e-6
print(json.dumps({"crouching_input": path, "cases": rows,
                  "controlled_crouching_rates": [previous[0] / 1.5, current[0] / 1.5]}, indent=2))
