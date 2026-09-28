import argparse
import json
import math
import re
import struct
import sys
import tempfile
import uuid
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
NAMESPACE = uuid.UUID("a4ea5f25-7f81-5f46-bb37-15ebc4778125")
CM_TO_M = 0.01
COLLISION_MODES = ("NoCollision", "QueryOnly", "PhysicsOnly", "QueryAndPhysics", "ProbeOnly", "QueryAndProbe")


def stable_id(source_asset, kind, label):
    return str(uuid.uuid5(NAMESPACE, f"{source_asset}/{kind}/{label}"))


def text_number(value):
    value = float(value)
    if not math.isfinite(value):
        raise ValueError(f"non-finite number: {value}")
    return f"{value:.9g}"


def text_vec(values):
    return "(" + ", ".join(text_number(value) for value in values) + ")"


def props(entries):
    return {entry["name"]: entry["exported_text"] for entry in entries}


def expanded_text(entry):
    if entry.get("fields"):
        return "(" + ",".join(f"{field['name']}={expanded_text(field)}" for field in entry["fields"]) + ")"
    return entry["exported_text"]


def collision_disabled_pairs(physics):
    return sorted([pair["body_index_1"], pair["body_index_2"]] for pair in physics["collision_disabled_pairs"])


def body_collision_layers(response):
    if response == "BodyCollision_Enabled":
        return (1, 1)
    if response == "BodyCollision_Disabled":
        return (0, 0)
    raise ValueError(f"unsupported body collision response: {response}")


def collision_enabled_intersection(body, shape):
    body_bits = COLLISION_MODES.index(body)
    shape_bits = COLLISION_MODES.index(shape)
    query = body_bits & shape_bits & 1
    simulation = bool(body_bits & 6) and bool(shape_bits & 6)
    probe = simulation and bool((body_bits | shape_bits) & 4)
    return COLLISION_MODES[query | (4 if probe else 2 if simulation else 0)]


def target_material(entries):
    values = props(entries)
    friction = float(values["Friction"])
    return {
        "dynamic_friction": friction,
        "static_friction": max(friction, float(values["StaticFriction"])),
        "restitution": float(values["Restitution"]),
        "friction_combine": values["FrictionCombineMode"],
        "restitution_combine": values["RestitutionCombineMode"],
    }


def target_spring(stiffness, damping, acceleration_mode, stiffness_scale, damping_scale):
    units = 1.0 if acceleration_mode else CM_TO_M ** 2
    return {
        "stiffness": stiffness * stiffness_scale * units,
        "damping": damping * damping_scale * units,
        "acceleration_mode": acceleration_mode,
    }


def active_linear_drive(profile):
    return any(
        motion != "LCM_Locked" and (drive["position_enabled"] or drive["velocity_enabled"])
        for motion, drive in zip(
            (profile["linear_x_motion"], profile["linear_y_motion"], profile["linear_z_motion"]),
            profile["linear_drives"],
        )
    )


def active_angular_drive(profile):
    if profile["angular_drive_mode"] != "TwistAndSwing":
        return False
    twist = profile["angular_drives"]["TwistDrive"]
    swing = profile["angular_drives"]["SwingDrive"]
    return (
        profile["twist_motion"] != "ACM_Locked" and (twist["position_enabled"] or twist["velocity_enabled"])
        or (profile["swing_1_motion"] != "ACM_Locked" or profile["swing_2_motion"] != "ACM_Locked")
        and (swing["position_enabled"] or swing["velocity_enabled"])
    )


def ue_angular_velocity(values):
    return [-values[0] * math.tau, -values[2] * math.tau, -values[1] * math.tau]


def target_linear_drive(profile, stiffness_scale, damping_scale):
    return {
        "linear_position_target_m": ue_point(profile["linear_position_target_ue_cm"]),
        "linear_velocity_target_m_per_second": ue_point(profile["linear_velocity_target_ue_cm_per_second"]),
        "linear_drive_acceleration_mode": profile["linear_drive_acceleration_mode"],
        "linear_drives": [
            {
                **profile["linear_drives"][index],
                "stiffness": profile["linear_drives"][index]["stiffness"] * stiffness_scale,
                "damping": profile["linear_drives"][index]["damping"] * damping_scale,
                "max_force": profile["linear_drives"][index]["max_force"] * CM_TO_M,
            } for index in (0, 2, 1)
        ],
    }


def target_angular_limit(mode, radians):
    return {"Limited": radians} if mode == "ACM_Limited" else mode.removeprefix("ACM_")


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


def project_max_angular_velocity_degrees(uasset):
    project = next((parent for parent in uasset.absolute().parents if next(parent.glob("*.uproject"), None)), None)
    if project is None:
        raise ValueError("PhysicsAsset input is not inside an Unreal project")
    config = project / "Config" / "DefaultEngine.ini"
    effective = 3600.0
    if config.is_file():
        in_physics_settings = False
        for raw in config.read_text(encoding="utf-8-sig").splitlines():
            line = raw.split(";", 1)[0].strip()
            if line.startswith("["):
                in_physics_settings = line == "[/Script/Engine.PhysicsSettings]"
            elif in_physics_settings and line.startswith("MaxAngularVelocity="):
                effective = float(line.split("=", 1)[1].strip())
    if not math.isfinite(effective) or effective < 0.0:
        raise ValueError("invalid project MaxAngularVelocity")
    return effective


