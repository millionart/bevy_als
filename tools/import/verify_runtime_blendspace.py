import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT.parent))

from mcp_probe import ensure_session, extract_text, post
from uasset_base import ParseError, UAsset
from uasset_reader import PropReader


PROPERTY_NAMES = [
    "blendParameters",
    "interpolationParam",
    "targetWeightInterpolationSpeedPerSec",
    "bTargetWeightInterpolationEaseInOut",
    "bAllowMeshSpaceBlending",
    "bLoop",
    "bAllowMarkerBasedSync",
    "bShouldMatchSyncPhases",
    "bUseLegacySamplePointAnimationLengthCalculations",
    "previewBasePose",
    "notifyTriggerMode",
    "bInterpolateUsingGrid",
    "preferredTriangulationDirection",
    "axisToScaleAnimation",
    "sampleData",
]


SPECS = {
    "BS_Als_Air_Lean": {
        "source": "Animations/Air/Lean/BS_Als_Air_Lean.uasset",
        "blueprints": ["Character/AnimationInstances/AB_Als_Locomotion.uasset"],
        "graph": "assets/graphs/ab_als/locomotion/air_lean.animgraph.ron",
        "routes": [
            "assets/graphs/ab_als/locomotion/state_fall.animgraph.ron",
            "assets/graphs/ab_als/locomotion/state_jump.animgraph.ron",
        ],
        "writer": "src/locomotion_layer.rs",
        "roles": [
            ("center", (0.0, 0.0)),
            ("forward", (0.0, 1.0)),
            ("backward", (0.0, -1.0)),
            ("left", (-1.0, 0.0)),
            ("right", (1.0, 0.0)),
        ],
        "inputs": {
            "x": "als_locomotion_air_lean_right",
            "y": "als_locomotion_air_lean_forward",
        },
        "writer_fragments": [
            "(\"als_locomotion_air_lean_right\", runtime.air_lean.y)",
            "(\"als_locomotion_air_lean_forward\", runtime.air_lean.x)",
        ],
        "route_fragments": ["air_lean.animgraph.ron", "air_lean.animgraph.ron"],
        "consumers": [
            {
                "kind": "evaluator",
                "loop": True,
                "play_rate": 1.0,
                "start_position": 0.0,
                "sync_group": "None",
                "sync_method": "DoNotSync",
            },
            {
                "kind": "evaluator",
                "loop": True,
                "play_rate": 1.0,
                "start_position": 0.0,
                "sync_group": "None",
                "sync_method": "DoNotSync",
            },
        ],
    },
    "BS_Als_Lean": {
        "source": "Animations/Grounded/Lean/BS_Als_Lean.uasset",
        "blueprints": [
            "Character/AnimationInstances/Stances/AB_Als_Crouching.uasset",
            "Character/AnimationInstances/Stances/AB_Als_Standing.uasset",
        ],
        "graphs": [
            {
                "path": "assets/graphs/ab_als/stances/crouching/lean.animgraph.ron",
                "prefix": "crouching lean",
                "inputs": {
                    "x": "als_crouching_lean_right_amount",
                    "y": "als_crouching_lean_forward_amount",
                },
            },
            {
                "path": "assets/graphs/ab_als/stances/standing/lean.animgraph.ron",
                "prefix": "lean",
                "inputs": {
                    "x": "als_standing_lean_right_amount",
                    "y": "als_standing_lean_forward_amount",
                },
            },
        ],
        "routes": [
            "assets/graphs/ab_als/stances/crouching/state_move.animgraph.ron",
            "assets/graphs/ab_als/stances/standing.animgraph.ron",
        ],
        "writers": [
            (
                "src/crouching_layer.rs",
                [
                    "\"als_crouching_lean_right_amount\"",
                    "\"als_crouching_lean_forward_amount\"",
                    "lean_right_amount: runtime.lean.y",
                    "lean_forward_amount: runtime.lean.x",
                ],
            ),
            (
                "src/standing_layer.rs",
                [
                    "(\"als_standing_lean_right_amount\", inputs.lean_right_amount)",
                    "\"als_standing_lean_forward_amount\"",
                    "lean_right_amount: runtime.lean.y",
                    "lean_forward_amount: runtime.lean.x",
                ],
            ),
        ],
        "roles": [
            ("center", (0.0, 0.0)),
            ("forward", (0.0, 1.0)),
            ("backward", (0.0, -1.0)),
            ("left", (-1.0, 0.0)),
            ("right", (1.0, 0.0)),
        ],
        "consumers": [
            {
                "kind": "evaluator",
                "loop": True,
                "play_rate": 1.0,
                "start_position": 0.0,
                "sync_group": "None",
                "sync_method": "DoNotSync",
            },
            {
                "kind": "evaluator",
                "loop": True,
                "play_rate": 1.0,
                "start_position": 0.0,
                "sync_group": "None",
                "sync_method": "DoNotSync",
            },
        ],
    },
    "BS_Als_WalkRun_Backward": {
        "source": "Animations/Grounded/WalkRun/BS_Als_WalkRun_Backward.uasset",
        "blueprints": ["Character/AnimationInstances/Stances/AB_Als_Standing.uasset"],
        "graph": "assets/graphs/ab_als/stances/standing/state_detail_cycle.animgraph.ron",
        "route": "assets/graphs/ab_als/stances/standing/state_movement_backward.animgraph.ron",
        "label": "Backward",
        "selector": ("backward", 3.0),
        "roles": [
            ("walk_pose", (0.0, 0.0)),
            ("walk", (1.0, 0.0)),
            ("run_pose", (0.0, 1.0)),
            ("run", (1.0, 1.0)),
        ],
        "consumers": [
            {
                "kind": "player",
                "loop": True,
                "play_rate": 1.0,
                "start_position": 0.2,
                "sync_group": "Movement",
                "sync_method": "EAnimSyncMethod::SyncGroup",
            },
        ],
    },
    "BS_Als_WalkRun_Forward": {
        "source": "Animations/Grounded/WalkRun/BS_Als_WalkRun_Forward.uasset",
        "blueprints": ["Character/AnimationInstances/Stances/AB_Als_Standing.uasset"],
        "graph": "assets/graphs/ab_als/stances/standing/state_detail_cycle.animgraph.ron",
        "route": "assets/graphs/ab_als/stances/standing/state_movement_forward.animgraph.ron",
        "label": "Forward",
        "selector": ("forward", 0.0),
        "roles": [
            ("walk_pose", (0.0, 0.0)),
            ("walk", (1.0, 0.0)),
            ("run_pose", (0.0, 1.0)),
            ("run", (1.0, 1.0)),
        ],
        "consumers": [
            {
                "kind": "player",
                "loop": True,
                "play_rate": 1.0,
                "start_position": 0.2,
                "sync_group": "Movement",
                "sync_method": "EAnimSyncMethod::SyncGroup",
            },
        ],
    },
    "BS_Als_WalkRun_Left_Backward": {
        "source": "Animations/Grounded/WalkRun/BS_Als_WalkRun_Left_Backward.uasset",
        "blueprints": ["Character/AnimationInstances/Stances/AB_Als_Standing.uasset"],
        "graph": "assets/graphs/ab_als/stances/standing/state_detail_cycle.animgraph.ron",
        "route": "assets/graphs/ab_als/stances/standing/state_movement_left_backward.animgraph.ron",
        "label": "Left Backward",
        "selector": ("left_backward", 4.0),
        "roles": [
            ("walk_pose", (0.0, 0.0)),
            ("walk", (1.0, 0.0)),
            ("run_pose", (0.0, 1.0)),
            ("run", (1.0, 1.0)),
        ],
        "consumers": [
            {
                "kind": "player",
                "loop": True,
                "play_rate": 1.0,
                "start_position": 0.2,
                "sync_group": "Movement",
                "sync_method": "EAnimSyncMethod::SyncGroup",
            },
        ],
    },
    "BS_Als_WalkRun_Left_Forward": {
        "source": "Animations/Grounded/WalkRun/BS_Als_WalkRun_Left_Forward.uasset",
        "blueprints": ["Character/AnimationInstances/Stances/AB_Als_Standing.uasset"],
        "graph": "assets/graphs/ab_als/stances/standing/state_detail_cycle.animgraph.ron",
        "route": "assets/graphs/ab_als/stances/standing/state_movement_left_forward.animgraph.ron",
        "label": "Left Forward",
        "selector": ("left_forward", 5.0),
        "roles": [
            ("walk_pose", (0.0, 0.0)),
            ("walk", (1.0, 0.0)),
            ("run_pose", (0.0, 1.0)),
            ("run", (1.0, 1.0)),
        ],
        "consumers": [
            {
                "kind": "player",
                "loop": True,
                "play_rate": 1.0,
                "start_position": 0.2,
                "sync_group": "Movement",
                "sync_method": "EAnimSyncMethod::SyncGroup",
            },
        ],
    },
    "BS_Als_WalkRun_Right_Backward": {
        "source": "Animations/Grounded/WalkRun/BS_Als_WalkRun_Right_Backward.uasset",
        "blueprints": ["Character/AnimationInstances/Stances/AB_Als_Standing.uasset"],
        "graph": "assets/graphs/ab_als/stances/standing/state_detail_cycle.animgraph.ron",
        "route": "assets/graphs/ab_als/stances/standing/state_movement_right_backward.animgraph.ron",
        "label": "Right Backward",
        "selector": ("right_backward", 2.0),
        "roles": [
            ("walk_pose", (0.0, 0.0)),
            ("walk", (1.0, 0.0)),
            ("run_pose", (0.0, 1.0)),
            ("run", (1.0, 1.0)),
        ],
        "consumers": [
            {
                "kind": "player",
                "loop": True,
                "play_rate": 1.0,
                "start_position": 0.2,
                "sync_group": "Movement",
                "sync_method": "EAnimSyncMethod::SyncGroup",
            },
        ],
    },
    "BS_Als_WalkRun_Right_Forward": {
        "source": "Animations/Grounded/WalkRun/BS_Als_WalkRun_Right_Forward.uasset",
        "blueprints": ["Character/AnimationInstances/Stances/AB_Als_Standing.uasset"],
        "graph": "assets/graphs/ab_als/stances/standing/state_detail_cycle.animgraph.ron",
        "route": "assets/graphs/ab_als/stances/standing/state_movement_right_forward.animgraph.ron",
        "label": "Right Forward",
        "selector": ("right_forward", 1.0),
        "roles": [
            ("walk_pose", (0.0, 0.0)),
            ("walk", (1.0, 0.0)),
            ("run_pose", (0.0, 1.0)),
            ("run", (1.0, 1.0)),
        ],
        "consumers": [
            {
                "kind": "player",
                "loop": True,
                "play_rate": 1.0,
                "start_position": 0.2,
                "sync_group": "Movement",
                "sync_method": "EAnimSyncMethod::SyncGroup",
            },
        ],
    },
    "BS_Als_Look": {
        "source": "Animations/View/BS_Als_Look.uasset",
        "blueprints": ["Character/AnimationInstances/AB_Als_Head.uasset"],
        "graph": "assets/graphs/ab_als/head.animgraph.ron",
        "route": "assets/graphs/ab_als/ab_als.animgraph.ron",
        "roles": [
            ("down", (-90.0, 0.0)),
            ("forward", (0.0, 0.0)),
            ("up", (90.0, 0.0)),
        ],
        "consumers": [
            {
                "kind": "evaluator",
                "loop": True,
                "play_rate": 1.0,
                "start_position": 0.0,
                "sync_group": "None",
                "sync_method": "DoNotSync",
            },
        ],
    },
}


