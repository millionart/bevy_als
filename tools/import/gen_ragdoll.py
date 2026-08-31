import json
import math
import re
import struct
import sys
import uuid
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "target" / "bf109" / "source_native" / "PA_Als.ue.json"
GLTF = ROOT / "assets" / "animations" / "Base" / "A_Als_Idle.gltf"
RAGDOLL = ROOT / "assets" / "ragdolls" / "als_manny.rag.ron"
BONE_MAP = ROOT / "assets" / "ragdoll_bone_maps" / "als_manny.bm.ron"
PROFILE = ROOT / "assets" / "ragdolls" / "pa_als.physics.json"
NAMESPACE = uuid.UUID("a4ea5f25-7f81-5f46-bb37-15ebc4778125")
CM_TO_M = 0.01


def stable_id(kind, label):
    return str(uuid.uuid5(NAMESPACE, f"pa_als/{kind}/{label}"))


def text_number(value):
    value = float(value)
    if not math.isfinite(value):
        raise ValueError(f"non-finite number: {value}")
    return f"{value:.9g}"


def text_vec(values):
    return "(" + ", ".join(text_number(value) for value in values) + ")"


def props(entries):
    return {entry["name"]: entry["exported_text"] for entry in entries}


def numeric(entries, key, default=0.0):
    value = props(entries).get(key)
    return float(value) if value is not None else default


def boolean(entries, key, default=False):
    value = props(entries).get(key)
    if value is None:
        return default
    if value == "True":
        return True
    if value == "False":
        return False
    raise ValueError(f"invalid boolean: {key}={value}")


def source_props(source):
    result = {}
    for obj in source["asset_objects"]:
        values = props(obj["properties"])
        bone = values.get("BoneName")
        if bone:
            result[("body", bone)] = values
        instance = values.get("DefaultInstance", "")
        match = re.search(r'JointName="([^"]+)"', instance)
        if match:
            result[("constraint", match.group(1))] = values
    return result


def number_after(text, key, default=0.0):
    match = re.search(
        rf"\b{re.escape(key)}=([-+]?(?:\d+(?:\.\d*)?|\.\d+)(?:[Ee][-+]?\d+)?)",
        text,
    )
    return float(match.group(1)) if match else default


def word_after(text, key, default=""):
    match = re.search(rf"\b{re.escape(key)}=([A-Za-z0-9_]+)", text)
    return match.group(1) if match else default


def bool_after(text, key, default=False):
    value = word_after(text, key, "")
    if not value:
        return default
    if value == "True":
        return True
    if value == "False":
        return False
    raise ValueError(f"invalid boolean: {key}={value}")


def nested_after(text, key):
    marker = f"{key}=("
    start = text.find(marker)
    if start < 0:
        return ""
    depth = 0
    for index in range(start + len(key) + 1, len(text)):
        character = text[index]
        if character == "(":
            depth += 1
        elif character == ")":
            depth -= 1
            if depth == 0:
                return text[start + len(key) + 2 : index]
    raise ValueError(f"unclosed field: {key}")


def vector_after(text, key):
    match = re.search(
        rf"\b{re.escape(key)}=\(X=([-+0-9.Ee]+),Y=([-+0-9.Ee]+),Z=([-+0-9.Ee]+)\)",
        text,
    )
    return [float(match.group(1)), float(match.group(2)), float(match.group(3))] if match else [0.0, 0.0, 0.0]


def rotator_after(text, key):
    match = re.search(
        rf"\b{re.escape(key)}=\(Pitch=([-+0-9.Ee]+),Yaw=([-+0-9.Ee]+),Roll=([-+0-9.Ee]+)\)",
        text,
    )
    return [float(match.group(1)), float(match.group(2)), float(match.group(3))] if match else [0.0, 0.0, 0.0]


def ue_point(values):
    return [values[0] * CM_TO_M, values[2] * CM_TO_M, values[1] * CM_TO_M]


def normalize(values):
    length = math.sqrt(sum(value * value for value in values))
    if length <= 1e-8:
        raise ValueError(f"zero vector: {values}")
    return [value / length for value in values]


def cross(left, right):
    return [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]


