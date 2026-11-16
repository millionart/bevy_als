import gzip
import json
import math
import struct
import sys
from pathlib import Path


AUDIT = Path(__file__).resolve().parent
sys.path.insert(0, str(AUDIT.parents[4] / "tools/import"))
from coordinate_basis import quaternion_multiply, source_point_to_runtime
from gen_ragdoll import quat_rotate


STEP = struct.pack("f", 0.006985542)
BONES = ("root", "pelvis", "lowerarm_l", "lowerarm_r", "hand_l", "hand_r", "foot_l", "foot_r")


def load(name):
    path = AUDIT / name
    archive = path.with_suffix(".json.gz")
    if archive.exists():
        with gzip.open(archive, "rb") as stream:
            return json.load(stream)
    return json.loads(path.read_bytes())


def vector(data, axes="xyz"):
    return [data[axis] for axis in axes]


def source_point(transform):
    return source_point_to_runtime(vector(transform["translation"]))


def relative(point, origin):
    return [value - center for value, center in zip(point, origin)]


def source_pose_positions(row, final=False):
    mesh = row["mesh_to_world"]
    mesh_scale = vector(mesh["scale"])
    pose = row["pose_boundaries"]["rig_source"]["pose"]["bones"]
    output = []
    local = (
        {bone["name"]: bone["local"] for bone in row["mesh_local_pose"]}
        if final else {bone["name"]: bone["local_transform"] for bone in pose}
    )
    for bone in pose:
        index = bone["parent_index"]
        position, rotation, scale = output[index] if index >= 0 else (
            [0, 0, 0], vector(mesh["rotation"], "xyzw"), mesh_scale,
        )
        child = local[bone["name"]]
        offset = quat_rotate(rotation, [
            axis * size for axis, size in zip(vector(child["translation"]), scale)
        ])
        output.append((
            [start + delta for start, delta in zip(position, offset)],
            quaternion_multiply(rotation, vector(child["rotation"], "xyzw")),
            [axis * size for axis, size in zip(vector(child["scale"]), scale)],
        ))
    return {
        bone["name"]: source_point_to_runtime(position)
        for bone, (position, _, _) in zip(pose, output) if bone["name"] in BONES
    }


def rates(before, after, step):
    return {bone: math.dist(before[bone], after[bone]) / step for bone in BONES}


def source_cases(name):
    return load(name)["character_animation_probe"]["cases"]


def source_entry(case):
    rows = case["frames"]
    index = next(i for i, row in enumerate(rows) if row["simulating_pelvis"])
    before, after = rows[index - 1:index + 1]
    step = after["world_delta_seconds"]
    assert struct.pack("f", step) == STEP
    assert vector(after["actor_transform"]["scale"]) == [2, 2, 2]
    assert before["mesh_to_world"] == after["mesh_to_world"]
    assert not after["simulating_root"]
    result = {
        "entry_step": index + 1,
        "step_seconds": step,
        "entry_actor_y_m": source_point(after["actor_transform"])[1],
        "incoming_vertical_velocity_m_s": source_point_to_runtime(vector(before["actor_velocity_cm_s"]))[1],
        "mesh_y_m": source_point(after["mesh_to_world"])[1],
        "root_world_step_m": math.dist(source_point(before["root"]), source_point(after["root"])),
        "root_body_velocity_m_s": source_point_to_runtime(vector(after["root_body_velocity_cm_s"])),
    }
    if "pose_boundaries" in before:
        assert struct.pack("f", after["pose_boundaries"]["rig_source"]["delta_seconds"]) == STEP
        result["rig_input_speed_m_s"] = rates(source_pose_positions(before), source_pose_positions(after), step)
        result["published_speed_m_s"] = rates(source_pose_positions(before, final=True), source_pose_positions(after, final=True), step)
    if "bodies_before_physics" in after:
        pre = after["bodies_before_physics"]
        post = after["bodies_after_physics"]
        assert struct.pack("f", after["physics_step_seconds"]) == STEP
        result["physics_step_m"] = {
            bone: math.dist(source_point(pre[bone]["world"]), source_point(post[bone]["world"]))
            for bone in BONES
        }
        result["floor_physics_enabled"] = after["floor_physics_enabled"]
    if "constraints_at_activation" in after:
        constraints = after["constraints_at_activation"]
        result["constraint_count"] = len(constraints)
        result["joint_creation_scales"] = sorted({joint["last_known_scale"] for joint in constraints.values()})
        result["left_foot_parent_anchor_m"] = source_point(constraints["foot_l"]["solver_frame_2"])
        result["self_contacts_suppressed"] = after["self_contacts_suppressed"]
    return result


