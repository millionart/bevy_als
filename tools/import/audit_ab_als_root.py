import argparse
import hashlib
import json
import re
from pathlib import Path

from apply_source_layout import (
    all_source_nodes,
    place_nodes,
    source_nodes,
    source_owner,
    target_edges,
    target_nodes,
)
from capture_animgraph_ron_topology import parse_positions


ROOT = Path(__file__).resolve().parents[2]
DEFAULT_SOURCE = ROOT / "target" / "bf109" / "source_native" / "AB_Als.ue.json"
DEFAULT_CONTRACT = ROOT / "docs" / "bf109" / "source_contracts" / "AB_Als.source.json"
DEFAULT_TARGET = ROOT / "assets" / "graphs" / "ab_als" / "ab_als.animgraph.ron"
DEFAULT_MAPPING = ROOT / "tools" / "import" / "ab_als_source_layout_map.json"
DEFAULT_REPORT = ROOT / "target" / "bf109" / "audits" / "AB_Als.audit.json"


def canonical_hash(value):
    encoded = json.dumps(value, ensure_ascii=False, separators=(",", ":"), sort_keys=True).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def canonical_source_payload(value):
    def normalize(current, path):
        if isinstance(current, dict):
            if current.get("name") == "GraphGuid" and "exported_text" in current:
                return None
            output = {}
            for key, child in current.items():
                if key == "pin_id":
                    continue
                if path == "" and key in {"blueprint_cdo_path", "blueprint_cdo_properties"}:
                    continue
                normalized = normalize(child, f"{path}/{key}")
                if normalized is not None:
                    output[key] = normalized
            return output
        if isinstance(current, list):
            output = [
                normalized
                for index, child in enumerate(current)
                if (normalized := normalize(child, f"{path}/{index}")) is not None
            ]
            if path.endswith("/linked_to") or path.endswith("/links"):
                output.sort(key=lambda item: json.dumps(item, ensure_ascii=False, sort_keys=True))
            return output
        return current

    return normalize(value, "")


def canonical_source_hash(value):
    return canonical_hash(canonical_source_payload(value))


def find_graph(source, name):
    for graph in source["graphs"]:
        if graph["name"] == name:
            return graph
    raise ValueError(f"missing graph: {name}")


def source_indices(entry):
    values = entry.get("source_indices")
    if values is None:
        values = [entry.get("source_index")]
    return [value for value in values if value is not None]


def editor_position(text, name):
    pattern = re.compile(
        rf'\b{re.escape(name)}:\s*\(\s*([-+0-9.eE]+)\s*,\s*([-+0-9.eE]+)\s*\)'
    )
    match = pattern.search(text)
    return None if match is None else [float(match.group(1)), float(match.group(2))]


def node_data_edge(text, target_id, target_pin, source_id, source_pin):
    pattern = re.compile(
        rf'NodeData\("{re.escape(target_id)}",\s*"{re.escape(target_pin)}"\):\s*'
        rf'NodeData\("{re.escape(source_id)}",\s*"{re.escape(source_pin)}"\)'
    )
    return pattern.search(text) is not None


def node_time_edge(text, target_id, target_pin, source_id):
    pattern = re.compile(
        rf'NodeTime\("{re.escape(target_id)}",\s*"{re.escape(target_pin)}"\):\s*'
        rf'NodeTime\("{re.escape(source_id)}"\)'
    )
    return pattern.search(text) is not None


