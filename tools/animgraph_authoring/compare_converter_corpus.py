import argparse
import collections
import json
import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(ROOT / "tools" / "import"))

import ue_animgraph_converter as converter
from extract_ue_visible_graph_contract import contract
from ue_animgraph_pose_contract import PoseContractError, property_binding_paths
from ue_animgraph_reachability import traverse


MANUAL_CAPABILITY_CANDIDATES = {
    "/Script/AnimGraph.AnimGraphNode_MultiWayBlend": "als_ext::SourceMultiWayBlendNode",
    "/Script/AnimGraph.AnimGraphNode_BlendSpacePlayer": "als_ext::NormalizedBlendSpaceNode",
    "/Script/AnimGraph.AnimGraphNode_BlendListByEnum": "als_ext::SourceEnumPoseBlendNode",
    "/Script/AnimGraph.AnimGraphNode_LayeredBoneBlend": "als_ext::SourceLayeredBoneBlendNode",
    "/Script/AnimGraph.AnimGraphNode_Slot": "als_ext::SourceSlotNode",
    "/Script/ALS.AlsAnimGraphNode_GameplayTagsBlend": "als_ext::SourceGameplayTagBlendNode",
    "/Script/AnimGraph.K2Node_AnimGetter": "als_ext::SourceScalarExpressionNode",
    "/Script/BlueprintGraph.K2Node_Knot": "als_ext::SourceScalarExpressionNode",
}
MANUAL_FUNCTION_CANDIDATES = {
    "SetHipsDirection": "als_ext::SourceSetHipsDirectionNode",
    "NOT Boolean": "als_ext::SourceScalarExpressionNode",
    "Absolute (Float)": "als_ext::SourceScalarExpressionNode",
}


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def canonical(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def manual_node_types(topology):
    values = []
    for file in topology.get("files", []):
        data = file.get("topology", {}) if isinstance(file, dict) else {}
        for node in data.get("nodes", []) if isinstance(data, dict) else []:
            entry = node.get("entry", "") if isinstance(node, dict) else ""
            if not isinstance(entry, str):
                continue
            match = re.search(r'inner:\s*\{\s*"([^"]+)"', entry)
            if match is not None:
                values.append(match.group(1))
    return collections.Counter(values)


def reachable_source_classes(source):
    closure = traverse(source, "AnimGraph")
    reachable = {
        node
        for graph in closure["graphs"]
        for node in graph.get("reachable_nodes", [])
    }
    values = []
    for graph in source.get("graphs", []):
        for node in graph.get("nodes", []):
            if node.get("object_path") in reachable:
                values.append(node.get("class_path"))
    return closure, collections.Counter(value for value in values if isinstance(value, str))


def reachable_property_bindings(source, registry):
    closure = traverse(source, "AnimGraph")
    rows = collections.Counter()
    for entry in closure["graphs"]:
        graph = contract(source, entry["source_graph"])
        reachable = set(entry["reachable_nodes"])
        for node in graph["source_graph"]["nodes"]:
            if node.get("object_path") not in reachable:
                continue
            try:
                bindings = property_binding_paths(node)
            except PoseContractError as error:
                rows[(node.get("class_path"), "invalid", "", str(error))] += 1
                continue
            renderer, _ = converter.mapping_for(node, registry)
            for pin, path in bindings.items():
                rows[(node.get("class_path"), renderer or "unregistered", pin, path)] += 1
    return [
        {
            "source_class": source_class,
            "renderer": renderer,
            "source_pin": pin,
            "source_path": path,
            "count": count,
        }
        for (source_class, renderer, pin, path), count in sorted(rows.items())
    ]


def candidate_rows(class_counts, manual_types, registry, issues):
    blocker_counts = collections.Counter(
        issue.get("source_class")
        for issue in issues
        if isinstance(issue.get("source_class"), str)
    )
    rows = []
    for source_class, target_type in MANUAL_CAPABILITY_CANDIDATES.items():
        count = class_counts.get(source_class, 0)
        if count == 0:
            continue
        rows.append(
            {
                "source_class": source_class,
                "source_count": count,
                "converter_registered": source_class in registry["node_classes"],
                "converter_blocker_count": blocker_counts[source_class],
                "manual_candidate_node": target_type,
                "manual_candidate_count": manual_types[target_type],
                "manual_candidate_only": True,
            }
        )
    return rows


def function_candidate_rows(manual_types, issues):
    blocker_counts = collections.Counter(
        issue.get("source_signature")
        for issue in issues
        if issue.get("kind") == "unregistered_source_function"
    )
    return [
        {
            "source_signature": signature,
            "converter_blocker_count": blocker_counts[signature],
            "manual_candidate_node": target_type,
            "manual_candidate_count": manual_types[target_type],
            "manual_candidate_only": True,
        }
        for signature, target_type in MANUAL_FUNCTION_CANDIDATES.items()
        if blocker_counts[signature]
    ]


def resolved_property_bindings(ir):
    counts = collections.Counter()
    for graph in ir.get("graphs", []):
        for node in graph.get("nodes", []):
            semantic = node.get("semantic") if isinstance(node, dict) else None
            if not isinstance(semantic, dict):
                continue
            bindings = semantic.get("resolved_property_bindings")
            if not isinstance(bindings, list):
                continue
            for binding in bindings:
                if not isinstance(binding, dict):
                    continue
                path = binding.get("source_property_path")
                key = binding.get("target_key")
                if isinstance(path, str) and path and isinstance(key, str) and key:
                    counts[(path, key)] += 1
    return [
        {"source_path": path, "target_key": key, "count": count}
        for (path, key), count in sorted(counts.items())
    ]


def resolved_custom_curves(ir):
    rows = []
    for graph in ir.get("graphs", []):
        plan = graph.get("fsm_renderer")
        if not isinstance(plan, dict):
            continue
        for transition in plan.get("transitions", []):
            render = transition.get("render") if isinstance(transition, dict) else None
            if not isinstance(render, dict):
                continue
            source_curve = render.get("source_custom_curve")
            keys = render.get("custom_curve")
            if isinstance(source_curve, str) and source_curve and isinstance(keys, list):
                rows.append({"source_curve": source_curve, "key_count": len(keys)})
    counts = collections.Counter((row["source_curve"], row["key_count"]) for row in rows)
    return [
        {"source_curve": source_curve, "key_count": key_count, "transition_count": count}
        for (source_curve, key_count), count in sorted(counts.items())
    ]


def compare(source, topology, registry, asset_root, source_content_root, mount):
    closure, class_counts = reachable_source_classes(source)
    ir, issues = converter.build_ir(
        source,
        "AnimGraph",
        registry,
        asset_root,
        source_content_root,
        mount,
    )
    manual_types = manual_node_types(topology)
    return {
        "format": "ue_animgraph_converter_corpus_diff_v1",
        "source_asset": source["asset_path"],
        "source": {
            "reachable_graph_count": len(closure["graphs"]),
            "reachable_node_count": sum(len(graph["reachable_nodes"]) for graph in closure["graphs"]),
            "classes": dict(sorted(class_counts.items())),
            "property_bindings": reachable_property_bindings(source, registry),
        },
        "manual": {
            "file_count": len(topology.get("files", [])),
            "node_types": dict(sorted(manual_types.items())),
        },
        "converter": {
            "pass": not issues,
            "reachable_graph_count": len(ir["graphs"]),
            "blockers": dict(
                sorted(collections.Counter(issue.get("kind") for issue in issues).items())
            ),
            "unregistered_classes": dict(
                sorted(
                    collections.Counter(
                        issue.get("source_class")
                        for issue in issues
                        if issue.get("kind") == "unregistered_source_node"
                    ).items()
                )
            ),
            "unregistered_functions": dict(
                sorted(
                    collections.Counter(
                        issue.get("source_signature")
                        for issue in issues
                        if issue.get("kind") == "unregistered_source_function"
                    ).items()
                )
            ),
            "resolved_property_bindings": resolved_property_bindings(ir),
            "resolved_custom_curves": resolved_custom_curves(ir),
        },
        "manual_capability_candidates": candidate_rows(
            class_counts,
            manual_types,
            registry,
            issues,
        ),
        "manual_function_candidates": function_candidate_rows(manual_types, issues),
    }


def fixture_source():
    return {
        "asset_path": "/Asset",
        "graphs": [
            {
                "object_path": "/Asset:AnimGraph",
                "class_path": "/Script/AnimGraph.AnimationGraph",
                "nodes": [
                    {
                        "object_path": "/Asset:AnimGraph.Root",
                        "class_path": "/Script/AnimGraph.AnimGraphNode_Root",
                        "title": "Output Pose",
                        "pins": [],
                    },
                    {
                        "object_path": "/Asset:AnimGraph.Multi",
                        "class_path": "/Script/AnimGraph.AnimGraphNode_MultiWayBlend",
                        "title": "Blend Multi",
                        "pins": [],
                    },
                ],
            }
        ],
    }


def run_self_test():
    topology = {
        "files": [
            {
                "topology": {
                    "nodes": [
                        {
                            "entry": '(\n    inner: {\n        "als_ext::SourceMultiWayBlendNode": ()\n    }\n)'
                        }
                    ]
                }
            }
        ]
    }
    manual_types = manual_node_types(topology)
    assert manual_types["als_ext::SourceMultiWayBlendNode"] == 1
    source = fixture_source()
    source["graphs"][0]["nodes"][0]["properties"] = [
        {
            "name": "Binding",
            "exported_text": "/Script/AnimGraph.AnimGraphNodeBinding_Base'/Asset:AnimGraph.Root.Binding'",
        }
    ]
    source["asset_objects"] = [
        {
            "object_path": "/Asset:AnimGraph.Root.Binding",
            "class_path": "/Script/AnimGraph.AnimGraphNodeBinding_Base",
            "properties": [
                {
                    "name": "PropertyBindings",
                    "exported_text": '(("Alpha", (PropertyName="Alpha",PropertyPath=("GetParent","InAirState","GroundPredictionAmount"),PathAsText="GetParent.InAirState.GroundPredictionAmount",bIsBound=True)))',
                }
            ],
        }
    ]
    assert reachable_property_bindings(source, converter.fixture_registry()) == [
        {
            "source_class": "/Script/AnimGraph.AnimGraphNode_Root",
            "renderer": "root_output",
            "source_pin": "Alpha",
            "source_path": "InAirState.GroundPredictionAmount",
            "count": 1,
        }
    ]
    rows = candidate_rows(
        collections.Counter(
            {"/Script/AnimGraph.AnimGraphNode_MultiWayBlend": 2}
        ),
        manual_types,
        {"node_classes": {}},
        [
            {
                "source_class": "/Script/AnimGraph.AnimGraphNode_MultiWayBlend",
                "kind": "unregistered_source_node",
            }
        ],
    )
    assert rows[0]["converter_blocker_count"] == 1
    assert rows[0]["manual_candidate_count"] == 1
    assert resolved_property_bindings(
        {
            "graphs": [
                {
                    "nodes": [
                        {
                            "semantic": {
                                "resolved_property_bindings": [
                                    {
                                        "source_property_path": "GroundedState.VelocityBlend.ForwardAmount",
                                        "target_key": "grounded_state.velocity_blend.forward_amount",
                                    }
                                ],
                            }
                        },
                        {
                            "semantic": {
                                "resolved_property_bindings": [
                                    {
                                        "source_property_path": "GroundedState.RotationYawOffsets.ForwardAngle",
                                        "target_key": "grounded_state.rotation_yaw_offsets.forward_angle",
                                    }
                                ],
                            }
                        },
                        {
                            "semantic": {
                                "resolved_property_bindings": [
                                    {
                                        "source_property_path": "GroundedState.HipsDirection",
                                        "target_key": "grounded_state.hips_direction",
                                    }
                                ],
                            }
                        },
                    ]
                }
            ]
        }
    ) == [
        {
            "source_path": "GroundedState.HipsDirection",
            "target_key": "grounded_state.hips_direction",
            "count": 1,
        },
        {
            "source_path": "GroundedState.RotationYawOffsets.ForwardAngle",
            "target_key": "grounded_state.rotation_yaw_offsets.forward_angle",
            "count": 1,
        },
        {
            "source_path": "GroundedState.VelocityBlend.ForwardAmount",
            "target_key": "grounded_state.velocity_blend.forward_amount",
            "count": 1,
        }
    ]
    assert resolved_custom_curves(
        {
            "graphs": [
                {
                    "fsm_renderer": {
                        "transitions": [
                            {
                                "render": {
                                    "source_custom_curve": "/ALS/Curves/CF_Test.CF_Test",
                                    "custom_curve": [{}, {}, {}],
                                }
                            }
                        ]
                    }
                }
            ]
        }
    ) == [
        {
            "source_curve": "/ALS/Curves/CF_Test.CF_Test",
            "key_count": 3,
            "transition_count": 1,
        }
    ]
    return {
        "manual_node_type_inventory": "PASS",
        "reachable_source_property_bindings_reported": "PASS",
        "source_to_manual_candidate_gap_reported": "PASS",
        "resolved_source_property_bindings_reported": "PASS",
        "resolved_source_custom_curves_reported": "PASS",
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path)
    parser.add_argument("--manual-topology", type=Path)
    parser.add_argument("--asset-root", type=Path, default=ROOT / "assets")
    parser.add_argument("--source-content-root", type=Path)
    parser.add_argument("--mount", default="/ALS")
    parser.add_argument("--registry", type=Path, default=ROOT / "tools" / "animgraph_authoring" / "als_ref_mapping_registry.json")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
        return 0
    if (
        args.source is None
        or args.manual_topology is None
        or args.source_content_root is None
        or args.output is None
        or args.write == args.check
    ):
        raise ValueError("source, manual-topology, source-content-root, output, and exactly one mode are required")
    registry = converter.validate_registry(read_json(args.registry))
    value = compare(
        read_json(args.source),
        read_json(args.manual_topology),
        registry,
        args.asset_root,
        args.source_content_root,
        args.mount,
    )
    if args.write:
        write_json(args.output, value)
    elif not args.output.is_file() or canonical(value) != canonical(read_json(args.output)):
        raise ValueError("converter corpus differential differs")
    print(
        json.dumps(
            {
                "source_asset": value["source_asset"],
                "converter_pass": value["converter"]["pass"],
                "blockers": value["converter"]["blockers"],
            },
            ensure_ascii=False,
        )
    )


if __name__ == "__main__":
    raise SystemExit(main())