def target_entry(name):
    rows = load(name)
    index = next(i for i, row in enumerate(rows) if row["readback_active"])
    before, after = rows[index - 1:index + 1]
    step = after["delta_seconds"]
    assert struct.pack("f", step) == STEP
    assert after["actor_scale"] == [2, 2, 2]
    assert before["mesh_attached"] and not after["mesh_attached"]
    assert before["mesh_position"] == after["mesh_position"]
    assert after["bodies"]["root"]["mode"] == "Kinematic"
    assert after["bodies"]["pelvis"]["mode"] == "Dynamic"
    result = {
        "entry_step": index,
        "step_seconds": step,
        "release_actor_y_m": rows[0]["actor_position"][1],
        "entry_actor_y_m": after["actor_position"][1],
        "mesh_y_m": after["mesh_position"][1],
        "selector": [before["air_selector"], after["air_selector"]],
        "root_world_step_m": math.dist(before["bodies"]["root"]["position"], after["bodies"]["root"]["position"]),
        "root_body_velocity_m_s": after["bodies"]["root"]["velocity"],
        "published_body_position_error_m": {
            bone: math.dist(after["bodies"][bone]["position"], after["final_pose"][bone]["translation"])
            for bone in BONES
        },
        "body_position_step_m": {
            bone: math.dist(before["bodies"][bone]["position"], after["bodies"][bone]["position"])
            for bone in BONES
        },
    }
    if "left_foot_parent_anchor" in after:
        result["left_foot_parent_anchor_m"] = after["left_foot_parent_anchor"]
    for key, origin, output in (
        ("pre_rig", "pre_rig_origin", "rig_input_speed_m_s"),
        ("final_pose", "mesh_position", "published_speed_m_s"),
    ):
        relative_poses = [
            {bone: relative(row[key][bone]["translation"], row[origin]) for bone in BONES}
            for row in (before, after)
        ]
        result[output] = rates(*relative_poses, step)
    return result


def main():
    cases = source_cases("native-constraints.json")
    matches = source_cases("native-matched.json")
    matched = source_entry(matches[0])
    original = target_entry("target-trace/scaled-short-step.json")
    repaired = target_entry("target-trace-after/scaled-short-step.json")
    assert matched["entry_step"] == original["entry_step"]
    assert matched["entry_step"] == repaired["entry_step"]
    assert matched["rig_input_speed_m_s"]["lowerarm_l"] > 40
    assert matched["rig_input_speed_m_s"]["lowerarm_r"] > 40
    inputs = [source_entry(case) for case in cases]
    assert [row["joint_creation_scales"] for row in inputs] == [[1], [1], [2]]
    assert inputs[0]["left_foot_parent_anchor_m"] == inputs[1]["left_foot_parent_anchor_m"]
    assert math.dist(repaired["left_foot_parent_anchor_m"], inputs[0]["left_foot_parent_anchor_m"]) < 1e-6
    assert math.isclose(
        math.dist(inputs[2]["left_foot_parent_anchor_m"], [0, 0, 0]),
        2 * math.dist(inputs[0]["left_foot_parent_anchor_m"], [0, 0, 0]),
        rel_tol=1e-12,
    )
    print(json.dumps({
        "source": matched,
        "source_controls": {case["name"]: result for case, result in zip(cases, inputs)},
        "target_before": original,
        "target_after": repaired,
    }, indent=2))


if __name__ == "__main__":
    main()
