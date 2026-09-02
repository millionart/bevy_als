import argparse
import hashlib
import json
import math
import re
from pathlib import Path

from coordinate_basis import (
    SOURCE_LENGTH_TO_RUNTIME_SCALE,
    source_direction_to_runtime,
    source_point_to_runtime,
    source_rotation_to_runtime,
)


ROOT = Path(__file__).resolve().parents[2]
DEFAULT_SOURCE = ROOT / "target" / "bf109" / "source_native" / "CR_Als.ue.json"
DEFAULT_MESH = ROOT / "target" / "bf109" / "source_native" / "SKM_Als.ue.json"
DEFAULT_GLTF = ROOT / "assets" / "animations" / "Base" / "A_Als_Idle.gltf"
DEFAULT_PROFILE = ROOT / "assets" / "control_rigs" / "cr_als.profile.json"
DEFAULT_RUNTIME_PROFILE = ROOT / "assets" / "control_rigs" / "cr_als.runtime.json"
DEFAULT_REPORT = ROOT / "target" / "bf109" / "audits" / "CR_Als.audit.json"
TOLERANCE = 3e-5
APPLY_CHAIN_PATTERN = re.compile(r"fn apply_chain_target\([\s\S]*?\n}\n\nfn source_foot_ik_axes")


STAGE_ANCHORS = {
    "Rig Graph": [
        "refresh_control_rig_bones",
        "refresh_control_rig_input",
        "control_rig_pre_apply_system",
        "control_rig_hand_apply_system",
    ],
    "RefreshDiagonalScaling": ["diagonal_scale_factor", "apply_diagonal_scaling"],
    "RefreshSpineRotation": ["apply_spine_rotation", "armature_yaw_rotation"],
    "RefreshFootOffset": ["source_foot_trace_sample", "source_trace_offset"],
    "RefreshPelvisOffset": ["spring_damper_float", "pelvis_offset_y"],
    "RefreshFootIk": [
        "source_apply_foot_offset_location",
        "source_foot_rotation",
        "source_constrain_foot_lock",
    ],
    "ApplyFootIk": [
        "source_apply_foot_offset_location",
        "source_foot_rotation",
        "apply_chain_target",
        "solve_basic_two_bone_ik",
        "source_foot_ik_axes",
        "SourcePoleState",
        "source_constrain_foot_lock",
    ],
    "RefreshHandIk": ["apply_arm_ik"],
    "TraceFootOffset": ["source_foot_trace_sample", "source_trace_offset"],
    "RigVMFunctionLibraryEdGraph": ["execution_order"],
}


def props(entries):
    return {entry["name"]: entry["exported_text"] for entry in entries}


def pin_default(node, suffix):
    values = [pin["default_value"] for pin in node["pins"] if pin["name"].endswith(suffix)]
    if len(values) != 1:
        raise ValueError(f"missing source pin {suffix}")
    return values[0]


def foot_ik_call_axes(source):
    graph = next((graph for graph in source["graphs"] if graph["name"] == "RefreshFootIk"), None)
    if graph is None:
        raise ValueError("missing RefreshFootIk source graph")
    result = {}
    for node in graph["nodes"]:
        if "ApplyFootIk" not in node["title"]:
            continue
        thigh = pin_default(node, "ThighItem.Name")
        if thigh not in {"thigh_l", "thigh_r"}:
            continue
        result[thigh] = {
            "primary": [
                float(pin_default(node, f"PrimaryAxis.{axis}"))
                for axis in ("X", "Y", "Z")
            ],
            "secondary": [
                float(pin_default(node, f"SecondaryAxis.{axis}"))
                for axis in ("X", "Y", "Z")
            ],
        }
    if set(result) != {"thigh_l", "thigh_r"}:
        raise ValueError("source ApplyFootIk side call axes are incomplete")
    return result


def source_nodes(source):
    rows = []
    for graph in source["graphs"]:
        for node in graph["nodes"]:
            rows.append(
                {
                    "graph": graph["name"],
                    "index": node["index"],
                    "model": props(node["properties"]).get("ModelNodePath", ""),
                    "title": node["title"],
                    "position": [node["position_x"], node["position_y"]],
                    "pin_count": len(node["pins"]),
                }
            )
    return rows