def call(session, toolset, tool_name, arguments):
    body, error = post(
        {
            "jsonrpc": "2.0",
            "id": 91,
            "method": "tools/call",
            "params": {
                "name": "call_tool",
                "arguments": {
                    "toolset_name": toolset,
                    "tool_name": tool_name,
                    "arguments": arguments,
                },
            },
        },
        session,
    )
    if body is None:
        raise RuntimeError(error or "MCP request failed")
    if "error" in body:
        raise RuntimeError(json.dumps(body["error"]))
    return json.loads(extract_text(body))["returnValue"]


def source_properties(session, name, spec):
    package = "/ALS/ALS/" + spec["source"].removesuffix(".uasset")
    instance = {"refPath": package + "." + name}
    value = call(
        session,
        "editor_toolset.toolsets.object.ObjectTools",
        "get_properties",
        {"instance": instance, "properties": PROPERTY_NAMES},
    )
    return json.loads(value)


def untag(value):
    if isinstance(value, dict) and "__tagged__" in value:
        return value["__tagged__"]
    return value


def get_field(value, name, default=None):
    value = untag(value)
    return value.get(name, default) if isinstance(value, dict) else default


def asset_leaf(value):
    if not isinstance(value, str):
        return ""
    return value.rsplit("/", 1)[-1].split(".", 1)[0]