def explicit_root_chain(text):
    standing = "1a230000-0000-4000-8000-000000000017"
    crouching = "1a230000-0000-4000-8000-000000000018"
    grounded = "1a230000-0000-4000-8000-000000000001"
    transition = "1a230000-0000-4000-8000-000000000019"
    locomotion = "1a230000-0000-4000-8000-000000000003"
    post_locomotion = "1a230000-0000-4000-8000-000000000020"
    layering = "1a230000-0000-4000-8000-000000000009"
    head = "1a230000-0000-4000-8000-000000000010"
    control_rig = "1a230000-0000-4000-8000-000000000011"
    ragdolling = "1a230000-0000-4000-8000-000000000012"
    zero = "1a230000-0000-4000-8000-000000000013"
    standing_curves = [
        "PoseStanding",
        "PoseMoving",
        "RotationYawOffset",
        "FootLeftLock",
        "FootRightLock",
        "AllowTransitions",
        "RotationYawSpeed",
    ]
    crouching_curves = [
        "PoseCrouching",
        "PoseMoving",
        "RotationYawOffset",
        "FootLeftLock",
        "FootRightLock",
        "AllowTransitions",
        "RotationYawSpeed",
    ]
    slot_curves = [
        "PoseStanding",
        "PoseCrouching",
        "PoseMoving",
        "RotationYawOffset",
        "FootLeftLock",
        "FootRightLock",
        "AllowTransitions",
        "RotationYawSpeed",
        "PoseGrounded",
        "FootLeftIk",
        "FootRightIk",
    ]
    base = [
        node_data_edge(text, grounded, "Standing Input", standing, "pose"),
        node_time_edge(text, grounded, "standing_time", standing),
        node_data_edge(text, grounded, "Crouching Input", crouching, "pose"),
        node_time_edge(text, grounded, "crouching_time", crouching),
        node_data_edge(text, transition, "Source", grounded, "pose"),
        node_time_edge(text, transition, "source_time", grounded),
        node_data_edge(text, locomotion, "Grounded Input", transition, "pose"),
        node_time_edge(text, locomotion, "grounded_time", transition),
        node_data_edge(text, post_locomotion, "Source", locomotion, "pose"),
        node_time_edge(text, post_locomotion, "source_time", locomotion),
        node_data_edge(text, layering, "Locomotion Input", post_locomotion, "pose"),
        node_data_edge(text, head, "View Input", layering, "pose"),
        node_data_edge(text, control_rig, "Source", head, "pose"),
        node_data_edge(text, ragdolling, "Ragdolling Input", control_rig, "pose"),
        node_time_edge(text, layering, "locomotion_time_local_left_mesh", post_locomotion),
        node_data_edge(text, transition, "source_PoseInAir", zero, "value"),
        node_data_edge(text, locomotion, "grounded_PoseInAir", transition, "PoseInAir"),
        node_data_edge(text, post_locomotion, "source_PoseInAir", locomotion, "PoseInAir"),
        node_data_edge(text, layering, "locomotion_PoseInAir", post_locomotion, "PoseInAir"),
    ]
    standing_links = [
        node_data_edge(text, grounded, f"standing_{curve}", standing, curve)
        for curve in standing_curves
    ]
    crouching_links = [
        node_data_edge(text, grounded, f"crouching_{curve}", crouching, curve)
        for curve in crouching_curves
    ]
    slot_links = []
    for curve in slot_curves:
        slot_links.extend(
            [
                node_data_edge(text, transition, f"source_{curve}", grounded, curve),
                node_data_edge(text, locomotion, f"grounded_{curve}", transition, curve),
                node_data_edge(text, post_locomotion, f"source_{curve}", locomotion, curve),
                node_data_edge(text, layering, f"locomotion_{curve}", post_locomotion, curve),
            ]
        )
    return all(base + standing_links + crouching_links + slot_links)


def explicit_overlay_chain(text):
    reference = "1a230000-0000-4000-8000-000000000005"
    overlay = "1a230000-0000-4000-8000-000000000006"
    selector = "1a230000-0000-4000-8000-000000000007"
    inertialization = "1a230000-0000-4000-8000-000000000021"
    layering = "1a230000-0000-4000-8000-000000000009"
    curves = [
        "LayerArmLeft",
        "LayerArmLeftAdditive",
        "LayerArmLeftLocalSpace",
        "LayerArmRight",
        "LayerArmRightAdditive",
        "LayerArmRightLocalSpace",
        "LayerHead",
        "LayerHeadAdditive",
        "LayerLegs",
        "LayerPelvis",
        "LayerSpine",
        "LayerSpineAdditive",
        "HandLeftIk",
        "HandRightIk",
        "HipsDirectionLock",
        "LayerHandLeft",
        "LayerHandRight",
        "PoseAiming",
    ]
    base = [
        node_data_edge(text, overlay, "layer_index", selector, "als_root_overlay_index"),
        node_data_edge(text, overlay, "base_pose", reference, "pose"),
        node_time_edge(text, overlay, "base_time", reference),
        node_data_edge(text, inertialization, "Source", overlay, "pose"),
        node_time_edge(text, inertialization, "source_time", overlay),
        node_data_edge(text, layering, "overlay_pose", inertialization, "pose"),
        node_time_edge(text, layering, "overlay_time_legs", inertialization),
    ]
    curve_links = []
    for curve in curves:
        curve_links.extend(
            [
                node_data_edge(text, inertialization, f"source_{curve}", overlay, curve),
                node_data_edge(text, layering, f"overlay_{curve}", inertialization, curve),
            ]
        )
    output_pattern = re.compile(
        rf'OutputData\("transition_left_request"\):\s*'
        rf'NodeData\("{re.escape(inertialization)}",\s*"transition_left_request"\)'
    )
    return all(base + curve_links) and output_pattern.search(text) is not None