def source_summary(source):
    return {
        graph["name"]: {
            "node_count": len(graph["nodes"]),
            "pin_count": sum(len(node["pins"]) for node in graph["nodes"]),
        }
        for graph in source["graphs"]
    }


def rig_vm_links(source):
    return [
        item
        for item in source["asset_objects"]
        if item["class_path"] == "/Script/RigVMDeveloper.RigVMLink"
    ]


def canonical_hash(value):
    encoded = json.dumps(value, ensure_ascii=False, separators=(",", ":"), sort_keys=True).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def vector(value):
    return [float(value[axis]) for axis in ("x", "y", "z")]


def quaternion(value):
    return [float(value[axis]) for axis in ("x", "y", "z", "w")]


def distance(left, right):
    return math.sqrt(sum((a - b) ** 2 for a, b in zip(left, right)))


def quat_alignment(left, right):
    return abs(sum(a * b for a, b in zip(left, right)))


def gltf_value(node, key, default):
    value = node.get(key)
    return list(value) if value is not None else list(default)


def joint_nodes(gltf):
    joints = gltf["skins"][0]["joints"]
    nodes = gltf["nodes"]
    parents = {}
    for parent_index, node in enumerate(nodes):
        for child in node.get("children", []):
            parents[child] = parent_index
    result = {}
    for index in joints:
        name = nodes[index].get("name")
        if name in result:
            raise ValueError(f"duplicate joint name {name}")
        result[name] = (index, nodes[index], parents.get(index))
    return result


def validate_source_raw(source, differences):
    if source.get("asset_class") != "/Script/ControlRigDeveloper.ControlRigBlueprint":
        differences.append("source asset class")
    if len(source.get("graphs", [])) != 10:
        differences.append("source graph count")
    if len(source.get("asset_objects", [])) != 1948:
        differences.append("source asset object count")
    cdo = props(source.get("blueprint_cdo_properties", []))
    if cdo.get("SpineRotationBones") != '((Type=Bone,Name="pelvis"),(Type=Bone,Name="spine_01"),(Type=Bone,Name="spine_02"),(Type=Bone,Name="spine_03"))':
        differences.append("source spine CDO")
    object_classes = {}
    for item in source.get("asset_objects", []):
        object_classes[item["class_path"]] = object_classes.get(item["class_path"], 0) + 1
    if object_classes.get("/Script/RigVMDeveloper.RigVMPin") != 1407:
        differences.append("source RigVM pin count")
    if object_classes.get("/Script/RigVMDeveloper.RigVMLink") != 160:
        differences.append("source RigVM link count")
    for graph in source.get("graphs", []):
        for node in graph.get("nodes", []):
            if not isinstance(node.get("position_x"), int) or not isinstance(node.get("position_y"), int):
                differences.append(f"source node position {graph['name']}:{node['index']}")
            for pin in node.get("pins", []):
                for field in ("name", "direction", "default_value", "linked_to", "pin_type_properties"):
                    if field not in pin:
                        differences.append(f"source pin field {graph['name']}:{node['index']}:{field}")
    for item in source.get("asset_objects", []):
        if item.get("class_path") != "/Script/RigVMDeveloper.RigVMLink":
            continue
        values = props(item["properties"])
        if not values.get("SourcePinPath") or not values.get("TargetPinPath"):
            differences.append(f"source RigVM link {item['object_path']}")
    try:
        foot_ik_call_axes(source)
    except ValueError as error:
        differences.append(str(error))