def target_max_angular_speed(entries, project_degrees):
    degrees = numeric(entries, "MaxAngularVelocity") if boolean(entries, "bOverrideMaxAngularVelocity") else project_degrees
    if not math.isfinite(degrees) or degrees < 0.0:
        raise ValueError("invalid body MaxAngularVelocity")
    return math.radians(degrees)


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
    intersections = (
        ("NoCollision",) * 6,
        ("NoCollision", "QueryOnly", "NoCollision", "QueryOnly", "NoCollision", "QueryOnly"),
        ("NoCollision", "NoCollision", "PhysicsOnly", "PhysicsOnly", "ProbeOnly", "ProbeOnly"),
        ("NoCollision", "QueryOnly", "PhysicsOnly", "QueryAndPhysics", "ProbeOnly", "QueryAndProbe"),
        ("NoCollision", "NoCollision", "ProbeOnly", "ProbeOnly", "ProbeOnly", "ProbeOnly"),
        ("NoCollision", "QueryOnly", "ProbeOnly", "QueryAndProbe", "ProbeOnly", "QueryAndProbe"),
    )
    for body, row in zip(COLLISION_MODES, intersections):
        for shape, expected in zip(COLLISION_MODES, row):
            assert collision_enabled_intersection(body, shape) == expected
    entries = [{"name": "bOverrideMaxAngularVelocity", "exported_text": "False"},
        {"name": "MaxAngularVelocity", "exported_text": "720.000000"}]
    assert math.isclose(target_max_angular_speed(entries, 400.0), math.radians(400.0))
    assert math.isclose(target_max_angular_speed(entries, 3600.0), math.radians(3600.0))
    entries[0]["exported_text"] = "True"
    assert math.isclose(target_max_angular_speed(entries, 400.0), math.radians(720.0))
    with tempfile.TemporaryDirectory() as temporary:
        project = Path(temporary)
        (project / "Project.uproject").write_text("{}", encoding="utf-8")
        uasset = project / "Plugins" / "Example" / "Content" / "PA.uasset"
        assert project_max_angular_velocity_degrees(uasset) == 3600.0
        (project / "Config").mkdir()
        (project / "Config" / "DefaultEngine.ini").write_text(
            "[/Script/Engine.PhysicsSettings]\nMaxAngularVelocity=400.0 ; degrees per second\n",
            encoding="utf-8")
        assert project_max_angular_velocity_degrees(uasset) == 400.0
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
    assert target_joint_variant(math.radians(60.0), 0.0, 0.0) == "Revolute"
    assert target_joint_variant(math.radians(65.0), math.radians(20.0), math.radians(15.0)) == "Spherical"
    assert native_swing_limit(math.radians(65.0), math.radians(20.0)) is None
    assert native_swing_limit(math.radians(20.0), math.radians(20.0)) == [
        -math.radians(20.0),
        math.radians(20.0),
    ]
    assert hinge_axis(math.radians(60.0), 0.0, 0.0) == direction([0.0, 0.0, 1.0])
    assert stable_id("/A.A", "body", "pelvis") != stable_id("/B.B", "body", "pelvis")
    assert target_spring(50.0, 5.0, True, 100000.0, 1000.0) == {
        "stiffness": 5000000.0, "damping": 5000.0, "acceleration_mode": True,
    }
    assert math.isclose(target_spring(50.0, 0.0, False, 1.5, 1.5)["stiffness"], 0.0075)
    assert target_angular_limit("ACM_Locked", 0.0) == "Locked"
    assert target_angular_limit("ACM_Limited", 0.0) == {"Limited": 0.0}
    box = {"kind": "box", "mass_kg": 2.0, "half_extents_m": [0.1, 0.2, 0.3],
        "translation_m": [0.0, 0.0, 0.0], "rotation_xyzw": [0.0, 0.0, 0.0, 1.0]}
    mass = target_mass_properties([box], 2.0, ue_point([10.0, 20.0, 30.0]), direction([2.0, 1.0, 0.5]))
    assert_close(mass["center_of_mass_m"], [0.1, 0.3, 0.2])
    assert_close([mass["inertia_tensor_kg_m2"][i][i] for i in range(3)],
        [0.0866666667, 0.2666666667, 0.1133333333])
    assert target_mass_properties([box], 2.0, [0.0, 0.0, 0.0], [1.0, 1.0, 1.0]) is None
    forced = target_mass_properties([box], 2.0, [0.0, 0.0, 0.0], [1.0, 1.0, 1.0], True)
    assert_close([forced["inertia_tensor_kg_m2"][i][i] for i in range(3)],
        [0.0866666667, 0.0666666667, 0.0333333333])
    sphere = {"kind": "sphere", "mass_kg": 1.0, "radius_m": 0.5,
        "translation_m": [0.0, 0.0, 0.0], "rotation_xyzw": [0.0, 0.0, 0.0, 1.0]}
    sphere_mass = target_mass_properties([sphere], 1.0, [0.1, 0.0, 0.0], [1.0, 1.0, 1.0])
    assert_close([sphere_mass["inertia_tensor_kg_m2"][i][i] for i in range(3)], [0.11, 0.1, 0.1])
    capsule = {"kind": "capsule", "mass_kg": 3.0, "radius_m": 0.2, "half_length_m": 0.4,
        "translation_m": [0.0, 0.0, 0.0], "rotation_xyzw": [0.0, 0.0, 0.0, 1.0]}
    capsule_mass = target_mass_properties([capsule], 3.0, [0.0, 0.0, 0.0], [1.2, 1.0, 0.8])
    assert capsule_mass["inertia_tensor_kg_m2"][0][0] > capsule_mass["inertia_tensor_kg_m2"][1][1] > 0.0
    assert_close(capsule_mass["center_of_mass_m"], [0.0, 0.0, 0.0])
    material = target_material([
        {"name": key, "exported_text": value} for key, value in {
            "Friction": "0.7", "StaticFriction": "0", "Restitution": "0.3",
            "FrictionCombineMode": "Average", "RestitutionCombineMode": "Average",
        }.items()
    ])
    assert material["static_friction"] == material["dynamic_friction"] == 0.7
    assert expanded_text({"exported_text": "()", "fields": [{"name": "Enabled", "exported_text": "True"}]}) == "(Enabled=True)"
    assert collision_disabled_pairs({"collision_disabled_pairs": [{"body_index_1": 0, "body_index_2": 1, "disabled": False}]}) == [[0, 1]]
    assert parsed_profile([{"name": "ProfileInstance", "exported_text": "()", "fields": [
        {"name": "bDisableCollision", "exported_text": "True"},
    ]}])["collision_disabled"]
    assert body_collision_layers("BodyCollision_Enabled") == (1, 1)
    assert body_collision_layers("BodyCollision_Disabled") == (0, 0)
    drive = "(PositionTarget=(X=100,Y=200,Z=300),VelocityTarget=(X=10,Y=20,Z=30),XDrive=(Stiffness=40,Damping=2,MaxForce=100,bEnablePositionDrive=True,bEnableVelocityDrive=False),YDrive=(Stiffness=50,Damping=3,MaxForce=0,bEnablePositionDrive=False,bEnableVelocityDrive=True),ZDrive=(Stiffness=60,Damping=4,MaxForce=80,bEnablePositionDrive=False,bEnableVelocityDrive=False),bAccelerationMode=False)"
    parsed = parsed_profile([{"name": "ProfileInstance", "exported_text": "()", "fields": [
        {"name": "LinearDrive", "exported_text": drive},
    ]}])
    assert parsed["linear_position_target_ue_cm"] == [100.0, 200.0, 300.0]
    assert parsed["linear_velocity_target_ue_cm_per_second"] == [10.0, 20.0, 30.0]
    assert parsed["linear_drives"][0] == {"stiffness": 40.0, "damping": 2.0, "max_force": 100.0,
        "position_enabled": True, "velocity_enabled": False}
    assert parsed["linear_drives"][1]["velocity_enabled"] and not parsed["linear_drive_acceleration_mode"]
    assert active_linear_drive(parsed)
    target_drive = target_linear_drive(parsed, 2.0, 3.0)
    assert target_drive["linear_position_target_m"] == [1.0, 3.0, 2.0]
    assert target_drive["linear_velocity_target_m_per_second"] == [0.1, 0.3, 0.2]
    assert target_drive["linear_drives"][0]["stiffness"] == 80.0
    assert target_drive["linear_drives"][0]["damping"] == 6.0
    assert target_drive["linear_drives"][0]["max_force"] == 1.0
    assert target_drive["linear_drives"][1]["position_enabled"] is False
    assert target_drive["linear_drives"][2]["velocity_enabled"] is True
    parsed["linear_x_motion"] = parsed["linear_y_motion"] = parsed["linear_z_motion"] = "LCM_Locked"
    assert not active_linear_drive(parsed)
    swing = parsed_profile([{"name": "ProfileInstance", "exported_text": "()", "fields": [
        {"name": "AngularDrive", "exported_text": "(AngularDriveMode=TwistAndSwing,AngularVelocityTarget=(X=0,Y=1,Z=0),TwistDrive=(Stiffness=50,Damping=4,MaxForce=0,bEnablePositionDrive=False,bEnableVelocityDrive=False),SwingDrive=(Stiffness=50,Damping=4,MaxForce=0,bEnablePositionDrive=True,bEnableVelocityDrive=True),SlerpDrive=(Stiffness=50,Damping=4,MaxForce=0,bEnablePositionDrive=False,bEnableVelocityDrive=False))"},
    ]}])
    assert swing["angular_drive_enabled"]
    assert swing["angular_velocity_target"] == [0.0, 1.0, 0.0]
    assert_close(ue_angular_velocity(swing["angular_velocity_target"]), [0.0, 0.0, -math.tau])
    for axis in ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]):
        mapped = ue_angular_velocity(axis)
        target = quat_axis(normalize(mapped), math.tau * 0.2)
        source = ue_quat_to_bevy(quat_axis(axis, math.tau * 0.2))
        assert abs(sum(left * right for left, right in zip(target, source))) > 1.0 - 1e-6
    swing["swing_1_motion"] = swing["swing_2_motion"] = "ACM_Locked"
    assert not active_angular_drive(swing)
    swing["angular_drives"]["TwistDrive"]["velocity_enabled"] = True
    assert active_angular_drive(swing)


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