def enum_text(value):
    value = untag(value)
    if isinstance(value, dict):
        return str(value.get("value", value.get("enum_name", "None")))
    return str(value)


def source_class(asset, export):
    if export.class_index >= 0:
        return ""
    imported = asset.imports[-export.class_index - 1]
    return imported.object_name if imported.class_name == "Class" else imported.class_name


def source_consumers(source_root, name, spec):
    records = []
    for relative in spec["blueprints"]:
        asset = UAsset(str(source_root / relative))
        reader = PropReader(asset)
        for index, export in enumerate(asset.exports):
            class_name = source_class(asset, export)
            if class_name not in {
                "AnimGraphNode_BlendSpacePlayer",
                "AnimGraphNode_BlendSpaceEvaluator",
            }:
                continue
            properties, valid, _ = reader.read_export(index)
            if not valid:
                continue
            node = untag(properties.get("Node", {}))
            if asset_leaf(get_field(node, "BlendSpace", "")) != name:
                continue
            records.append(
                {
                    "kind": "evaluator" if class_name.endswith("Evaluator") else "player",
                    "loop": bool(get_field(node, "bLoop", True)),
                    "play_rate": float(get_field(node, "PlayRate", 1.0)),
                    "start_position": float(get_field(node, "StartPosition", 0.0)),
                    "sync_group": str(get_field(node, "GroupName", "None")),
                    "sync_method": enum_text(get_field(node, "Method", "DoNotSync")),
                }
            )
    return sorted(records, key=lambda record: (record["kind"], record["start_position"]))