def validate_profile(source, profile, runtime, control_source, foot_source, differences):
    if profile.get("source_asset") != source.get("asset_path"):
        differences.append("profile source asset")
    if profile.get("source_blueprint_cdo_properties") != source.get("blueprint_cdo_properties"):
        differences.append("profile source blueprint CDO")
    if profile.get("source_graphs") != source.get("graphs"):
        differences.append("profile source graph contract")
    if profile.get("source_rig_vm_links") != rig_vm_links(source):
        differences.append("profile source RigVM links")
    source_foot_ik_axes = foot_ik_call_axes(source)
    if profile.get("source_foot_ik_call_axes") != source_foot_ik_axes:
        differences.append("profile source foot ik call axes")
    source_rows = source_nodes(source)
    if profile.get("source_node_layout") != source_rows:
        differences.append("profile source node layout")
    if profile.get("source_graph_summary") != source_summary(source):
        differences.append("profile graph summary")
    source_construction = profile.get("source_construction", {})
    if source_construction.get("spine_rotation_bones") != ["pelvis", "spine_01", "spine_02", "spine_03"]:
        differences.append("profile spine bones")
    if profile.get("runtime_asset") != "assets/control_rigs/cr_als.runtime.json":
        differences.append("profile runtime asset")
    construction = runtime.get("construction", {})
    if construction != {
        key: value
        for key, value in source_construction.items()
        if key
        not in {
            "foot_height_cm",
            "leg_length_cm",
            "foot_left_thigh_axis_pelvis_space_source",
            "foot_right_thigh_axis_pelvis_space_source",
        }
    }:
        differences.append("runtime construction materialization")
    if runtime.get("execution_order") != [
        "RefreshDiagonalScaling",
        "RefreshSpineRotation",
        "RefreshFootOffset",
        "RefreshPelvisOffset",
        "RefreshFootIk",
        "RefreshHandIk",
    ]:
        differences.append("profile execution order")
    target = runtime.get("target", {})
    if abs(target.get("foot_height_m", 0.0) - source_construction.get("foot_height_cm", 0.0) * SOURCE_LENGTH_TO_RUNTIME_SCALE) > TOLERANCE:
        differences.append("profile foot height conversion")
    if abs(target.get("leg_length_m", 0.0) - source_construction.get("leg_length_cm", 0.0) * SOURCE_LENGTH_TO_RUNTIME_SCALE) > TOLERANCE:
        differences.append("profile leg length conversion")
    expected_vectors = {
        "diagonal_scale_delta": source_direction_to_runtime([0.4, 0.4, 0.0]),
        "hand_left_pole_location_m": source_point_to_runtime([0.0, -30.0, 0.0]),
        "hand_right_pole_location_m": source_point_to_runtime([0.0, 30.0, 0.0]),
        "foot_left_primary_axis": source_direction_to_runtime(
            source_foot_ik_axes["thigh_l"]["primary"]
        ),
        "foot_left_secondary_axis": source_direction_to_runtime(
            source_foot_ik_axes["thigh_l"]["secondary"]
        ),
        "foot_right_primary_axis": source_direction_to_runtime(
            source_foot_ik_axes["thigh_r"]["primary"]
        ),
        "foot_right_secondary_axis": source_direction_to_runtime(
            source_foot_ik_axes["thigh_r"]["secondary"]
        ),
        "foot_left_thigh_axis_pelvis_space": source_direction_to_runtime(
            source_construction.get("foot_left_thigh_axis_pelvis_space_source", [math.inf] * 3)
        ),
        "foot_right_thigh_axis_pelvis_space": source_direction_to_runtime(
            source_construction.get("foot_right_thigh_axis_pelvis_space_source", [math.inf] * 3)
        ),
    }
    for name, expected in expected_vectors.items():
        if distance(target.get(name, [math.inf] * 3), expected) > TOLERANCE:
            differences.append(f"profile vector {name}")
    for name in ("source_point_to_runtime", "source_direction_to_runtime", "source_rotation_to_runtime"):
        if name in control_source or name in foot_source:
            differences.append(f"runtime coordinate bridge {name}")
    if "AlsGroundedVelocityBlend" not in control_source:
        differences.append("control rig grounded velocity blend source")
    chain_match = APPLY_CHAIN_PATTERN.search(foot_source)
    if chain_match is None:
        differences.append("control rig foot ik target stage")
    else:
        chain_source = chain_match.group(0)
        if "constrain_foot_lock_location" in chain_source:
            differences.append("control rig foot ik applies lock constraint twice")
        if "limit_leg_stretch" in chain_source:
            differences.append("control rig foot ik applies stretch limit twice")
        if "source_foot_ik_axes(side, &profile.0.target)" not in foot_source:
            differences.append("control rig foot ik side call axes")
        if "source_basic_ik_writeback" not in chain_source:
            differences.append("control rig foot ik global writeback")
        if "source_local_transform" not in foot_source:
            differences.append("control rig foot ik hierarchy writeback")
    coverage = []
    for row in source_rows:
        anchors = STAGE_ANCHORS.get(row["graph"])
        if anchors is None:
            differences.append(f"unmapped graph {row['graph']}")
            anchors = []
        code = control_source if row["graph"] in {"Rig Graph", "RefreshDiagonalScaling", "RefreshSpineRotation", "RefreshHandIk", "RigVMFunctionLibraryEdGraph"} else foot_source
        missing = [anchor for anchor in anchors if anchor not in code]
        if missing:
            differences.append(f"unmapped node {row['graph']}:{row['index']}:{','.join(missing)}")
        coverage.append(
            {
                "graph": row["graph"],
                "source_index": row["index"],
                "source_model": row["model"],
                "source_position": row["position"],
                "target_position": row["position"],
                "target_anchors": anchors,
                "missing_anchors": missing,
            }
        )
    return coverage