def runtime_context_input(text, node_id, pose_input, time_input, curve_prefix, missing_value):
    value = "None" if missing_value is None else f"Some({missing_value:.1f})"
    fragment = (
        f'(pose_input: "{pose_input}", time_input: "{time_input}", '
        f'curve_prefix: "{curve_prefix}", missing_curve_value: {value})'
    )
    return fragment in text and f'"{node_id}"' in text


def runtime_context_root_chain(text):
    standing = "1a230000-0000-4000-8000-000000000017"
    crouching = "1a230000-0000-4000-8000-000000000018"
    grounded = "1a230000-0000-4000-8000-000000000001"
    transition = "1a230000-0000-4000-8000-000000000019"
    locomotion = "1a230000-0000-4000-8000-000000000003"
    post_locomotion = "1a230000-0000-4000-8000-000000000020"
    layering = "1a230000-0000-4000-8000-000000000009"
    head = "1a230000-0000-4000-8000-000000000010"
    control_rig = "1a230000-0000-4000-8000-000000000011"
    ragdolling = "1a230000-0000-4000-8000-000000000012"
    base = [
        node_data_edge(text, grounded, "Standing Input", standing, "pose"),
        node_data_edge(text, grounded, "Crouching Input", crouching, "pose"),
        node_data_edge(text, transition, "Source", grounded, "pose"),
        node_data_edge(text, locomotion, "Grounded Input", transition, "pose"),
        node_data_edge(text, post_locomotion, "Source", locomotion, "pose"),
        node_data_edge(text, layering, "Locomotion Input", post_locomotion, "pose"),
        node_data_edge(text, head, "View Input", layering, "pose"),
        node_data_edge(text, control_rig, "Source", head, "pose"),
        node_data_edge(text, ragdolling, "Ragdolling Input", control_rig, "pose"),
        runtime_context_input(
            text, grounded, "Standing Input", "standing_time", "standing_", 0.0
        ),
        runtime_context_input(text, grounded, "Crouching Input", "crouching_time", "crouching_", 0.0),
        runtime_context_input(text, transition, "Source", "source_time", "source_", 0.0),
        runtime_context_input(text, locomotion, "Grounded Input", "grounded_time", "grounded_", None),
        runtime_context_input(text, post_locomotion, "Source", "source_time", "source_", None),
        runtime_context_input(
            text,
            layering,
            "Locomotion Input",
            "locomotion_time_local_left_mesh",
            "locomotion_",
            0.0,
        ),
    ]
    return (
        all(base)
        and 'name: "root zero curve"' not in text
        and "NodeTime(" not in text
        and "input_times: []" in text
        and "output_time: false" in text
        and "hide_context_auxiliary_outputs: true" in text
        and 'exposed_output_data: ["ragdoll_config"]' in text
    )


def runtime_context_overlay_chain(text):
    overlay = "1a230000-0000-4000-8000-000000000006"
    inertialization = "1a230000-0000-4000-8000-000000000021"
    layering = "1a230000-0000-4000-8000-000000000009"
    base = [
        node_data_edge(text, inertialization, "Source", overlay, "pose"),
        node_data_edge(text, layering, "Overlay Input", inertialization, "pose"),
        runtime_context_input(text, layering, "Overlay Input", "overlay_time_legs", "overlay_", 0.0),
    ]
    return (
        all(base)
        and 'selector_global_key: Some("als_root_overlay_index")' in text
        and 'global_data_inputs: [(input: "base_pose", global_key: "als_root_reference_pose")]'
        in text
        and 'name: "AB_Als Overlay local reference pose"' not in text
    )