def quat_multiply(left, right):
    lx, ly, lz, lw = left
    rx, ry, rz, rw = right
    return [
        lw * rx + lx * rw + ly * rz - lz * ry,
        lw * ry - lx * rz + ly * rw + lz * rx,
        lw * rz + lx * ry - ly * rx + lz * rw,
        lw * rw - lx * rx - ly * ry - lz * rz,
    ]


def quat_axis(axis, angle):
    sine = math.sin(angle * 0.5)
    return [axis[0] * sine, axis[1] * sine, axis[2] * sine, math.cos(angle * 0.5)]


def quat_conjugate(values):
    return [-values[0], -values[1], -values[2], values[3]]


def quat_rotate(quaternion, values):
    return quat_multiply(quat_multiply(quaternion, [values[0], values[1], values[2], 0.0]), quat_conjugate(quaternion))[:3]


def quat_matrix(columns):
    x_axis, y_axis, z_axis = columns
    matrix = [
        [x_axis[0], y_axis[0], z_axis[0]],
        [x_axis[1], y_axis[1], z_axis[1]],
        [x_axis[2], y_axis[2], z_axis[2]],
    ]
    trace = matrix[0][0] + matrix[1][1] + matrix[2][2]
    if trace > 0.0:
        scale = math.sqrt(trace + 1.0) * 2.0
        return normalize([
            (matrix[2][1] - matrix[1][2]) / scale,
            (matrix[0][2] - matrix[2][0]) / scale,
            (matrix[1][0] - matrix[0][1]) / scale,
            0.25 * scale,
        ])
    if matrix[0][0] > matrix[1][1] and matrix[0][0] > matrix[2][2]:
        scale = math.sqrt(1.0 + matrix[0][0] - matrix[1][1] - matrix[2][2]) * 2.0
        return normalize([
            0.25 * scale,
            (matrix[0][1] + matrix[1][0]) / scale,
            (matrix[0][2] + matrix[2][0]) / scale,
            (matrix[2][1] - matrix[1][2]) / scale,
        ])
    if matrix[1][1] > matrix[2][2]:
        scale = math.sqrt(1.0 + matrix[1][1] - matrix[0][0] - matrix[2][2]) * 2.0
        return normalize([
            (matrix[0][1] + matrix[1][0]) / scale,
            0.25 * scale,
            (matrix[1][2] + matrix[2][1]) / scale,
            (matrix[0][2] - matrix[2][0]) / scale,
        ])
    scale = math.sqrt(1.0 + matrix[2][2] - matrix[0][0] - matrix[1][1]) * 2.0
    return normalize([
        (matrix[0][2] + matrix[2][0]) / scale,
        (matrix[1][2] + matrix[2][1]) / scale,
        0.25 * scale,
        (matrix[1][0] - matrix[0][1]) / scale,
    ])


def ue_quat_to_bevy(values):
    return normalize([-values[0], -values[2], -values[1], values[3]])


def source_rotator(values):
    pitch, yaw, roll = (math.radians(value) for value in values)
    return quat_multiply(
        quat_multiply(quat_axis([0.0, 0.0, 1.0], yaw), quat_axis([0.0, 1.0, 0.0], -pitch)),
        quat_axis([1.0, 0.0, 0.0], -roll),
    )


def ue_rotator(values):
    return ue_quat_to_bevy(source_rotator(values))


def ue_frame(primary, secondary):
    primary = normalize(primary)
    normal = normalize(cross(primary, secondary))
    secondary = normalize(cross(normal, primary))
    return ue_quat_to_bevy(quat_matrix([primary, secondary, normal]))


def direction(values):
    return [values[0], values[2], values[1]]


def assert_close(left, right, tolerance=1e-5):
    if any(abs(a - b) > tolerance for a, b in zip(left, right)):
        raise AssertionError(f"transform mismatch: {left} != {right}")


def self_test():
    assert_close(ue_point([100.0, 200.0, 300.0]), [1.0, 3.0, 2.0])
    for rotator in [(0.0, 0.0, 0.0), (10.0, 30.0, 0.0), (90.0, 90.0, 0.0), (-20.0, -120.0, 60.0)]:
        source = source_rotator(rotator)
        target = ue_rotator(rotator)
        for vector in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 2.0, 3.0]]:
            assert_close(direction(quat_rotate(source, vector)), quat_rotate(target, direction(vector)))
    primary = normalize([1.0, 2.0, 3.0])
    secondary = normalize([-2.0, 1.0, 0.0])
    source_basis = quat_matrix([primary, normalize(cross(normalize(cross(primary, secondary)), primary)), normalize(cross(primary, secondary))])
    target_basis = ue_frame(primary, secondary)
    for vector in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]:
        assert_close(direction(quat_rotate(source_basis, vector)), quat_rotate(target_basis, direction(vector)))