def validate_skeleton(mesh, gltf, differences):
    source_mesh = mesh.get("skeletal_mesh", {})
    source_bones = source_mesh.get("bones", [])
    if source_mesh.get("raw_bone_count") != 68 or source_mesh.get("bone_count") != 79:
        differences.append("source skeletal mesh counts")
    if len(source_mesh.get("virtual_bones", [])) != 11:
        differences.append("source virtual bone count")
    joints = joint_nodes(gltf)
    source_names = {bone["name"] for bone in source_bones}
    if source_names != set(joints):
        differences.append("skeleton joint names")
    by_index = {bone["index"]: bone for bone in source_bones}
    for bone in source_bones:
        name = bone["name"]
        if name not in joints:
            continue
        _, node, target_parent_index = joints[name]
        source_local = bone["local_transform"]
        expected_translation = source_point_to_runtime(vector(source_local["translation"]))
        expected_rotation = source_rotation_to_runtime(quaternion(source_local["rotation"]))
        expected_scale = source_direction_to_runtime(vector(source_local["scale"]))
        actual_translation = gltf_value(node, "translation", [0.0, 0.0, 0.0])
        actual_rotation = gltf_value(node, "rotation", [0.0, 0.0, 0.0, 1.0])
        actual_scale = gltf_value(node, "scale", [1.0, 1.0, 1.0])
        if distance(actual_translation, expected_translation) > TOLERANCE:
            differences.append(f"skeleton translation {name}")
        if quat_alignment(actual_rotation, expected_rotation) < 1.0 - TOLERANCE:
            differences.append(f"skeleton rotation {name}")
        if distance(actual_scale, expected_scale) > TOLERANCE:
            differences.append(f"skeleton scale {name}")
        source_parent = bone["parent_index"]
        if source_parent < 0:
            continue
        expected_parent_name = by_index[source_parent]["name"]
        actual_parent_name = gltf["nodes"][target_parent_index].get("name") if target_parent_index is not None else None
        if actual_parent_name != expected_parent_name:
            differences.append(f"skeleton parent {name}")


