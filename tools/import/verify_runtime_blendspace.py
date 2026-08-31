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


def compare(root, source_root, session, name, spec):
    properties = source_properties(session, name, spec)
    consumers = source_consumers(source_root, name, spec)
    failures = graph_failures(root, properties, spec)
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