def audit(source, contract, target_text, mapping):
    differences = []
    if contract.get("source_asset") != source.get("asset_path"):
        differences.append("contract source asset")
    if contract.get("source_class") != source.get("asset_class"):
        differences.append("contract source class")
    if canonical_source_payload(contract.get("source_payload")) != canonical_source_payload(source):
        differences.append("contract source payload")
    expected_source_hash = contract.get("source_semantic_sha256")
    if expected_source_hash is None:
        expected_source_hash = canonical_source_hash(contract.get("source_payload"))
    if expected_source_hash != canonical_source_hash(source):
        differences.append("contract source hash")
    anim_graph = find_graph(source, "AnimGraph")
    source_graphs = {graph["object_path"]: graph for graph in source["graphs"]}
    source_by_path = all_source_nodes(source_graphs)
    root_entry = next(
        (
            entry
            for entry in mapping["graphs"]
            if entry["source_suffix"] == "AnimGraph" and not entry.get("graph_scope_only")
        ),
        None,
    )
    if root_entry is None:
        differences.append("root layout entry")
        return differences, {}
    nodes = target_nodes(target_text)
    node_ids = {node["id"] for node in nodes}
    overrides = root_entry.get("node_overrides", [])
    override_ids = {entry["target_id"] for entry in overrides}
    if node_ids != override_ids:
        differences.append("layout target node coverage")
    source_index_set = {
        node["index"] for node in source_nodes(anim_graph) if node["index"] != 0
    }
    mapped_indices = {index for entry in overrides for index in source_indices(entry)}
    if not source_index_set.issubset(mapped_indices):
        differences.append("layout source node coverage")
    graph_scope_suffixes = {
        entry["source_suffix"]
        for entry in mapping["graphs"]
        if entry.get("graph_scope_only")
    }
    if not {"Overlay", "EventGraph"}.issubset(graph_scope_suffixes):
        differences.append("graph scope coverage")
    positions, node_rows = place_nodes(
        nodes,
        target_edges(target_text),
        source_nodes(anim_graph),
        source_by_path,
        source_owner(anim_graph, source_by_path),
        overrides,
    )
    actual_positions = {
        entry["id"]: entry["position"]
        for entry in parse_positions(target_text, "node_positions")["entries"]
    }
    if actual_positions != positions:
        differences.append("target node positions")
    for name, expected in root_entry.get("editor_positions", {}).items():
        if editor_position(target_text, name) != expected:
            differences.append(f"editor position {name}")
    required_fragments = [
        'name: "AB_Als_Standing"',
        'name: "AB_Als_Crouching"',
        'name: "AB_Als_Grounded"',
        'name: "Slot \'Transition\'"',
        'name: "AB_Als_Locomotion"',
        'name: "Slot \'PostLocomotion\'"',
        'graph: "graphs/ab_als/root_slot_relay.animgraph.ron"',
        'slot_name: Some("Transition")',
        'slot_name: Some("PostLocomotion")',
        "always_update_source_pose: false",
        'name: "ALI_Overlay - Overlay"',
        'name: "Inertialization"',
        '"als_ext::InertializationPassthroughNode"',
        'selector: "layer_index"',
        "reset_on_becoming_relevant: true",
        "forward_requests_through_skipped_cached_poses: true",
        "capture_context: true",
        "internalize_time: true",
        'name: "ControlRig CR_Als"',
        'rig_class: "CR_Als"',
        "alpha_enabled: true",
        "transfer_input_pose: true",
        "transfer_input_curves: true",
        "transfer_pose_in_global_space: false",
        'graph: "graphs/ab_als/layering.animgraph.ron"',
        'graph: "graphs/ab_als/head.animgraph.ron"',
        'graph: "graphs/ab_als/ragdolling.animgraph.ron"',
        'implementations: ["graphs/ab_als/overlays/default.animgraph.ron"',
        'OutputData("ragdoll_config")',
    ]
    for fragment in required_fragments:
        if fragment not in target_text:
            differences.append(f"target fragment {fragment}")
    runtime_context = "internalize_time: true" in target_text
    root_chain = (
        runtime_context_root_chain(target_text)
        if runtime_context
        else explicit_root_chain(target_text)
    )
    if not root_chain:
        differences.append("runtime root context chain")
    overlay_chain = (
        runtime_context_overlay_chain(target_text)
        if runtime_context
        else explicit_overlay_chain(target_text)
    )
    if not overlay_chain:
        differences.append("runtime overlay context chain")
    report = {
        "source_asset": source["asset_path"],
        "source_semantic_sha256": canonical_source_hash(source),
        "source_graphs": len(source["graphs"]),
        "source_nodes": sum(len(graph["nodes"]) for graph in source["graphs"]),
        "source_pins": sum(len(node["pins"]) for graph in source["graphs"] for node in graph["nodes"]),
        "source_asset_objects": len(source["asset_objects"]),
        "anim_graph_source_nodes": len(anim_graph["nodes"]),
        "anim_graph_source_pins": sum(len(node["pins"]) for node in anim_graph["nodes"]),
        "target_nodes": len(nodes),
        "layout_nodes": node_rows,
        "runtime_context": runtime_context,
        "explicit_root_chain": root_chain,
        "explicit_overlay_chain": overlay_chain,
        "differences": differences,
        "pass": not differences,
    }
    return differences, report