def target_mass_properties(shapes, mass, nudge_m, inertia_scale, force=False):
    if not force and all(value == 0.0 for value in nudge_m) and all(abs(value - 1.0) <= 1e-3 for value in inertia_scale):
        return None
    if not shapes or mass <= 0.0 or abs(sum(shape["mass_kg"] for shape in shapes) - mass) > 1e-3:
        raise ValueError("body mass properties lack a valid shape mass closure")
    center = [sum(shape["mass_kg"] * shape["translation_m"][axis] for shape in shapes) / mass for axis in range(3)]
    tensor = [[0.0] * 3 for _ in range(3)]
    for shape in shapes:
        shape_mass = shape["mass_kg"]
        if shape["kind"] == "sphere":
            diagonal = [0.4 * shape_mass * shape["radius_m"] ** 2] * 3
        elif shape["kind"] == "box":
            x, y, z = shape["half_extents_m"]
            diagonal = [shape_mass * (y * y + z * z) / 3,
                shape_mass * (x * x + z * z) / 3,
                shape_mass * (x * x + y * y) / 3]
        elif shape["kind"] == "capsule":
            radius = shape["radius_m"]
            height = 2.0 * shape["half_length_m"]
            perpendicular = shape_mass * (5 * height ** 3 + 20 * height ** 2 * radius
                + 45 * height * radius ** 2 + 32 * radius ** 3) / (60 * height + 80 * radius)
            axial = shape_mass * radius ** 2 * (15 * height + 16 * radius) / (30 * height + 40 * radius)
            diagonal = [perpendicular, axial, perpendicular]
        else:
            raise ValueError(f"unsupported mass shape: {shape['kind']}")
        basis = [quat_rotate(shape["rotation_xyzw"], axis) for axis in ([1, 0, 0], [0, 1, 0], [0, 0, 1])]
        offset = [shape["translation_m"][axis] - center[axis] for axis in range(3)]
        radius_squared = sum(value * value for value in offset)
        for row in range(3):
            for column in range(3):
                tensor[row][column] += sum(diagonal[axis] * basis[axis][row] * basis[axis][column] for axis in range(3))
                tensor[row][column] += shape_mass * ((radius_squared if row == column else 0.0) - offset[row] * offset[column])
    scale = [abs(value) for value in inertia_scale]
    if all(abs(value - 1.0) <= 1e-3 for value in scale):
        scale = [1.0] * 3
    half_trace = sum(tensor[axis][axis] for axis in range(3)) * 0.5
    axis_squared = [(half_trace - tensor[axis][axis]) * scale[axis] ** 2 for axis in range(3)]
    scaled = [[tensor[row][column] * scale[row] * scale[column] for column in range(3)] for row in range(3)]
    for axis in range(3):
        scaled[axis][axis] = axis_squared[(axis + 1) % 3] + axis_squared[(axis + 2) % 3]
        scaled[axis][axis] += mass * nudge_m[axis] ** 2
    return {
        "mass_kg": mass,
        "center_of_mass_m": [center[axis] + nudge_m[axis] for axis in range(3)],
        "inertia_tensor_kg_m2": scaled,
    }


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