def volume(shape):
    if shape["kind"] == "sphere":
        return (4.0 / 3.0) * math.pi * shape["radius"] ** 3
    if shape["kind"] == "box":
        return shape["x"] * shape["y"] * shape["z"]
    return math.pi * shape["radius"] ** 2 * shape["length"] + (4.0 / 3.0) * math.pi * shape["radius"] ** 3


def target_shape(shape):
    result = {
        "kind": shape["kind"],
        "translation_m": ue_point([shape["center"]["x"], shape["center"]["y"], shape["center"]["z"]]),
        "rotation_xyzw": ue_rotator([
            shape.get("rotation", {}).get("pitch", 0.0),
            shape.get("rotation", {}).get("yaw", 0.0),
            shape.get("rotation", {}).get("roll", 0.0),
        ]),
    }
    if shape["kind"] == "sphere":
        result["radius_m"] = shape["radius"] * CM_TO_M
    elif shape["kind"] == "box":
        result["half_extents_m"] = [shape["x"] * 0.005, shape["z"] * 0.005, shape["y"] * 0.005]
    else:
        result["radius_m"] = shape["radius"] * CM_TO_M
        result["half_length_m"] = shape["length"] * 0.5 * CM_TO_M
    return result


def multiply(left, right):
    return [[sum(left[row][index] * right[index][column] for index in range(4)) for column in range(4)] for row in range(4)]


def inverse(matrix):
    rotation = [[matrix[column][row] for column in range(3)] for row in range(3)]
    translation = [matrix[0][3], matrix[1][3], matrix[2][3]]
    return [
        rotation[0] + [-sum(rotation[0][index] * translation[index] for index in range(3))],
        rotation[1] + [-sum(rotation[1][index] * translation[index] for index in range(3))],
        rotation[2] + [-sum(rotation[2][index] * translation[index] for index in range(3))],
        [0.0, 0.0, 0.0, 1.0],
    ]


def matrix_quat(matrix):
    return quat_matrix([
        [matrix[0][0], matrix[1][0], matrix[2][0]],
        [matrix[0][1], matrix[1][1], matrix[2][1]],
        [matrix[0][2], matrix[1][2], matrix[2][2]],
    ])


def load_skeleton():
    document = json.loads(GLTF.read_text(encoding="utf-8"))
    buffers = []
    for buffer in document["buffers"]:
        buffers.append(((GLTF.parent / buffer["uri"]).read_bytes(), buffer.get("byteOffset", 0)))
    accessor = document["accessors"][document["skins"][0]["inverseBindMatrices"]]
    view = document["bufferViews"][accessor["bufferView"]]
    data, base = buffers[view["buffer"]]
    start = base + view.get("byteOffset", 0) + accessor.get("byteOffset", 0)
    inverse_binds = []
    for index in range(accessor["count"]):
        values = struct.unpack_from("<16f", data, start + index * 64)
        inverse_binds.append([
            [values[0], values[4], values[8], values[12]],
            [values[1], values[5], values[9], values[13]],
            [values[2], values[6], values[10], values[14]],
            [values[3], values[7], values[11], values[15]],
        ])
    rest = {}
    for node, inverse_bind in zip(document["skins"][0]["joints"], inverse_binds):
        rest[document["nodes"][node]["name"]] = inverse(inverse_bind)
    names = {}
    parents = {}
    for index, node in enumerate(document["nodes"]):
        if "name" in node:
            names[node["name"]] = index
        for child in node.get("children", []):
            parents[child] = index
    return document, rest, names, parents


def path(document, names, parents, bone):
    parts = []
    index = names[bone]
    while True:
        parts.append(document["nodes"][index].get("name", f"GltfNode{index}"))
        if index not in parents:
            break
        index = parents[index]
    return list(reversed(parts))


def path_text(parts):
    return ", ".join(json.dumps(part) for part in parts)


