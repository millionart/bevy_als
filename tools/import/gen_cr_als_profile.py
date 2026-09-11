import argparse
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
DEFAULT_OUTPUT = ROOT / "assets" / "control_rigs" / "cr_als.profile.json"
DEFAULT_RUNTIME_OUTPUT = ROOT / "assets" / "control_rigs" / "cr_als.runtime.json"


def props(entries):
    return {entry["name"]: entry["exported_text"] for entry in entries}


def target_vector(values):
    return [float(f"{value:.9g}") for value in values]


def vector_from_fields(value):
    return [float(value[axis]) for axis in ("x", "y", "z")]


def quat_from_fields(value):
    return [float(value[axis]) for axis in ("x", "y", "z", "w")]


def quat_conjugate(value):
    return [-value[0], -value[1], -value[2], value[3]]


def quat_multiply(left, right):
    lx, ly, lz, lw = left
    rx, ry, rz, rw = right
    return [
        lw * rx + lx * rw + ly * rz - lz * ry,
        lw * ry - lx * rz + ly * rw + lz * rx,
        lw * rz + lx * ry - ly * rx + lz * rw,
        lw * rw - lx * rx - ly * ry - lz * rz,
    ]


def parse_spine_bones(text):
    return re.findall(r'Name="([^"]+)"', text)


def graph_node_layout(source):
    result = []
    for graph in source["graphs"]:
        for node in graph["nodes"]:
            result.append(
                {
                    "graph": graph["name"],
                    "index": node["index"],
                    "model": props(node["properties"]).get("ModelNodePath", ""),
                    "title": node["title"],
                    "position": [node["position_x"], node["position_y"]],
                    "pin_count": len(node["pins"]),
                }
            )
    return result


def graph_summary(source):
    result = {}
    for graph in source["graphs"]:
        result[graph["name"]] = {
            "node_count": len(graph["nodes"]),
            "pin_count": sum(len(node["pins"]) for node in graph["nodes"]),
        }
    return result


def rig_vm_links(source):
    return [
        item
        for item in source["asset_objects"]
        if item["class_path"] == "/Script/RigVMDeveloper.RigVMLink"
    ]


def bone_map(mesh):
    return {bone["name"]: bone for bone in mesh["skeletal_mesh"]["bones"]}


def chain_length_cm(bones, ancestor, descendant):
    by_index = {bone["index"]: bone for bone in bones.values()}
    current = bones[descendant]
    length = 0.0
    while current["name"] != ancestor:
        local = vector_from_fields(current["local_transform"]["translation"])
        length += math.sqrt(sum(component * component for component in local))
        parent_index = current["parent_index"]
        if parent_index not in by_index:
            raise ValueError(f"missing parent for {current['name']}")
        current = by_index[parent_index]
    return length


def normalized_vector(values):
    length = math.sqrt(sum(component * component for component in values))
    if length <= 1e-12:
        raise ValueError("source thigh axis is zero")
    return [component / length for component in values]


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


def construction(source, mesh):
    cdo = props(source["blueprint_cdo_properties"])
    bones = bone_map(mesh)
    return {
        "spine_rotation_bones": parse_spine_bones(cdo["SpineRotationBones"]),
        "foot_root_if_ik": "ik_foot_root",
        "foot_root_if_virtual": "VB foot_root",
        "hand_left_if_ik": "VB ik_hand_l",
        "hand_left_if_virtual": "VB hand_l",
        "hand_right_if_ik": "VB ik_hand_r",
        "hand_right_if_virtual": "VB hand_r",
        "foot_height_cm": bones["foot_l"]["component_transform"]["translation"]["z"],
        "leg_length_cm": chain_length_cm(bones, "thigh_l", "foot_l"),
        "foot_left_thigh_axis_pelvis_space_source": normalized_vector(
            vector_from_fields(bones["thigh_l"]["local_transform"]["translation"])
        ),
        "foot_right_thigh_axis_pelvis_space_source": normalized_vector(
            vector_from_fields(bones["thigh_r"]["local_transform"]["translation"])
        ),
    }


