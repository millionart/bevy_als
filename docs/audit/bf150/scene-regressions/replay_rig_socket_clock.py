import gzip
import json
import math
import re
import statistics
import sys
from pathlib import Path

root = Path(__file__).resolve().parent
sys.path.insert(0, str(root.parents[3] / "tools/import"))
from import_moving_actors import compose


def transform(value):
    return {
        key: [value[key][axis] for axis in axes]
        for key, axes in (("translation", "xyz"), ("rotation", "xyzw"), ("scale", "xyz"))
    }


def mesh_positions(row, component):
    bones = row["pose_boundaries"]["rig_source"]["pose"]["bones"]
    local = {item["name"]: transform(item["local"]) for item in row["mesh_local_pose"]}
    component_poses = []
    for bone in bones:
        index = bone["parent_index"]
        parent = component if index < 0 else component_poses[index]
        component_poses.append(compose(parent, local[bone["name"]]))
    return {bone["name"]: pose["translation"] for bone, pose in zip(bones, component_poses)}


with gzip.open(root / "rig-socket-clock.native.json.gz", "rt") as stream:
    rows = json.load(stream)["character_animation_probe"]["cases"][0]["frames"]

errors = []
peaks = {}
for previous, current in zip(rows[24:], rows[25:]):
    component = transform(current["mesh_to_world"])
    previous_socket = mesh_positions(previous, component)
    current_socket = mesh_positions(current, component)
    relative = [
        mesh_positions(row, {**transform(row["mesh_to_world"]), "translation": [0, 0, 0]})
        for row in (previous, current)
    ]
    state = next(item["fields"] for item in current["source_state_after_tick"] if item["name"] == "FeetState")
    for side, suffix in (("Left", "l"), ("Right", "r")):
        foot = next(item["fields"] for item in state if item["name"] == side)
        text = next(item["exported_text"] for item in foot if item["name"] == "TargetLocationWorldSpace")
        target = [float(value) for value in re.findall(r"[XYZ]=([-+0-9.eE]+)", text)]
        marker = "ik_foot_" + suffix
        errors.append((math.dist(target, previous_socket[marker]), math.dist(target, current_socket[marker])))
        bone = "foot_" + suffix
        speed = math.dist(relative[0][bone], relative[1][bone]) * 0.01 / current["world_delta_seconds"]
        if speed > peaks.get(bone, {}).get("measured_speed_m_s", 0):
            peaks[bone] = {
                "frame": current["frame"],
                "prior_delta_seconds": previous["world_delta_seconds"],
                "delta_seconds": current["world_delta_seconds"],
                "measured_speed_m_s": speed,
            }

packet = json.loads((root / "rig-socket-clock-comparison.json").read_text())
assert len(errors) == packet["socket_checks"]
assert math.isclose(max(error[0] for error in errors), packet["previous_mesh_pose_in_current_comp_max_error_cm"], abs_tol=1e-12)
assert math.isclose(statistics.median(error[1] for error in errors), packet["current_mesh_pose_median_error_cm"], abs_tol=1e-12)
for bone, peak in peaks.items():
    assert peak["frame"] == packet["original_40_m_s_guard_applied_to_ue_variable_steps"][bone]["frame"]
    assert math.isclose(peak["measured_speed_m_s"], packet["original_40_m_s_guard_applied_to_ue_variable_steps"][bone]["measured_speed_m_s"], abs_tol=1e-12)
    assert peak["measured_speed_m_s"] > 40
print(f"Verified {len(errors)} source foot socket positions and the variable-step speed peaks.")