def self_test():
    if source_point_to_runtime([100.0, 200.0, 300.0]) != [1.0, 3.0, 2.0]:
        raise AssertionError("point conversion")
    if source_direction_to_runtime([1.0, 2.0, 3.0]) != [1.0, 3.0, 2.0]:
        raise AssertionError("scale conversion")
    if source_rotation_to_runtime([0.2, -0.3, 0.4, 0.5]) != [-0.2, -0.4, 0.3, 0.5]:
        raise AssertionError("quaternion conversion")
    source = {
        "graphs": [
            {
                "name": "RefreshFootIk",
                "nodes": [
                    {
                        "title": "ApplyFootIk",
                        "pins": [
                            {"name": "ApplyFootIk.ThighItem.Name", "default_value": "thigh_l"},
                            {"name": "ApplyFootIk.PrimaryAxis.X", "default_value": "-1.000000"},
                            {"name": "ApplyFootIk.PrimaryAxis.Y", "default_value": "0.000000"},
                            {"name": "ApplyFootIk.PrimaryAxis.Z", "default_value": "0.000000"},
                            {"name": "ApplyFootIk.SecondaryAxis.X", "default_value": "0.000000"},
                            {"name": "ApplyFootIk.SecondaryAxis.Y", "default_value": "1.000000"},
                            {"name": "ApplyFootIk.SecondaryAxis.Z", "default_value": "0.000000"},
                        ],
                    },
                    {
                        "title": "ApplyFootIk",
                        "pins": [
                            {"name": "ApplyFootIk_1.ThighItem.Name", "default_value": "thigh_r"},
                            {"name": "ApplyFootIk_1.PrimaryAxis.X", "default_value": "1.000000"},
                            {"name": "ApplyFootIk_1.PrimaryAxis.Y", "default_value": "0.000000"},
                            {"name": "ApplyFootIk_1.PrimaryAxis.Z", "default_value": "0.000000"},
                            {"name": "ApplyFootIk_1.SecondaryAxis.X", "default_value": "0.000000"},
                            {"name": "ApplyFootIk_1.SecondaryAxis.Y", "default_value": "-1.000000"},
                            {"name": "ApplyFootIk_1.SecondaryAxis.Z", "default_value": "0.000000"},
                        ],
                    },
                ],
            }
        ]
    }
    axes = foot_ik_call_axes(source)
    if axes["thigh_l"]["secondary"] != [0.0, 1.0, 0.0]:
        raise AssertionError("left call secondary axis")
    if axes["thigh_r"]["primary"] != [1.0, 0.0, 0.0]:
        raise AssertionError("right call primary axis")
    if APPLY_CHAIN_PATTERN.search("fn apply_chain_target()\n{\n}\n\nfn source_foot_ik_axes") is None:
        raise AssertionError("foot ik writeback boundary")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, default=DEFAULT_SOURCE)
    parser.add_argument("--mesh", type=Path, default=DEFAULT_MESH)
    parser.add_argument("--gltf", type=Path, default=DEFAULT_GLTF)
    parser.add_argument("--profile", type=Path, default=DEFAULT_PROFILE)
    parser.add_argument("--runtime-profile", type=Path, default=DEFAULT_RUNTIME_PROFILE)
    parser.add_argument("--report", type=Path, default=DEFAULT_REPORT)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    self_test()
    if args.self_test:
        print("CR_Als audit self-test PASS")
        return
    source = json.loads(args.source.read_text(encoding="utf-8"))
    mesh = json.loads(args.mesh.read_text(encoding="utf-8"))
    gltf = json.loads(args.gltf.read_text(encoding="utf-8"))
    profile = json.loads(args.profile.read_text(encoding="utf-8"))
    runtime = json.loads(args.runtime_profile.read_text(encoding="utf-8"))
    differences = []
    validate_source_raw(source, differences)
    coverage = validate_profile(
        source,
        profile,
        runtime,
        (ROOT / "src" / "control_rig_apply.rs").read_text(encoding="utf-8"),
        (ROOT / "src" / "foot_ik_apply.rs").read_text(encoding="utf-8"),
        differences,
    )
    validate_skeleton(mesh, gltf, differences)
    report = {
        "source_asset": source["asset_path"],
        "source_mesh": mesh["asset_path"],
        "source_graphs": source_summary(source),
        "source_nodes": len(source_nodes(source)),
        "source_rig_vm_pins": sum(
            item["class_path"] == "/Script/RigVMDeveloper.RigVMPin"
            for item in source["asset_objects"]
        ),
        "source_rig_vm_links": sum(
            item["class_path"] == "/Script/RigVMDeveloper.RigVMLink"
            for item in source["asset_objects"]
        ),
        "source_graph_contract_sha256": canonical_hash(source["graphs"]),
        "source_rig_vm_links_sha256": canonical_hash(rig_vm_links(source)),
        "source_skeleton_bones": mesh["skeletal_mesh"]["bone_count"],
        "source_virtual_bones": len(mesh["skeletal_mesh"]["virtual_bones"]),
        "coverage": coverage,
        "differences": differences,
        "pass": not differences,
    }
    if not args.check:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(
        "CR_Als audit "
        f"nodes={report['source_nodes']} "
        f"pins={report['source_rig_vm_pins']} "
        f"links={report['source_rig_vm_links']} "
        f"bones={report['source_skeleton_bones']} "
        f"virtual_bones={report['source_virtual_bones']} "
        f"differences={len(differences)} "
        f"-> {'PASS' if not differences else 'FAIL'}"
    )
    if differences:
        for value in differences:
            print(f"DIFF {value}")
        raise SystemExit(1)


if __name__ == "__main__":
    main()