def local_clip(reference):
    if not isinstance(reference, dict):
        raise RuntimeError("source sample does not contain an animation reference")
    path = reference.get("refPath", "")
    package = path.rsplit(".", 1)[0]
    prefix = "/ALS/ALS/Animations/"
    if not package.startswith(prefix):
        raise RuntimeError(f"unexpected source animation reference: {path}")
    return "animations/" + package.removeprefix(prefix) + ".anim.ron"


def required_sample_map(properties, roles):
    samples = {}
    for item in properties["sampleData"]:
        point = item["sampleValue"]
        samples[(float(point["x"]), float(point["y"]))] = local_clip(item["animation"])
    result = {}
    for role, point in roles:
        clip = samples.get(point)
        if clip is None:
            raise RuntimeError(f"source is missing sample at {point}")
        result[role] = clip
    return result


def source_clip_properties(session, reference):
    value = call(
        session,
        "editor_toolset.toolsets.object.ObjectTools",
        "get_properties",
        {
            "instance": reference,
            "properties": [
                "sequenceLength",
                "skeleton",
                "additiveAnimType",
                "refPoseType",
                "refPoseSeq",
            ],
        },
    )
    return json.loads(value)


def local_clip_resource(root, clip):
    path = root / "assets" / clip
    text = path.read_text(encoding="utf-8")
    source = re.search(r'path:\s*"([^"]+)"', text)
    animation = re.search(r'animation_name:\s*"([^"]+)"', text)
    skeleton = re.search(r'skeleton:\s*"([^"]+)"', text)
    if source is None or animation is None or skeleton is None:
        raise RuntimeError(f"invalid project clip resource: {clip}")
    return source.group(1), animation.group(1), skeleton.group(1)