def load_skeleton(gltf):
    document = json.loads(gltf.read_text(encoding="utf-8"))
    buffers = []
    for buffer in document["buffers"]:
        buffers.append(((gltf.parent / buffer["uri"]).read_bytes(), buffer.get("byteOffset", 0)))
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
    entry = next(entry for entry in entries if entry["name"] == "ProfileInstance")
    if not entry.get("fields"):
        raise ValueError("native reader lacks complete constraint profile fields")
    text = expanded_text(entry)
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
        "linear_position_target_ue_cm": vector_after(linear_drive, "PositionTarget"),
        "linear_velocity_target_ue_cm_per_second": vector_after(linear_drive, "VelocityTarget"),
        "linear_drive_acceleration_mode": bool_after(linear_drive, "bAccelerationMode", True),
        "linear_drives": [
            {
                "stiffness": number_after(nested_after(linear_drive, name), "Stiffness"),
                "damping": number_after(nested_after(linear_drive, name), "Damping"),
                "max_force": number_after(nested_after(linear_drive, name), "MaxForce"),
                "position_enabled": bool_after(nested_after(linear_drive, name), "bEnablePositionDrive"),
                "velocity_enabled": bool_after(nested_after(linear_drive, name), "bEnableVelocityDrive"),
            } for name in ("XDrive", "YDrive", "ZDrive")
        ],
        "angular_drive_stiffness": number_after(nested_after(angular_drive, "TwistDrive"), "Stiffness"),
        "angular_drive_damping": number_after(nested_after(angular_drive, "TwistDrive"), "Damping"),
        "angular_drive_mode": word_after(angular_drive, "AngularDriveMode"),
        "angular_drives": {
            name: {
                "stiffness": number_after(nested_after(angular_drive, name), "Stiffness"),
                "damping": number_after(nested_after(angular_drive, name), "Damping"),
                "max_force": number_after(nested_after(angular_drive, name), "MaxForce"),
                "position_enabled": bool_after(nested_after(angular_drive, name), "bEnablePositionDrive"),
                "velocity_enabled": bool_after(nested_after(angular_drive, name), "bEnableVelocityDrive"),
            } for name in ("TwistDrive", "SwingDrive", "SlerpDrive")
        },
        "angular_velocity_target": vector_after(angular_drive, "AngularVelocityTarget"),
        "angular_drive_target_ue": rotator_after(angular_drive, "OrientationTarget"),
        "projection_linear_tolerance_cm": number_after(text, "ProjectionLinearTolerance"),
        "projection_angular_tolerance_degrees": number_after(text, "ProjectionAngularTolerance"),
        "projection_linear_alpha": number_after(text, "ProjectionLinearAlpha"),
        "projection_angular_alpha": number_after(text, "ProjectionAngularAlpha"),
        "shock_propagation_alpha": number_after(text, "ShockPropagationAlpha"),
        "linear_break_threshold": number_after(text, "LinearBreakThreshold"),
        "angular_break_threshold": number_after(text, "AngularBreakThreshold"),
        "cone_soft": bool_after(cone, "bSoftConstraint", False),
        "twist_soft": bool_after(twist, "bSoftConstraint", False),
        "projection_enabled": bool_after(text, "bEnableProjection", False),
        "collision_disabled": bool_after(text, "bDisableCollision", False),
        "mass_conditioning_enabled": bool_after(text, "bEnableMassConditioning", False),
        "linear_joint_solver": bool_after(text, "bUseLinearJointSolver", False),
        "angular_drive_acceleration_mode": bool_after(angular_drive, "bAccelerationMode", False),
        "shock_propagation_enabled": bool_after(text, "bEnableShockPropagation", False),
        "parent_dominates": bool_after(text, "bParentDominates", False),
        "linear_breakable": bool_after(text, "bLinearBreakable", False),
        "angular_breakable": bool_after(text, "bAngularBreakable", False),
        "linear_plasticity": bool_after(text, "bLinearPlasticity", False),
        "angular_plasticity": bool_after(text, "bAngularPlasticity", False),
    }
    result["angular_drive_enabled"] = active_angular_drive(result)
    return result