def translation(rest, bone):
    matrix = rest[bone]
    return [matrix[0][3], matrix[1][3], matrix[2][3]]


def transform_point(matrix, values):
    return [
        matrix[row][0] * values[0] + matrix[row][1] * values[1] + matrix[row][2] * values[2] + matrix[row][3]
        for row in range(3)
    ]


def verify_constraint_rest_frames(profile, rest):
    for constraint in profile["constraints"]:
        child = constraint["constraint_bone_1"]
        parent = constraint["constraint_bone_2"]
        frame_1 = constraint["target"]["frame_1"]["translation_m"]
        frame_2 = constraint["target"]["frame_2"]["translation_m"]
        child_anchor = transform_point(rest[child], frame_1)
        parent_anchor = transform_point(rest[parent], frame_2)
        assert_close(child_anchor, parent_anchor, 2e-5)


def parsed_profile(entries):
    text = props(entries).get("ProfileInstance", "")
    cone = nested_after(text, "ConeLimit")
    twist = nested_after(text, "TwistLimit")
    linear_drive = nested_after(text, "LinearDrive")
    angular_drive = nested_after(text, "AngularDrive")
    result = {
        "linear_x_motion": word_after(text, "XMotion", "LCM_Free"),
        "linear_y_motion": word_after(text, "YMotion", "LCM_Free"),
        "linear_z_motion": word_after(text, "ZMotion", "LCM_Free"),
        "swing_1_motion": word_after(cone, "Swing1Motion", "ACM_Free"),
        "swing_2_motion": word_after(cone, "Swing2Motion", "ACM_Free"),
        "twist_motion": word_after(twist, "TwistMotion", "ACM_Free"),
        "swing_1_limit_degrees": number_after(cone, "Swing1LimitDegrees"),
        "swing_2_limit_degrees": number_after(cone, "Swing2LimitDegrees"),
        "twist_limit_degrees": number_after(twist, "TwistLimitDegrees"),
        "cone_stiffness": number_after(cone, "Stiffness"),
        "cone_damping": number_after(cone, "Damping"),
        "twist_stiffness": number_after(twist, "Stiffness"),
        "twist_damping": number_after(twist, "Damping"),
        "linear_drive_stiffness": number_after(nested_after(linear_drive, "XDrive"), "Stiffness"),
        "linear_drive_damping": number_after(nested_after(linear_drive, "XDrive"), "Damping"),
        "angular_drive_stiffness": number_after(nested_after(angular_drive, "TwistDrive"), "Stiffness"),
        "angular_drive_damping": number_after(nested_after(angular_drive, "TwistDrive"), "Damping"),
        "angular_drive_enabled": bool_after(nested_after(angular_drive, "TwistDrive"), "bEnablePositionDrive", False),
        "angular_drive_target_ue": rotator_after(angular_drive, "OrientationTarget"),
        "projection_linear_tolerance_cm": number_after(text, "ProjectionLinearTolerance"),
        "projection_angular_tolerance_degrees": number_after(text, "ProjectionAngularTolerance"),
        "projection_linear_alpha": number_after(text, "ProjectionLinearAlpha"),
        "shock_propagation_alpha": number_after(text, "ShockPropagationAlpha"),
        "linear_break_threshold": number_after(text, "LinearBreakThreshold"),
        "angular_break_threshold": number_after(text, "AngularBreakThreshold"),
        "cone_soft": bool_after(cone, "bSoftConstraint", False),
        "twist_soft": bool_after(twist, "bSoftConstraint", False),
        "projection_enabled": bool_after(text, "bEnableProjection", False),
        "mass_conditioning_enabled": bool_after(text, "bEnableMassConditioning", False),
        "linear_joint_solver": bool_after(text, "bUseLinearJointSolver", False),
    }
    return result


def mode_limit(mode, degrees):
    if mode == "ACM_Locked":
        return 0.0
    if mode == "ACM_Limited":
        return math.radians(degrees)
    return None