def self_test():
    payload = {"value": [1, 2, 3]}
    if canonical_hash(payload) != canonical_hash({"value": [1, 2, 3]}):
        raise AssertionError("canonical hash")
    if editor_position("output_position: (0.0, 0.0)", "output_position") != [0.0, 0.0]:
        raise AssertionError("editor position")
    source = {
        "blueprint_cdo_path": "legacy",
        "graphs": [
            {
                "properties": [{"name": "GraphGuid", "exported_text": "old"}],
                "nodes": [
                    {
                        "pins": [
                            {
                                "pin_id": "old",
                                "linked_to": [{"pin_id": "peer", "pin_name": "Pose"}],
                            }
                        ]
                    }
                ],
            }
        ],
    }
    volatile = {
        "graphs": [
            {
                "properties": [{"name": "GraphGuid", "exported_text": "new"}],
                "nodes": [
                    {
                        "pins": [
                            {
                                "pin_id": "new",
                                "linked_to": [{"pin_id": "other", "pin_name": "Pose"}],
                            }
                        ]
                    }
                ],
            }
        ],
    }
    if canonical_source_hash(source) != canonical_source_hash(volatile):
        raise AssertionError("volatile source fields")
    semantic = json.loads(json.dumps(volatile))
    semantic["graphs"][0]["nodes"][0]["pins"][0]["linked_to"][0]["pin_name"] = "Other"
    if canonical_source_hash(source) == canonical_source_hash(semantic):
        raise AssertionError("semantic source fields")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, default=DEFAULT_SOURCE)
    parser.add_argument("--contract", type=Path, default=DEFAULT_CONTRACT)
    parser.add_argument("--target", type=Path, default=DEFAULT_TARGET)
    parser.add_argument("--mapping", type=Path, default=DEFAULT_MAPPING)
    parser.add_argument("--report", type=Path, default=DEFAULT_REPORT)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    self_test()
    if args.self_test:
        print("AB_Als root audit self-test PASS")
        return
    source = json.loads(args.source.read_text(encoding="utf-8"))
    contract = json.loads(args.contract.read_text(encoding="utf-8"))
    target_text = args.target.read_text(encoding="utf-8")
    mapping = json.loads(args.mapping.read_text(encoding="utf-8"))
    differences, report = audit(source, contract, target_text, mapping)
    if not args.check:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(
        "AB_Als root audit "
        f"graphs={report.get('source_graphs', 0)} "
        f"nodes={report.get('source_nodes', 0)} "
        f"pins={report.get('source_pins', 0)} "
        f"target_nodes={report.get('target_nodes', 0)} "
        f"differences={len(differences)} "
        f"-> {'PASS' if not differences else 'FAIL'}"
    )
    if differences:
        for difference in differences:
            print(f"DIFF {difference}")
        raise SystemExit(1)


if __name__ == "__main__":
    main()