def build(source, mesh):
    source_construction = construction(source, mesh)
    source_foot_ik_axes = foot_ik_call_axes(source)
    source_bones = bone_map(mesh)
    left_initial_calf_space = quat_multiply(
        quat_conjugate(quat_from_fields(source_bones["calf_l"]["component_transform"]["rotation"])),
        quat_from_fields(source_bones["foot_l"]["component_transform"]["rotation"]),
    )
    right_initial_calf_space = quat_multiply(
        quat_conjugate(quat_from_fields(source_bones["calf_r"]["component_transform"]["rotation"])),
        quat_from_fields(source_bones["foot_r"]["component_transform"]["rotation"]),
    )
    target = {
        **foot_reference_segments(source),
        "foot_height_m": source_construction["foot_height_cm"] * SOURCE_LENGTH_TO_RUNTIME_SCALE,
        "leg_length_m": source_construction["leg_length_cm"] * SOURCE_LENGTH_TO_RUNTIME_SCALE,
        "diagonal_scale_delta": target_vector(source_direction_to_runtime([0.4, 0.4, 0.0])),
        "hand_left_pole_location_m": target_vector(source_point_to_runtime([0.0, -30.0, 0.0])),
        "hand_right_pole_location_m": target_vector(source_point_to_runtime([0.0, 30.0, 0.0])),
        "foot_left_primary_axis": target_vector(
            source_direction_to_runtime(source_foot_ik_axes["thigh_l"]["primary"])
        ),
        "foot_left_secondary_axis": target_vector(
            source_direction_to_runtime(source_foot_ik_axes["thigh_l"]["secondary"])
        ),
        "foot_right_primary_axis": target_vector(
            source_direction_to_runtime(source_foot_ik_axes["thigh_r"]["primary"])
        ),
        "foot_right_secondary_axis": target_vector(
            source_direction_to_runtime(source_foot_ik_axes["thigh_r"]["secondary"])
        ),
        "foot_left_thigh_axis_pelvis_space": target_vector(
            source_direction_to_runtime(source_construction["foot_left_thigh_axis_pelvis_space_source"])
        ),
        "foot_right_thigh_axis_pelvis_space": target_vector(
            source_direction_to_runtime(source_construction["foot_right_thigh_axis_pelvis_space_source"])
        ),
        "foot_left_initial_rotation_calf_space": target_vector(source_rotation_to_runtime(left_initial_calf_space)),
        "foot_right_initial_rotation_calf_space": target_vector(source_rotation_to_runtime(right_initial_calf_space)),
        "trace_distance_up_m": 50.0 * SOURCE_LENGTH_TO_RUNTIME_SCALE,
        "trace_distance_down_m": 80.0 * SOURCE_LENGTH_TO_RUNTIME_SCALE,
        "walkable_floor_angle_degrees": 45.0,
        "max_leg_stretch_ratio": 0.99,
        "foot_offset_frequency_hz": 12.0,
        "foot_offset_damping_ratio": 2.0,
        "foot_offset_half_life_seconds": 0.1,
        "pelvis_offset_frequency_hz": 2.0,
        "pelvis_offset_damping_ratio": 1.0,
        "pelvis_offset_min_m": -30.0 * SOURCE_LENGTH_TO_RUNTIME_SCALE,
        "pelvis_offset_max_m": 40.0 * SOURCE_LENGTH_TO_RUNTIME_SCALE,
        "pelvis_min_to_foot_standing_m": 20.0 * SOURCE_LENGTH_TO_RUNTIME_SCALE,
        "pelvis_min_to_foot_moving_m": 50.0 * SOURCE_LENGTH_TO_RUNTIME_SCALE,
        "foot_swing1_min_degrees": -20.0,
        "foot_swing1_max_degrees": 40.0,
        "foot_swing2_min_idle_degrees": -15.0,
        "foot_swing2_max_idle_degrees": 5.0,
        "foot_pole_distance_m": 40.0 * SOURCE_LENGTH_TO_RUNTIME_SCALE,
        "foot_pole_half_life_seconds": 0.05,
    }
    execution_order = [
        "RefreshDiagonalScaling",
        "RefreshSpineRotation",
        "RefreshFootOffset",
        "RefreshPelvisOffset",
        "RefreshFootIk",
        "RefreshHandIk",
    ]
    runtime = {
        "construction": {
            key: value
            for key, value in source_construction.items()
            if key
            not in {
                "foot_height_cm",
                "leg_length_cm",
                "foot_left_thigh_axis_pelvis_space_source",
                "foot_right_thigh_axis_pelvis_space_source",
            }
        },
        "target": target,
        "execution_order": execution_order,
    }
    profile = {
        "source_asset": source["asset_path"],
        "source_mesh": mesh["asset_path"],
        "basis": {
            "source_position_units": "centimeters",
            "target_position_units": "meters",
            "target_axis_mapping": ["x", "z", "y"],
            "target_quaternion_mapping": ["-x", "-z", "-y", "w"],
        },
        "source_blueprint_cdo_properties": source["blueprint_cdo_properties"],
        "source_graphs": source["graphs"],
        "source_rig_vm_links": rig_vm_links(source),
        "source_foot_ik_call_axes": source_foot_ik_axes,
        "source_graph_summary": graph_summary(source),
        "source_node_layout": graph_node_layout(source),
        "source_construction": source_construction,
        "runtime_asset": "assets/control_rigs/cr_als.runtime.json",
    }
    return profile, runtime


