import gzip
import json
import math
from pathlib import Path

root = Path(__file__).resolve().parent
with gzip.open(root / "short-step-native.native.json.gz", "rb") as stream:
    cases = json.load(stream)["character_animation_probe"]["cases"]
names = ("lowerarm_l", "lowerarm_r", "hand_l", "hand_r", "foot_l", "foot_r", "pelvis")


def xyz(value):
    return tuple(value[axis] for axis in "xyz")


def quaternion_product(left, right):
    ax, ay, az, aw = left
    bx, by, bz, bw = right
    return (
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    )


def compose(parent, local):
    position, rotation, scale = parent
    scaled_translation = tuple(a * b for a, b in zip(scale, xyz(local["translation"])))
    x, y, z, w = rotation
    offset = quaternion_product(
        quaternion_product(rotation, (*scaled_translation, 0)), (-x, -y, -z, w)
    )[:3]
    local_rotation = tuple(local["rotation"][axis] for axis in "xyzw")
    return (
        tuple(a + b for a, b in zip(position, offset)),
        quaternion_product(rotation, local_rotation),
        tuple(a * b for a, b in zip(scale, xyz(local["scale"]))),
    )


def positions(frame, stage):
    bones = frame["pose_boundaries"]["rig_source"]["pose"]["bones"]
    if stage == "mesh":
        transforms = {bone["name"]: bone["local"] for bone in frame["mesh_local_pose"]}
    else:
        transforms = {bone["name"]: bone["local_transform"] for bone in bones}
    component_poses = []
    origin = ((0, 0, 0), (0, 0, 0, 1), (1, 1, 1))
    for bone in bones:
        index = bone["parent_index"]
        parent = origin if index < 0 else component_poses[index]
        component_poses.append(compose(parent, transforms[bone["name"]]))
    return {
        bone["name"]: pose[0]
        for bone, pose in zip(bones, component_poses)
        if bone["name"] in names
    }



result = {"source": []}
for case in cases:
    rows = case["frames"]
    entry = next(
        i for i, row in enumerate(rows)
        if row["pose_boundaries"]["locomotion_action"]["tag_name"] == "Als.LocomotionAction.Ragdolling"
    )
    before, after = rows[entry - 1:entry + 1]
    incoming = [row["actor_velocity_cm_s"]["z"] for row in (before, after)]
    assert incoming[0] <= -1000 and incoming[1] >= -500
    assert before["mesh_to_world"] == after["mesh_to_world"]
    assert before["actor_transform"]["translation"] != after["actor_transform"]["translation"]
    delta = after["world_delta_seconds"]
    speeds = {}
    for stage in ("rig_source", "mesh"):
        previous = positions(before, stage)
        current = positions(after, stage)
        speeds[stage] = {
            name: math.dist(previous[name], current[name]) * 0.02 / delta
            for name in names
        }
    assert all(value <= 40 for value in speeds["mesh"].values())
    result["source"].append({
        "case": case["name"],
        "entry_index": entry,
        "delta_seconds": delta,
        "actor_vertical_velocity_cm_s": incoming,
        "mesh_component_origin_cm": after["mesh_to_world"]["translation"],
        "mesh_relative_speed_m_s_at_scale_2": speeds,
    })
assert all(
    result["source"][1]["mesh_relative_speed_m_s_at_scale_2"]["rig_source"][name] > 40
    for name in ("lowerarm_l", "lowerarm_r")
)

with gzip.open(root.parent / "after/ragdoll/report.json.gz", "rb") as stream:
    target = json.load(stream)
anomaly = next(
    value for value in target["temporal_anomalies"]
    if value["stage"] == "final" and value["subject"] == "foot_l"
)
samples = target["pose_samples"]
index = next(i for i, sample in enumerate(samples) if sample["elapsed_secs"] == anomaly["elapsed_secs"])
before, after = samples[index - 1:index + 1]
delta = anomaly["animation_delta_seconds"]
speeds = {}
for key in ("avatar_world_translation", "player_world_translation"):
    positions_in_frame = [
        [bone - origin for bone, origin in zip(
            sample["bones"]["foot_l"]["translation"], sample["ragdoll_root_space"][key]
        )]
        for sample in (before, after)
    ]
    speeds[key] = math.dist(*positions_in_frame) / delta
assert abs(speeds["avatar_world_translation"] - anomaly["rate"]) < 0.001
assert speeds["avatar_world_translation"] > 40 > speeds["player_world_translation"]
result["target"] = {
    "elapsed_secs": [before["elapsed_secs"], after["elapsed_secs"]],
    "delta_seconds": delta,
    "foot_l_speed_m_s": speeds,
}
print(json.dumps(result, indent=2))