def gltf_duration(path, animation_name):
    document = json.loads(path.read_text(encoding="utf-8"))
    animation = next(
        (item for item in document.get("animations", []) if item.get("name") == animation_name),
        None,
    )
    if animation is None:
        raise RuntimeError(f"missing glTF animation: {animation_name}")
    times = []
    for sampler in animation.get("samplers", []):
        accessor = document["accessors"][sampler["input"]]
        values = accessor.get("max", [])
        if values:
            times.append(float(values[0]))
    if not times:
        raise RuntimeError(f"missing glTF animation time range: {animation_name}")
    return max(times)


def resource_failures(root, session, properties):
    failures = []
    for sample in properties["sampleData"]:
        clip = local_clip(sample["animation"])
        try:
            gltf_path, animation_name, skeleton_path = local_clip_resource(root, clip)
            local_duration = gltf_duration(root / "assets" / gltf_path, animation_name)
        except (FileNotFoundError, IndexError, RuntimeError, json.JSONDecodeError) as error:
            failures.append(str(error))
            continue
        source = source_clip_properties(session, sample["animation"])
        source_duration = float(source["sequenceLength"])
        if abs(local_duration - source_duration) > 1e-5:
            failures.append(f"project clip duration differs: {clip}")
        if not (root / "assets" / skeleton_path).is_file():
            failures.append(f"project clip skeleton is missing: {clip}")
        if not isinstance(source.get("skeleton"), dict):
            failures.append(f"source clip skeleton is missing: {clip}")
    return failures


def source_sequence_metadata(source_root, animation):
    matches = list(source_root.rglob(f"{animation}.uasset"))
    if len(matches) != 1:
        raise RuntimeError(f"expected one source animation asset for {animation}")
    asset = UAsset(str(matches[0]))
    reader = PropReader(asset)
    for index, export in enumerate(asset.exports):
        if export.object_name != animation or source_class(asset, export) != "AnimSequence":
            continue
        properties, valid, _ = reader.read_export(index)
        if not valid:
            break
        markers = []
        for marker in properties.get("AuthoredSyncMarkers", []):
            if not isinstance(marker, dict):
                continue
            name = str(marker.get("MarkerName", "None"))
            if name != "None":
                markers.append((name, float(marker.get("Time", 0.0))))
        return float(properties.get("SequenceLength", 0.0)), markers
    raise RuntimeError(f"cannot read source animation metadata for {animation}")


def graph_point_metadata(node_line, role):
    match = re.search(
        rf'id:\s*"{re.escape(role)}"\s*,\s*point:\s*\([^)]*\)\s*,\s*duration:\s*([-0-9.]+)\s*,\s*markers:\s*\[(.*?)\]\)',
        node_line,
    )
    if match is None:
        raise RuntimeError(f"cannot read runtime graph point metadata for {role}")
    markers = [
        (name, float(time))
        for name, time in re.findall(r'name:\s*"([^"]+)"\s*,\s*time:\s*([-0-9.]+)', match.group(2))
    ]
    return float(match.group(1)), markers