def mode_limit(mode, degrees):
    if mode == "ACM_Locked":
        return 0.0
    if mode == "ACM_Limited":
        return math.radians(degrees)
    return None


def target_joint_variant(swing_1, swing_2, twist):
    if hinge_axis(swing_1, swing_2, twist) is not None:
        return "Revolute"
    return "Spherical"


def hinge_axis(swing_1, swing_2, twist):
    if twist != 0.0:
        return None
    if swing_1 is not None and swing_1 > 0.0 and swing_2 == 0.0:
        return [0.0, 1.0, 0.0]
    if swing_2 is not None and swing_2 > 0.0 and swing_1 == 0.0:
        return [0.0, 0.0, 1.0]
    return None


def hinge_limit(swing_1, swing_2, twist):
    if hinge_axis(swing_1, swing_2, twist) == [0.0, 1.0, 0.0]:
        return swing_1
    if hinge_axis(swing_1, swing_2, twist) == [0.0, 0.0, 1.0]:
        return swing_2
    return None


def native_swing_limit(swing_1, swing_2):
    if swing_1 is None or swing_2 is None:
        return None
    if not math.isclose(swing_1, swing_2, abs_tol=1e-3):
        return None
    return [-swing_1, swing_1]


def build_profile(source, project_max_angular_velocity_degrees):
    source_map = source_props(source)
    physics = source["physics_asset"]
    cvars = physics["constraint_console_variables"]
    limit_stiffness_scale = float(cvars["p.Chaos.JointConstraint.SoftAngularStiffnessScale"])
    limit_damping_scale = float(cvars["p.Chaos.JointConstraint.SoftAngularDampingScale"])
    linear_drive_stiffness_scale = float(cvars["p.Chaos.JointConstraint.LinearDriveStiffnessScale"])
    linear_drive_damping_scale = float(cvars["p.Chaos.JointConstraint.LinaearDriveDampingScale"])
    drive_stiffness_scale = float(cvars["p.Chaos.JointConstraint.AngularDriveStiffnessScale"])
    drive_damping_scale = float(cvars["p.Chaos.JointConstraint.AngularDriveDampingScale"])
    bodies = []
    for body in sorted(physics["bodies"], key=lambda item: item["index"]):
        if body["shape_count"] != len(body["shapes"]):
            raise ValueError(f"unsupported source collision geometry: {body['bone_name']}")
        bone = body["bone_name"]
        original = source_map[("body", bone)]
        default_instance = props(body["default_instance_properties"])
        shapes = []
        total = sum(volume(shape) for shape in body["shapes"])
        for index, shape in enumerate(body["shapes"]):
            item = dict(shape)
            item["source_shape_index"] = index
            item["target"] = target_shape(shape)
            item["target"]["collision_enabled"] = collision_enabled_intersection(
                default_instance["CollisionEnabled"], props(shape["properties"])["CollisionEnabled"])
            item["target"]["mass_kg"] = body["calculated_mass"] * volume(shape) / total
            shapes.append(item)
        physics_type = original["PhysicsType"]
        mode = "Kinematic" if physics_type == "PhysType_Kinematic" else "Dynamic"
        nudge = ue_point(vector_after("COMNudge=" + default_instance["COMNudge"], "COMNudge"))
        inertia_scale = direction(vector_after("InertiaTensorScale=" + default_instance["InertiaTensorScale"], "InertiaTensorScale"))
        mass_properties = target_mass_properties([shape["target"] for shape in shapes], body["calculated_mass"], nudge, inertia_scale,
            any(shape["target"]["collision_enabled"] in {"NoCollision", "ProbeOnly", "QueryAndProbe"} for shape in shapes))
        bodies.append({
            "source_index": body["index"],
            "bone_name": bone,
            "physics_type": physics_type,
            "collision_response": original["CollisionReponse"],
            "calculated_mass_kg": body["calculated_mass"],
            "source_properties": original,
            "source_default_instance": default_instance,
            "target": {
                "default_mode": mode,
                "active_mode": mode,
                "linear_damping": numeric(body["default_instance_properties"], "LinearDamping"),
                "angular_damping": numeric(body["default_instance_properties"], "AngularDamping"),
                "max_angular_speed_radians_per_second": target_max_angular_speed(body["default_instance_properties"], project_max_angular_velocity_degrees),
                "gyroscopic_torque_enabled": boolean(body["default_instance_properties"], "bGyroscopicTorqueEnabled"),
                "gravity_scale": 1.0 if boolean(body["default_instance_properties"], "bEnableGravity", True) else 0.0,
                "material": target_material(body["physical_material_properties"]),
                **({"mass_properties": mass_properties} if mass_properties is not None else {}),
            },
            "shapes": shapes,
        })
    constraints = []
    for constraint in sorted(physics["constraints"], key=lambda item: item["index"]):
        name = constraint["joint_name"]
        profile = parsed_profile(constraint["instance_properties"])
        swing_1 = mode_limit(profile["swing_1_motion"], profile["swing_1_limit_degrees"])
        swing_2 = mode_limit(profile["swing_2_motion"], profile["swing_2_limit_degrees"])
        twist = mode_limit(profile["twist_motion"], profile["twist_limit_degrees"])
        variant = target_joint_variant(swing_1, swing_2, twist)
        axis = hinge_axis(swing_1, swing_2, twist)
        limit = hinge_limit(swing_1, swing_2, twist)
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
                "twist_axis": [1.0, 0.0, 0.0],
                "swing_1_limit_radians": swing_1,
                "swing_2_limit_radians": swing_2,
                "swing_limit_radians": native_swing_limit(swing_1, swing_2),
                "twist_limit_radians": None if twist is None else [-twist, twist],
                "angular_limits": [target_angular_limit(profile[key], value) for key, value in (
                    ("twist_motion", twist), ("swing_1_motion", swing_1), ("swing_2_motion", swing_2),
                )],
                "joint_variant": variant,
                "hinge_axis": axis,
                "hinge_limit_radians": None if limit is None else [-limit, limit],
                "swing_spring": target_spring(profile["cone_stiffness"], profile["cone_damping"], True, limit_stiffness_scale, limit_damping_scale) if profile["cone_soft"] else None,
                "twist_spring": target_spring(profile["twist_stiffness"], profile["twist_damping"], True, limit_stiffness_scale, limit_damping_scale) if profile["twist_soft"] else None,
                "drive_mode": profile["angular_drive_mode"],
                "drive_target_xyzw": ue_rotator(profile["angular_drive_target_ue"]),
                "drive_velocity_target_radians_per_second": ue_angular_velocity(profile["angular_velocity_target"]),
                "drives": [
                    {
                        **target_spring(profile["angular_drives"][name]["stiffness"], profile["angular_drives"][name]["damping"], profile["angular_drive_acceleration_mode"], drive_stiffness_scale, drive_damping_scale),
                        "position_enabled": profile["angular_drives"][name]["position_enabled"],
                        "velocity_enabled": profile["angular_drives"][name]["velocity_enabled"],
                    } for name in ("TwistDrive", "SwingDrive", "SwingDrive")
                ],
                "angular_drive_stiffness_scale": drive_stiffness_scale * (1.0 if profile["angular_drive_acceleration_mode"] else CM_TO_M ** 2),
                "angular_drive_damping_scale": drive_damping_scale * (1.0 if profile["angular_drive_acceleration_mode"] else CM_TO_M ** 2),
                "mass_conditioning": profile["mass_conditioning_enabled"],
                "projection_linear_alpha": profile["projection_linear_alpha"] if profile["projection_enabled"] else 0.0,
                "projection_linear_tolerance_m": profile["projection_linear_tolerance_cm"] * CM_TO_M if profile["projection_enabled"] else 0.0,
                "projection_angular_alpha": profile["projection_angular_alpha"] if profile["projection_enabled"] else 0.0,
                "linear_locked": linear_locked,
                **target_linear_drive(profile, linear_drive_stiffness_scale, linear_drive_damping_scale),
                "enabled": linear_locked or swing_1 is not None or swing_2 is not None or twist is not None or profile["angular_drive_enabled"] or active_linear_drive(profile),
                "collision_disabled": profile["collision_disabled"],
            },
        })
    return {
        "format": "physics_asset_runtime_v1",
        "source_asset_path": source["asset_path"],
        "source_units_per_meter": 100.0,
        "target_axis_mapping": ["x", "z", "y"],
        "solver_settings": props(physics["solver_settings_properties"]),
        "target_solver": {
            "position_iterations": int(props(physics["solver_settings_properties"])["PositionIterations"]),
            "projection_iterations": int(props(physics["solver_settings_properties"])["ProjectionIterations"]),
            "min_parent_mass_ratio": float(cvars["p.Chaos.Solver.Joint.MinParentMassRatio"]),
            "max_inertia_ratio": float(cvars["p.Chaos.Solver.Joint.MaxInertiaRatio"]),
            "angle_tolerance_radians": float(cvars["p.Chaos.Solver.Joint.AngleTolerance"]),
            "position_tolerance_m": float(cvars["p.Chaos.Solver.Joint.PositionTolerance"]) * CM_TO_M,
            "projection_velocity_alpha": float(cvars["p.Chaos.Joint.VelProjectionAlpha"]),
        },
        "bodies": bodies,
        "constraints": constraints,
        "collision_disabled_pairs": collision_disabled_pairs(physics),
    }