def build_profile(source):
    source_map = source_props(source)
    physics = source["physics_asset"]
    bodies = []
    for body in sorted(physics["bodies"], key=lambda item: item["index"]):
        bone = body["bone_name"]
        original = source_map[("body", bone)]
        shapes = []
        total = sum(volume(shape) for shape in body["shapes"])
        for index, shape in enumerate(body["shapes"]):
            item = dict(shape)
            item["source_shape_index"] = index
            item["target"] = target_shape(shape)
            item["target"]["mass_kg"] = body["calculated_mass"] * volume(shape) / total
            shapes.append(item)
        physics_type = original["PhysicsType"]
        mode = "Kinematic" if physics_type == "PhysType_Kinematic" else "Dynamic"
        bodies.append({
            "source_index": body["index"],
            "bone_name": bone,
            "physics_type": physics_type,
            "collision_response": original["CollisionReponse"],
            "calculated_mass_kg": body["calculated_mass"],
            "source_properties": original,
            "source_default_instance": props(body["default_instance_properties"]),
            "target": {
                "default_mode": mode,
                "active_mode": mode,
                "linear_damping": numeric(body["default_instance_properties"], "LinearDamping"),
                "angular_damping": numeric(body["default_instance_properties"], "AngularDamping"),
                "max_angular_speed_radians_per_second": math.radians(numeric(body["default_instance_properties"], "MaxAngularVelocity")),
                "gravity_scale": 1.0 if boolean(body["default_instance_properties"], "bEnableGravity", True) else 0.0,
            },
            "shapes": shapes,
        })
    constraints = []
    for constraint in sorted(physics["constraints"], key=lambda item: item["index"]):
        name = constraint["joint_name"]
        profile = parsed_profile(constraint["instance_properties"])
        swing = [
            value
            for value in [
                mode_limit(profile["swing_1_motion"], profile["swing_1_limit_degrees"]),
                mode_limit(profile["swing_2_motion"], profile["swing_2_limit_degrees"]),
            ]
            if value is not None
        ]
        twist = mode_limit(profile["twist_motion"], profile["twist_limit_degrees"])
        frame_1_position = [constraint["position_1"][key] for key in ["x", "y", "z"]]
        frame_2_position = [constraint["position_2"][key] for key in ["x", "y", "z"]]
        primary_1 = [constraint["primary_axis_1"][key] for key in ["x", "y", "z"]]
        secondary_1 = [constraint["secondary_axis_1"][key] for key in ["x", "y", "z"]]
        primary_2 = [constraint["primary_axis_2"][key] for key in ["x", "y", "z"]]
        secondary_2 = [constraint["secondary_axis_2"][key] for key in ["x", "y", "z"]]
        angular_offset = [constraint["angular_rotation_offset"][key] for key in ["pitch", "yaw", "roll"]]
        linear_locked = all(profile[key] == "LCM_Locked" for key in ["linear_x_motion", "linear_y_motion", "linear_z_motion"])
        constraints.append({
            "source_index": constraint["index"],
            "joint_name": name,
            "constraint_bone_1": constraint["constraint_bone_1"],
            "constraint_bone_2": constraint["constraint_bone_2"],
            "source_properties": source_map[("constraint", name)],
            "source_instance_properties": props(constraint["instance_properties"]),
            "frame_1_ue": {"translation_cm": frame_1_position, "primary_axis": primary_1, "secondary_axis": secondary_1},
            "frame_2_ue": {"translation_cm": frame_2_position, "primary_axis": primary_2, "secondary_axis": secondary_2},
            "angular_rotation_offset_ue": angular_offset,
            "source_profile": profile,
            "target": {
                "frame_1": {"translation_m": ue_point(frame_1_position), "rotation_xyzw": ue_frame(primary_1, secondary_1)},
                "frame_2": {"translation_m": ue_point(frame_2_position), "rotation_xyzw": ue_frame(primary_2, secondary_2)},
                "angular_rotation_offset_xyzw": ue_rotator(angular_offset),
                "twist_axis": [1.0, 0.0, 0.0],
                "swing_limit_radians": None if not swing else [-max(swing), max(swing)],
                "twist_limit_radians": None if twist is None else [-twist, twist],
                "point_compliance": 0.0,
                "swing_compliance": 0.0 if not profile["cone_soft"] or profile["cone_stiffness"] <= 0.0 else 1.0 / profile["cone_stiffness"],
                "twist_compliance": 0.0 if not profile["twist_soft"] or profile["twist_stiffness"] <= 0.0 else 1.0 / profile["twist_stiffness"],
                "calibrate_frame_2_to_initial_pose": True,
                "linear_locked": linear_locked,
                "enabled": linear_locked or bool(swing) or twist is not None or profile["angular_drive_enabled"],
            },
        })
    return {
        "format": "pa_als_physics_v1",
        "source_asset_path": source["asset_path"],
        "source_units_per_meter": 100.0,
        "target_axis_mapping": ["x", "z", "y"],
        "solver_settings": props(physics["solver_settings_properties"]),
        "bodies": bodies,
        "constraints": constraints,
        "collision_disabled_pairs": [[pair["body_index_1"], pair["body_index_2"]] for pair in sorted(physics["collision_disabled_pairs"], key=lambda pair: (pair["body_index_1"], pair["body_index_2"]))],
    }