def graph_failures(root, properties, spec):
    failures = []
    graph = (root / spec["graph"]).read_text(encoding="utf-8")
    samples = required_sample_map(properties, spec["roles"])
    for role, point in spec["roles"]:
        clip_line = next(
            (line for line in graph.splitlines() if f'name: "air lean {role} clip"' in line),
            "",
        )
        if samples[role] not in clip_line:
            failures.append(f"runtime graph clip {role} differs")
        loop_line = next(
            (line for line in graph.splitlines() if f'name: "air lean {role} loop"' in line),
            "",
        )
        if "LoopNode" not in loop_line:
            failures.append(f"runtime graph loop {role} differs")
        point_pattern = rf'id:\s*"{role}"\s*,\s*point:\s*\(\s*{point[0]:g}(?:\.0)?\s*,\s*{point[1]:g}(?:\.0)?\s*\)'
        if not re.search(point_pattern, graph):
            failures.append(f"runtime graph point {role} differs")
    if "BlendSpaceNode" not in graph or "mode: LinearizedInterpolate" not in graph:
        failures.append("runtime graph blend mode differs")
    if "sync_mode: Absolute" not in graph:
        failures.append("runtime graph sync mode differs")
    for axis, key in spec["inputs"].items():
        if key not in graph:
            failures.append(f"runtime graph input {axis} differs")
    first_axis = properties["blendParameters"][0]
    second_axis = properties["blendParameters"][1]
    if (first_axis["min"], first_axis["max"], second_axis["min"], second_axis["max"]) != (-1, 1, -1, 1):
        failures.append("source axis range is unsupported by the runtime graph check")
    if any(entry["interpolationTime"] != 0 for entry in properties["interpolationParam"][:2]):
        failures.append("source axis smoothing is not represented by the runtime graph")
    for route, fragment in zip(spec["routes"], spec["route_fragments"]):
        if fragment not in (root / route).read_text(encoding="utf-8"):
            failures.append(f"runtime route {route} differs")
    writer = (root / spec["writer"]).read_text(encoding="utf-8")
    for fragment in spec["writer_fragments"]:
        if fragment not in writer:
            failures.append(f"runtime writer mapping differs: {fragment}")
    return failures


def lean_graph_failures(root, properties, spec):
    failures = []
    samples = required_sample_map(properties, spec["roles"])
    for graph_spec in spec["graphs"]:
        graph = (root / graph_spec["path"]).read_text(encoding="utf-8")
        for role, point in spec["roles"]:
            clip_line = next(
                (
                    line
                    for line in graph.splitlines()
                    if f'name: "{graph_spec["prefix"]} {role} clip"' in line
                ),
                "",
            )
            if samples[role] not in clip_line:
                failures.append(f"{graph_spec['path']}: clip {role} differs")
            evaluator_line = next(
                (
                    line
                    for line in graph.splitlines()
                    if f'name: "{graph_spec["prefix"]} {role} evaluator"' in line
                ),
                "",
            )
            if "TimePositionNode" not in evaluator_line or "loop_position: false" not in evaluator_line:
                failures.append(f"{graph_spec['path']}: evaluator {role} differs")
            point_pattern = rf'id:\s*"{role}"\s*,\s*point:\s*\(\s*{point[0]:g}(?:\.0)?\s*,\s*{point[1]:g}(?:\.0)?\s*\)'
            if not re.search(point_pattern, graph):
                failures.append(f"{graph_spec['path']}: point {role} differs")
        if "BlendSpaceNode" not in graph or "mode: LinearizedInterpolate" not in graph:
            failures.append(f"{graph_spec['path']}: blend mode differs")
        if "sync_mode: Absolute" not in graph:
            failures.append(f"{graph_spec['path']}: sync mode differs")
        for axis, key in graph_spec["inputs"].items():
            if key not in graph:
                failures.append(f"{graph_spec['path']}: input {axis} differs")
    if any(entry["interpolationTime"] != 0 for entry in properties["interpolationParam"][:2]):
        failures.append("source axis smoothing is not represented by runtime lean graphs")
    for route in spec["routes"]:
        if "lean.animgraph.ron" not in (root / route).read_text(encoding="utf-8"):
            failures.append(f"runtime route {route} differs")
    for relative, fragments in spec["writers"]:
        text = (root / relative).read_text(encoding="utf-8")
        for fragment in fragments:
            if fragment not in text:
                failures.append(f"runtime writer mapping differs: {relative}: {fragment}")
    return failures