def foot_reference_segments(source):
    bones = {bone["name"]: bone["initial_global_transform"] for bone in source["rig_hierarchy"]["bones"]}
    deltas, scales = [], []
    for side in ("l", "r"):
        side_deltas, side_scales = [], []
        for parent, child in ((f"thigh_{side}", f"calf_{side}"), (f"calf_{side}", f"foot_{side}")):
            start = vector_from_fields(bones[parent]["translation"])
            end = vector_from_fields(bones[child]["translation"])
            side_deltas.append(target_vector(source_point_to_runtime([a - b for a, b in zip(start, end)])))
            side_scales.append(target_vector(source_direction_to_runtime(vector_from_fields(bones[parent]["scale"]))))
        deltas.append(side_deltas)
        scales.append(side_scales)
    return {"foot_initial_deltas_m": deltas, "foot_initial_scales": scales}


def self_test():
    if source_point_to_runtime([100.0, 200.0, 300.0]) != [1.0, 3.0, 2.0]:
        raise AssertionError("point conversion")
    if source_direction_to_runtime([1.0, 2.0, 3.0]) != [1.0, 3.0, 2.0]:
        raise AssertionError("direction conversion")
    if normalized_vector([0.0, 3.0, 4.0]) != [0.0, 0.6, 0.8]:
        raise AssertionError("axis normalization")
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
    if axes["thigh_l"]["primary"] != [-1.0, 0.0, 0.0]:
        raise AssertionError("left call primary axis")
    if axes["thigh_r"]["secondary"] != [0.0, -1.0, 0.0]:
        raise AssertionError("right call secondary axis")
    source["rig_hierarchy"] = {"bones": [
        {"name": f"{bone}_{side}", "initial_global_transform": {
            "translation": dict(zip(("x", "y", "z"), [3 * index, 4 * index, 5 * index])),
            "scale": {"x": 1, "y": 2, "z": 3},
        }} for side in ("l", "r") for index, bone in enumerate(("thigh", "calf", "foot"))
    ]}
    segments = foot_reference_segments(source)
    assert segments["foot_initial_deltas_m"] == [[[-0.03, -0.05, -0.04]] * 2] * 2
    assert segments["foot_initial_scales"] == [[[1.0, 3.0, 2.0]] * 2] * 2


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, default=DEFAULT_SOURCE)
    parser.add_argument("--mesh", type=Path, default=DEFAULT_MESH)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--runtime-output", type=Path, default=DEFAULT_RUNTIME_OUTPUT)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    self_test()
    if args.self_test:
        print("CR_Als profile generator self-test PASS")
        return
    source = json.loads(args.source.read_text(encoding="utf-8"))
    mesh = json.loads(args.mesh.read_text(encoding="utf-8"))
    profile, runtime = build(source, mesh)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(profile, indent=2) + "\n", encoding="utf-8")
    args.runtime_output.parent.mkdir(parents=True, exist_ok=True)
    args.runtime_output.write_text(json.dumps(runtime, indent=2) + "\n", encoding="utf-8")
    print(
        "CR_Als profile generated "
        f"graphs={len(profile['source_graph_summary'])} "
        f"nodes={len(profile['source_node_layout'])} "
        f"foot_height_m={runtime['target']['foot_height_m']:.9f} "
        f"leg_length_m={runtime['target']['leg_length_m']:.9f}"
    )


if __name__ == "__main__":
    main()