def write_ragdoll(profile, rest):
    ids = {body["bone_name"]: stable_id("body", body["bone_name"]) for body in profile["bodies"]}
    collider_ids = {}
    for body in profile["bodies"]:
        for shape in body["shapes"]:
            key = f"{body['bone_name']}/{shape['source_shape_index']}"
            collider_ids[key] = stable_id("collider", key)
    lines = ["(", "    bodies: {"]
    for body in profile["bodies"]:
        bone = body["bone_name"]
        collider_refs = []
        for shape in body["shapes"]:
            key = f"{bone}/{shape['source_shape_index']}"
            collider_refs.extend(["                (", f'                    uuid: "{collider_ids[key]}",', "                ),"])
        lines.extend([
            "        (", f'            uuid: "{ids[bone]}",', "        ): (", "            id: (", f'                uuid: "{ids[bone]}",', "            ),",
            f'            label: "{bone}",', f"            offset: {text_vec(translation(rest, bone))},", "            colliders: [", *collider_refs, "            ],",
            f"            default_mode: {body['target']['default_mode']},", "            use_symmetry: false,", "            created_from: None,", "        ),",
        ])
    lines.append("    },")
    lines.append("    colliders: {")
    for body in profile["bodies"]:
        bone = body["bone_name"]
        for shape in body["shapes"]:
            target = shape["target"]
            key = f"{bone}/{shape['source_shape_index']}"
            if target["kind"] == "sphere":
                geometry = f"Sphere((radius: {text_number(target['radius_m'])}))"
            elif target["kind"] == "box":
                geometry = f"Cuboid((half_size: {text_vec(target['half_extents_m'])}))"
            else:
                geometry = f"Capsule((half_length: {text_number(target['half_length_m'])}, radius: {text_number(target['radius_m'])}))"
            lines.extend([
                "        (", f'            uuid: "{collider_ids[key]}",', "        ): (", "            id: (", f'                uuid: "{collider_ids[key]}",', "            ),",
                "            local_offset: (", f"                rotation: {text_vec(target['rotation_xyzw'])},", f"                translation: {text_vec(target['translation_m'])},", "            ),",
                f"            shape: {geometry},", "            layer_membership: 1,", "            layer_filter: 1,", "            override_layers: false,", f"            mass_mode: Override({text_number(target['mass_kg'])}),",
                f'            label: "{bone}_{shape["source_shape_index"]}_{target["kind"]}",', "            created_from: None,", "        ),",
            ])
    lines.append("    },")
    lines.append("    joints: {")
    for constraint in profile["constraints"]:
        target = constraint["target"]
        swing = target["swing_limit_radians"]
        twist = target["twist_limit_radians"]
        swing_text = "None" if swing is None else f"Some((min: {text_number(swing[0])}, max: {text_number(swing[1])}))"
        twist_text = "None" if twist is None else f"Some((min: {text_number(twist[0])}, max: {text_number(twist[1])}))"
        body_1 = constraint["constraint_bone_1"]
        body_2 = constraint["constraint_bone_2"]
        joint = stable_id("joint", constraint["joint_name"])
        lines.extend([
            "        (", f'            uuid: "{joint}",', "        ): (", "            id: (", f'                uuid: "{joint}",', "            ),",
            f'            label: "{constraint["joint_name"]}",', "            use_symmetry: false,", "            created_from: None,", "            variant: Spherical((",
            "                body1: (", f'                    uuid: "{ids[body_1]}",', "                ),", "                body2: (", f'                    uuid: "{ids[body_2]}",', "                ),",
            f"                position: {text_vec(translation(rest, body_1))},", f"                twist_axis: {text_vec(target['twist_axis'])},", f"                swing_limit: {swing_text},", f"                twist_limit: {twist_text},",
            f"                point_compliance: {text_number(target['point_compliance'])},", f"                swing_compliance: {text_number(target['swing_compliance'])},", f"                twist_compliance: {text_number(target['twist_compliance'])},", "            )),", "        ),",
        ])
    total_mass = sum(body["calculated_mass_kg"] for body in profile["bodies"])
    lines.extend(["    },", "    suffixes: (", '        original: ".R",', '        mirror: ".L",', "    ),", f"    total_mass: {text_number(total_mass)},", ")", ""])
    RAGDOLL.write_text("\n".join(lines), encoding="utf-8")
    return ids