def walkrun_graph_failures(root, source_root, properties, spec):
    failures = []
    graph = (root / spec["graph"]).read_text(encoding="utf-8")
    samples = required_sample_map(properties, spec["roles"])
    node_line = next(
        (
            line
            for line in graph.splitlines()
            if f'name: "{spec["label"]} walk run blendspace"' in line
        ),
        "",
    )
    if "NormalizedBlendSpaceNode" not in node_line or "looped: true" not in node_line:
        failures.append("runtime normalized blendspace loop mapping differs")
    for role, point in spec["roles"]:
        clip_line = next(
            (
                line
                for line in graph.splitlines()
                if f'name: "{spec["label"]} {role} clip"' in line
            ),
            "",
        )
        if samples[role] not in clip_line:
            failures.append(f"runtime graph clip {role} differs")
        loop_line = next(
            (
                line
                for line in graph.splitlines()
                if f'name: "{spec["label"]} {role} loop"' in line
            ),
            "",
        )
        if "LoopNode" not in loop_line:
            failures.append(f"runtime graph loop {role} differs")
        point_pattern = rf'id:\s*"{role}"\s*,\s*point:\s*\(\s*{point[0]:g}(?:\.0)?\s*,\s*{point[1]:g}(?:\.0)?\s*\)'
        if not re.search(point_pattern, node_line):
            failures.append(f"runtime graph point {role} differs")
        try:
            source_duration, source_markers = source_sequence_metadata(
                source_root,
                next(
                    asset_leaf(sample["animation"]["refPath"])
                    for sample in properties["sampleData"]
                    if (
                        float(sample["sampleValue"]["x"]),
                        float(sample["sampleValue"]["y"]),
                    )
                    == point
                ),
            )
            runtime_duration, runtime_markers = graph_point_metadata(node_line, role)
            if abs(source_duration - runtime_duration) > 1e-5:
                failures.append(f"runtime graph duration {role} differs")
            if len(source_markers) != len(runtime_markers) or any(
                source_name != runtime_name or abs(source_time - runtime_time) > 1e-5
                for (source_name, source_time), (runtime_name, runtime_time) in zip(
                    source_markers, runtime_markers
                )
            ):
                failures.append(f"runtime graph markers {role} differ")
        except RuntimeError as error:
            failures.append(str(error))
    if '"als_ext::CubicVec2AxisFilterNode": (x_window_seconds: 0.2, y_window_seconds: 0.4)' not in graph:
        failures.append("runtime graph cubic axis filter differs")
    for key in [
        "als_standing_stride_amount",
        "als_standing_walk_run_amount",
        "als_animation_delta_seconds",
        "als_standing_movement_marker_phase",
    ]:
        if key not in graph:
            failures.append(f"runtime graph input differs: {key}")
    selector_name, selector_value = spec["selector"]
    selector = rf'id:\s*"{selector_name}"\s*,\s*value:\s*{selector_value:g}(?:\.0)?'
    if not re.search(selector, graph):
        failures.append("runtime selector differs")
    route = (root / spec["route"]).read_text(encoding="utf-8")
    if "state_movement_blend.animgraph.ron" not in route or f"value: {selector_value:.1f}" not in route:
        failures.append("runtime movement route differs")
    standing = (root / "src/standing_layer.rs").read_text(encoding="utf-8")
    for fragment in [
        "const MOVEMENT_BLENDSPACE_START_PHASE: f32 = 0.2;",
        "(\"als_standing_stride_amount\", inputs.stride_amount)",
        "(\"als_standing_walk_run_amount\", inputs.walk_run_amount)",
        "\"als_standing_movement_marker_phase\"",
    ]:
        if fragment not in standing:
            failures.append(f"runtime standing writer differs: {fragment}")
    root_graph = (root / "src/ab_als_root.rs").read_text(encoding="utf-8")
    if "(\"als_animation_delta_seconds\", time.delta_secs())" not in root_graph:
        failures.append("runtime delta writer differs")
    x_filter, y_filter = properties["interpolationParam"][:2]
    if (
        x_filter["interpolationType"],
        x_filter["interpolationTime"],
        y_filter["interpolationType"],
        y_filter["interpolationTime"],
    ) != ("BSIT_Cubic", 0.20000000298023224, "BSIT_Cubic", 0.4000000059604645):
        failures.append("source cubic axis settings differ")
    return failures