def write_ragdoll(profile, rest, output):
    source_asset = profile["source_asset_path"]
    ids = {body["bone_name"]: stable_id(source_asset, "body", body["bone_name"]) for body in profile["bodies"]}
    collider_ids = {}
    for body in profile["bodies"]:
        for shape in body["shapes"]:
            key = f"{body['bone_name']}/{shape['source_shape_index']}"
            collider_ids[key] = stable_id(source_asset, "collider", key)
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
        membership, filtering = body_collision_layers(body["collision_response"])
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
                f"            shape: {geometry},", f"            layer_membership: {membership},", f"            layer_filter: {filtering},", "            override_layers: false,", f"            mass_mode: Override({text_number(target['mass_kg'])}),",
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
        joint = stable_id(source_asset, "joint", constraint["joint_name"])
        lines.extend([
            "        (", f'            uuid: "{joint}",', "        ): (", "            id: (", f'                uuid: "{joint}",', "            ),",
            f'            label: "{constraint["joint_name"]}",', "            use_symmetry: false,", "            created_from: None,",
        ])
        if target["joint_variant"] == "Revolute":
            hinge = target["hinge_limit_radians"]
            hinge_text = f"Some((min: {text_number(hinge[0])}, max: {text_number(hinge[1])}))"
            lines.extend([
                "            variant: Revolute((",
                "                body1: (", f'                    uuid: "{ids[body_1]}",', "                ),", "                body2: (", f'                    uuid: "{ids[body_2]}",', "                ),",
                f"                position: {text_vec(translation(rest, body_1))},", f"                hinge_axis: {text_vec(target['hinge_axis'])},", f"                angle_limit: {hinge_text},",
                "                point_compliance: 0,", "                align_compliance: 0,", "                limit_compliance: 0,", "            )),",
            ])
        else:
            lines.extend([
                "            variant: Spherical((",
                "                body1: (", f'                    uuid: "{ids[body_1]}",', "                ),", "                body2: (", f'                    uuid: "{ids[body_2]}",', "                ),",
                f"                position: {text_vec(translation(rest, body_1))},", f"                twist_axis: {text_vec(target['twist_axis'])},", f"                swing_limit: {swing_text},", f"                twist_limit: {twist_text},",
                "                point_compliance: 0,", "                swing_compliance: 0,", "                twist_compliance: 0,", "            )),",
            ])
        lines.append("        ),")
    total_mass = sum(body["calculated_mass_kg"] for body in profile["bodies"])
    lines.extend(["    },", "    suffixes: (", '        original: ".R",', '        mirror: ".L",', "    ),", f"    total_mass: {text_number(total_mass)},", ")", ""])
    output.write_text("\n".join(lines), encoding="utf-8")
    return ids