def write_bone_map(profile, document, rest, names, parents, ids):
    mapping = {body["bone_name"]: body["bone_name"] for body in profile["bodies"]}
    mapping["ball_l"] = "foot_l"
    mapping["ball_r"] = "foot_r"
    lines = ["(", "    bones_from_bodies: {"]
    for bone, body in sorted(mapping.items()):
        bone_path = path(document, names, parents, bone)
        local = multiply(inverse(rest[body]), rest[bone])
        lines.extend([
            "        [", f"            {path_text(bone_path)},", "        ]: (", "            bone_id: [", f"                {path_text(bone_path)},", "            ],", "            bodies: [", "                (",
            "                    body: (", f'                        uuid: "{ids[body]}",', "                    ),", "                    weight: 1.0,", "                    offset: (", f"                        rotation: {text_vec(matrix_quat(local))},", f"                        translation: {text_vec([local[0][3], local[1][3], local[2][3]])},", "                    ),", "                    override_offset: false,", "                ),", "            ],", "            created_from: None,", "        ),",
        ])
    lines.append("    },")
    lines.append("    bodies_from_bones: {")
    for body in profile["bodies"]:
        bone = body["bone_name"]
        bone_path = path(document, names, parents, bone)
        lines.extend([
            "        (", f'            uuid: "{ids[bone]}",', "        ): (", "            body_id: (", f'                uuid: "{ids[bone]}",', "            ),", "            bone: (", "                bone: [", f"                    {path_text(bone_path)},", "                ],", "                offset: (", "                    rotation: (0.0, 0.0, 0.0, 1.0),", "                    translation: (0.0, 0.0, 0.0),", "                ),", "                override_offset: false,", "            ),", "            created_from: None,", "        ),",
        ])
    lines.extend(["    },", '    skeleton: "skeletons/SKM_Als.skn.ron",', '    ragdoll: "ragdolls/als_manny.rag.ron",', "    skeleton_symmetry: (", "        name_mapper: Pattern((", '            key_1: "_l",', '            key_2: "_r",', '            pattern_before: "^.*",', '            pattern_after: "$",', "        )),", "        mode: MirrorX,", "    ),", ")", ""])
    BONE_MAP.write_text("\n".join(lines), encoding="utf-8")


def main():
    self_test()
    if "--self-test" in sys.argv:
        print("transform conversion self-test PASS")
        return
    if not SOURCE.exists():
        raise SystemExit(f"missing source export: {SOURCE}")
    source = json.loads(SOURCE.read_text(encoding="utf-8"))
    if source.get("format") != "ue_asset_source_v1" or "physics_asset" not in source:
        raise SystemExit("source export does not contain a PhysicsAsset")
    profile = build_profile(source)
    document, rest, names, parents = load_skeleton()
    missing = [body["bone_name"] for body in profile["bodies"] if body["bone_name"] not in rest]
    if missing:
        raise SystemExit(f"missing skeleton bones: {missing}")
    PROFILE.write_text(json.dumps(profile, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    verify_constraint_rest_frames(profile, rest)
    ids = write_ragdoll(profile, rest)
    write_bone_map(profile, document, rest, names, parents, ids)
    print(
        "generated "
        f"bodies={len(profile['bodies'])} "
        f"colliders={sum(len(body['shapes']) for body in profile['bodies'])} "
        f"constraints={len(profile['constraints'])} "
        f"collision_disabled_pairs={len(profile['collision_disabled_pairs'])}"
    )


if __name__ == "__main__":
    main()