def look_graph_failures(root, properties, spec):
    failures = []
    graph = (root / spec["graph"]).read_text(encoding="utf-8")
    samples = required_sample_map(properties, spec["roles"])
    for role, point in spec["roles"]:
        clip_line = next(
            (line for line in graph.splitlines() if f'name: "look {role} clip"' in line),
            "",
        )
        if samples[role] not in clip_line:
            failures.append(f"runtime graph clip {role} differs")
        evaluator_line = next(
            (line for line in graph.splitlines() if f'name: "look {role} evaluator"' in line),
            "",
        )
        if "TimePositionNode" not in evaluator_line or "loop_position: false" not in evaluator_line:
            failures.append(f"runtime graph evaluator {role} differs")
        point_pattern = rf'id:\s*"{role}"\s*,\s*position:\s*{point[0]:g}(?:\.0)?'
        if not re.search(point_pattern, graph):
            failures.append(f"runtime graph point {role} differs")
    if "BlendSpace1DNode" not in graph:
        failures.append("runtime graph blendspace node differs")
    for key in ["als_head_pitch_angle", "als_head_yaw_amount"]:
        if key not in graph:
            failures.append(f"runtime graph input differs: {key}")
    root_graph = (root / spec["route"]).read_text(encoding="utf-8")
    if "head.animgraph.ron" not in root_graph:
        failures.append("runtime head route differs")
    writer = (root / "src/head_layer.rs").read_text(encoding="utf-8")
    for fragment in [
        "(\"als_head_pitch_angle\", state.pitch_angle)",
        "(\"als_head_yaw_amount\", state.yaw_amount)",
    ]:
        if fragment not in writer:
            failures.append(f"runtime head writer differs: {fragment}")
    axis = properties["blendParameters"][0]
    if (axis["displayName"], axis["min"], axis["max"]) != ("Pitch Angle", -90, 90):
        failures.append("source pitch axis differs")
    if any(entry["interpolationTime"] != 0 for entry in properties["interpolationParam"][:1]):
        failures.append("source pitch smoothing is not represented by runtime head graph")
    return failures


def compare(root, source_root, session, name, spec):
    properties = source_properties(session, name, spec)
    consumers = source_consumers(source_root, name, spec)
    if name == "BS_Als_Air_Lean":
        failures = graph_failures(root, properties, spec)
    elif name == "BS_Als_Lean":
        failures = lean_graph_failures(root, properties, spec)
    elif name == "BS_Als_Look":
        failures = look_graph_failures(root, properties, spec)
    else:
        failures = walkrun_graph_failures(root, source_root, properties, spec)
    failures.extend(resource_failures(root, session, properties))
    if consumers != spec["consumers"]:
        failures.append("source consumer configuration differs")
    return {
        "asset": name,
        "pass": not failures,
        "failures": failures,
        "source_consumers": consumers,
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source-root", required=True, type=Path)
    parser.add_argument("--asset", choices=sorted(SPECS))
    args = parser.parse_args()
    root = Path.cwd()
    names = [args.asset] if args.asset else sorted(SPECS)
    session = ensure_session()
    try:
        results = [compare(root, args.source_root, session, name, SPECS[name]) for name in names]
    except (FileNotFoundError, ParseError, RuntimeError, ValueError, json.JSONDecodeError) as error:
        print(json.dumps({"pass": False, "error": str(error)}, indent=2))
        return 2
    passed = all(result["pass"] for result in results)
    print(json.dumps({"pass": passed, "results": results}, indent=2))
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