def write_bone_map(profile, document, rest, names, parents, ids, output, skeleton_asset, ragdoll_asset):
    mapping = {body["bone_name"]: body["bone_name"] for body in profile["bodies"]}
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
    lines.extend(["    },", f'    skeleton: {json.dumps(skeleton_asset)},', f'    ragdoll: {json.dumps(ragdoll_asset)},', "    skeleton_symmetry: (", "        name_mapper: Pattern((", '            key_1: "_l",', '            key_2: "_r",', '            pattern_before: "^.*",', '            pattern_after: "$",', "        )),", "        mode: MirrorX,", "    ),", ")", ""])
    output.write_text("\n".join(lines), encoding="utf-8")


def validate_native_reference(mesh, rest, body_names):
    native = mesh.get("skeletal_mesh")
    if not native:
        raise ValueError("native reader lacks the SkeletalMesh reference skeleton")
    bones = {bone["name"]: bone for bone in native["bones"]}
    for name in body_names:
        source = bones[name]["component_transform"]
        assert_close([source["scale"][key] for key in "xyz"], [1.0, 1.0, 1.0])
        assert_close(ue_point([source["translation"][key] for key in "xyz"]), translation(rest, name), 2e-5)
        rotation = ue_quat_to_bevy([source["rotation"][key] for key in "xyzw"])
        if abs(sum(left * right for left, right in zip(rotation, matrix_quat(rest[name])))) < 1.0 - 1e-5:
            raise ValueError(f"runtime skeleton reference rotation differs from source: {name}")


def generate(source, mesh, gltf, asset_root, skeleton_path, ragdoll_path, bone_map_path, profile_path, project_max_angular_velocity_degrees):
    if source.get("asset_class") != "/Script/Engine.PhysicsAsset" or "physics_asset" not in source:
        raise ValueError("source input is not a PhysicsAsset")
    profile = build_profile(source, project_max_angular_velocity_degrees)
    document, rest, names, parents = load_skeleton(gltf)
    missing = [body["bone_name"] for body in profile["bodies"] if body["bone_name"] not in rest]
    if missing:
        raise ValueError(f"missing skeleton bones: {missing}")
    validate_native_reference(mesh, rest, [body["bone_name"] for body in profile["bodies"]])
    verify_constraint_rest_frames(profile, rest)
    skeleton_asset = skeleton_path.resolve().relative_to(asset_root.resolve()).as_posix()
    ragdoll_asset = ragdoll_path.resolve().relative_to(asset_root.resolve()).as_posix()
    for body in profile["bodies"]:
        if any(shape["kind"] not in {"sphere", "box", "capsule"} for shape in body["shapes"]):
            raise ValueError(f"unsupported shape on body: {body['bone_name']}")
        if re.search(r"(?:ConvexElems|TaperedCapsuleElems|LevelSetElems|SkinnedLevelSetElems)=\(", body["source_properties"].get("AggGeom", "")):
            raise ValueError(f"unsupported aggregate geometry on body: {body['bone_name']}")
        for shape in body["shapes"]:
            values = props(shape["properties"])
            if float(values["RestOffset"]) != 0.0 or values["bContributeToMass"] != "True":
                raise ValueError(f"unsupported primitive mass or rest offset: {body['bone_name']}")
    for joint in profile["constraints"]:
        fields = joint["source_profile"]
        linear = {fields[key] for key in ("linear_x_motion", "linear_y_motion", "linear_z_motion")}
        if linear not in ({"LCM_Locked"}, {"LCM_Free"}):
            raise ValueError(f"unsupported linear constraint modes: {joint['joint_name']}")
        pair = (fields["swing_1_motion"], fields["swing_2_motion"])
        if joint["target"]["joint_variant"] != "Revolute" and pair not in {
            ("ACM_Free", "ACM_Free"), ("ACM_Locked", "ACM_Locked"),
            ("ACM_Limited", "ACM_Limited"), ("ACM_Limited", "ACM_Locked"),
        }:
            raise ValueError(f"unsupported independent swing modes: {joint['joint_name']}")
        for feature in ("linear_breakable", "angular_breakable", "linear_plasticity", "angular_plasticity", "parent_dominates", "shock_propagation_enabled"):
            if fields[feature]:
                raise ValueError(f"unsupported active constraint feature: {joint['joint_name']}:{feature}")
        active_drive = any(drive["position_enabled"] or drive["velocity_enabled"] for drive in fields["angular_drives"].values())
        if active_drive and fields["angular_drive_mode"] != "TwistAndSwing":
            raise ValueError(f"unsupported angular drive mode: {joint['joint_name']}")
        if fields["projection_enabled"] and fields["projection_angular_alpha"] != 0.0:
            raise ValueError(f"unsupported angular projection: {joint['joint_name']}")
        if any(drive["max_force"] != 0.0 for drive in fields["angular_drives"].values()):
            raise ValueError(f"unsupported capped angular drive: {joint['joint_name']}")
    with tempfile.TemporaryDirectory() as temporary:
        staged = Path(temporary)
        ids = write_ragdoll(profile, rest, staged / "ragdoll.ron")
        write_bone_map(profile, document, rest, names, parents, ids, staged / "map.ron", skeleton_asset, ragdoll_asset)
        (staged / "profile.json").write_text(json.dumps(profile, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        for name, destination in (("ragdoll.ron", ragdoll_path), ("map.ron", bone_map_path), ("profile.json", profile_path)):
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes((staged / name).read_bytes())
    return {
        "source_asset": source["asset_path"],
        "source_mesh": mesh["asset_path"],
        "bodies": len(profile["bodies"]),
        "colliders": sum(len(body["shapes"]) for body in profile["bodies"]),
        "constraints": len(profile["constraints"]),
        "collision_disabled_pairs": len(profile["collision_disabled_pairs"]),
        "outputs": [path.resolve().relative_to(asset_root.resolve()).as_posix() for path in (ragdoll_path, bone_map_path, profile_path)],
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--uasset", type=Path)
    parser.add_argument("--content-root", type=Path)
    parser.add_argument("--mount", default="/ALS")
    parser.add_argument("--asset-root", type=Path, default=ROOT / "assets")
    parser.add_argument("--skeleton", type=Path)
    parser.add_argument("--ragdoll-output", type=Path)
    parser.add_argument("--bone-map-output", type=Path)
    parser.add_argument("--profile-output", type=Path)
    parser.add_argument("--report", type=Path, default=ROOT / "target" / "physics_asset_conversion.report.json")
    parser.add_argument("--source-reader", type=Path, default=ROOT / "tools" / "run_ue_source_reader.ps1")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    self_test()
    if args.self_test:
        print("PhysicsAsset conversion self-test PASS")
        return
    report = {"format": "physics_asset_conversion_report_v1", "success": False}
    try:
        if args.uasset is None:
            raise ValueError("uasset input is required")
        sys.path.insert(0, str(ROOT / "tools" / "animgraph_authoring"))
        from ue_animgraph_converter import content_root_for_uasset, read_uasset_source
        content_root = content_root_for_uasset(args.uasset, args.content_root)
        print(json.dumps({"progress": "read_physics_asset", "asset": args.uasset.name}), flush=True)
        source, _ = read_uasset_source(args.uasset, content_root, args.mount, args.source_reader, 300, 150)
        preview = props(source["asset_properties"]).get("PreviewSkeletalMesh", "")
        prefix = args.mount.rstrip("/") + "/"
        if not preview.startswith(prefix):
            raise ValueError("PhysicsAsset preview mesh is missing or belongs to another mount")
        mesh_uasset = content_root / (preview.split(".")[0][len(prefix):] + ".uasset")
        print(json.dumps({"progress": "read_skeletal_mesh", "asset": mesh_uasset.name}), flush=True)
        mesh, _ = read_uasset_source(mesh_uasset, content_root, args.mount, args.source_reader, 300, 150)
        skeleton = args.skeleton or args.asset_root / "skeletons" / (mesh_uasset.stem + ".skn.ron")
        match = re.search(r'source:\s*"([^"]+\.gltf)"', skeleton.read_text(encoding="utf-8"))
        if not match:
            raise ValueError("runtime skeleton does not reference a supported glTF source")
        name = args.uasset.stem
        report.update(generate(
            source, mesh, args.asset_root / match.group(1), args.asset_root, skeleton,
            args.ragdoll_output or args.asset_root / "ragdolls" / (name + ".rag.ron"),
            args.bone_map_output or args.asset_root / "ragdoll_bone_maps" / (name + ".bm.ron"),
            args.profile_output or args.asset_root / "ragdolls" / (name + ".physics.json"),
            project_max_angular_velocity_degrees(args.uasset),
        ))
        report["success"] = True
    except (ValueError, KeyError, OSError, AssertionError, RuntimeError, StopIteration) as error:
        report["error"] = str(error)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report))
    if not report["success"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
