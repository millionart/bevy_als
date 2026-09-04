import json
import re
import sys
import uuid
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools" / "import"))

from extract_ue_visible_graph_contract import contract
import generate_ue_transition_expression_graph as transition_renderer
from ue_animgraph_pose_renderer import PoseRenderError, render as render_pose_graph


class MaterializationError(ValueError):
    pass


def quoted(value):
    return json.dumps(value, ensure_ascii=False)


def stable_id(source_asset, category, value):
    return str(uuid.uuid5(uuid.NAMESPACE_URL, f"bevy-als/{category}/{source_asset}/{value}"))


def graph_path(source_asset, suffix):
    return f"graphs/{stable_id(source_asset, 'graph', suffix)}.animgraph.ron"


def fsm_path(source_asset, suffix):
    return f"fsms/{stable_id(source_asset, 'fsm', suffix)}.fsm.ron"


def transition_path(source_asset, transition_id):
    return f"transitions/{stable_id(source_asset, 'transition', transition_id)}.animgraph.ron"


def synthetic_state_path(source_asset, state_id):
    return f"states/{stable_id(source_asset, 'synthetic-state', state_id)}.animgraph.ron"


def graph_for_suffix(source, suffix):
    path = f"{source['asset_path']}:{suffix}"
    matches = [graph for graph in source.get("graphs", []) if graph.get("object_path") == path]
    if len(matches) != 1:
        raise MaterializationError("source graph is missing or ambiguous")
    return matches[0]


def graph_nodes_by_path(source, suffix):
    return {
        node.get("object_path"): node
        for node in graph_for_suffix(source, suffix).get("nodes", [])
        if isinstance(node.get("object_path"), str)
    }


def structural_order(source, plan):
    nodes = graph_nodes_by_path(source, plan["source_graph"])
    actual = [*plan["states"], *plan["conduits"]]
    aliases = plan["aliases"]
    if any(item["source_node"] not in nodes for item in [*actual, *aliases]):
        raise MaterializationError("source FSM structural node is missing")
    actual.sort(key=lambda item: nodes[item["source_node"]].get("index", -1))
    aliases.sort(key=lambda item: nodes[item["source_node"]].get("index", -1))
    entry = f"{source['asset_path']}:{plan['entry_target']}"
    entry_index = next(
        (index for index, item in enumerate(actual) if item["source_node"] == entry),
        None,
    )
    if entry_index is None:
        raise MaterializationError("source FSM entry target is unsupported")
    actual.insert(0, actual.pop(entry_index))
    return actual, aliases


def placeholder_graph_text(source_asset, state_id):
    node = stable_id(source_asset, "placeholder", state_id)
    return "(\n" + "\n".join(
        (
            "    nodes: [",
            f"        (id: {quoted(node)}, name: \"Source conduit or alias\", inner: {{ \"als_ext::LocalRefPoseNode\": (internalize_time: true) }}, should_debug: false),",
            "    ],",
            "    edges_inverted: {",
            f"        OutputData(\"Result\"): NodeData({quoted(node)}, \"pose\"),",
            "    },",
            "    io_spec: (input_times: [], input_data: {}, output_time: false, output_data: { \"Result\": Pose }, input_order: {}, output_order: { Data(\"Result\"): 0 }),",
            "    default_data: {},",
            "    editor_metadata: (node_positions: {}, input_position: (0.0, 0.0), output_position: (0.0, 0.0), input_param_order: {}, input_time_order: {}, output_data_order: {}, output_pose_order: {}),",
            ")",
            "",
        )
    )


def transition_graph_text(source_asset, transition):
    render = transition["render"]
    node = stable_id(source_asset, "transition-node", transition["id"])
    position = transition.get("position", [0.0, 0.0])
    if (
        not isinstance(position, list)
        or len(position) != 2
        or not all(isinstance(value, (int, float)) for value in position)
    ):
        raise MaterializationError("source transition position is invalid")
    duration = render["duration"]
    if render["logic"] == "inertialization":
        node_type = "als_ext::InertializationNode"
        inner = f"(transition_duration: {duration!r}, half_life: {(duration / 4.0)!r})"
    else:
        profile = render["blend_profile"]
        scales = profile.get("bone_scales", [])
        if not isinstance(scales, list):
            raise MaterializationError("source transition blend profile is invalid")
        scale_ron = ", ".join(
            f"(bone: {quoted(scale['bone'])}, scale: {scale['scale']!r})"
            for scale in scales
        )
        node_type = "als_ext::ProfiledBlendNode"
        inner = (
            "(curve_names: [], bone_scales: ["
            + scale_ron
            + "], alpha_blend_mode: "
            + render["blend_mode"]
            + ", custom_curve: [])"
        )
    return "(\n" + "\n".join(
        (
            "    nodes: [",
            f"        (id: {quoted(node)}, name: {quoted(transition['title'])}, inner: {{ {quoted(node_type)}: {inner} }}, should_debug: false),",
            "    ],",
            "    edges_inverted: {",
            f"        NodeData({quoted(node)}, \"factor\"): InputData(FsmBuiltin(PercentThroughDuration)),",
            f"        NodeData({quoted(node)}, \"pose_a\"): InputData(FromFsmSource(\"pose\")),",
            f"        NodeTime({quoted(node)}, \"time_a\"): InputTime(FromFsmSource(\"time\")),",
            f"        NodeData({quoted(node)}, \"pose_b\"): InputData(FromFsmTarget(\"pose\")),",
            f"        NodeTime({quoted(node)}, \"time_b\"): InputTime(FromFsmTarget(\"time\")),",
            f"        OutputData(\"pose\"): NodeData({quoted(node)}, \"pose\"),",
            f"        OutputTime: NodeTime({quoted(node)}),",
            "    },",
            "    io_spec: (input_times: [FromFsmSource(\"time\"), FromFsmTarget(\"time\")], input_data: { FsmBuiltin(PercentThroughDuration): F32, FromFsmSource(\"pose\"): Pose, FromFsmTarget(\"pose\"): Pose }, output_time: true, output_data: { \"pose\": Pose }, input_order: { Time(FromFsmSource(\"time\")): 0, Time(FromFsmTarget(\"time\")): 1, Data(FsmBuiltin(PercentThroughDuration)): 2, Data(FromFsmSource(\"pose\")): 3, Data(FromFsmTarget(\"pose\")): 4 }, output_order: { Data(\"pose\"): 0, Time: 1 }),",
            "    default_data: {},",
            f"    editor_metadata: (node_positions: {{ {quoted(node)}: ({float(position[0]):.3f}, {float(position[1]):.3f}) }}, input_position: (0.0, 0.0), output_position: (0.0, 0.0), input_param_order: {{}}, input_time_order: {{}}, output_data_order: {{}}, output_pose_order: {{}}),",
            ")",
            "",
        )
    )


def fsm_config(source, plan, registry, graph_paths):
    source_asset = source["asset_path"]
    actual, aliases = structural_order(source, plan)
    state_ids = {item["source_node"]: item["id"] for item in [*actual, *aliases]}
    label_keys = registry.get("fsm_state_label_keys", {})
    key = label_keys.get(graph_for_suffix(source, plan["source_graph"]).get("name"))
    if not isinstance(key, str) or not key:
        raise MaterializationError("source FSM state label provider is unavailable")
    alias_bindings = []
    for alias in aliases:
        members = []
        for suffix in alias["members"]:
            source_node = f"{source_asset}:{suffix}"
            member = state_ids.get(source_node)
            if member is None:
                raise MaterializationError("source FSM alias member is unavailable")
            members.append(member)
        alias_bindings.append({"state_id": alias["id"], "member_state_ids": members})
    return {
        "path": fsm_path(source_asset, plan["source_graph"]),
        "state_label_key": key,
        "state_labels": [item["title"] for item in [*actual, *aliases]],
        "state_aliases": alias_bindings,
        "actual": actual,
        "aliases": aliases,
        "state_ids": state_ids,
        "graph_paths": graph_paths,
    }


def fsm_text(source, plan, config, graph_paths):
    source_asset = source["asset_path"]
    state_rows = []
    files = {}
    for item in [*config["actual"], *config["aliases"]]:
        state_path = None
        if "graph" in item:
            state_path = graph_paths.get(item["graph"])
        if state_path is None:
            state_path = synthetic_state_path(source_asset, item["id"])
            files[state_path] = placeholder_graph_text(source_asset, item["id"])
        state_rows.append(
            "(id: "
            + quoted(item["id"])
            + ", label: "
            + quoted(item["title"])
            + ", graph: "
            + quoted(state_path)
            + ", state_transition: None)"
        )
    transitions = []
    for transition in plan["transitions"]:
        source_node = f"{source_asset}:{transition['source']}"
        target_node = f"{source_asset}:{transition['target']}"
        source_id = config["state_ids"].get(source_node)
        target_id = config["state_ids"].get(target_node)
        if source_id is None or target_id is None:
            raise MaterializationError("source FSM transition endpoint is unavailable")
        path = transition_path(source_asset, transition["id"])
        files[path] = transition_graph_text(source_asset, transition)
        transitions.append(
            "(id: "
            + quoted(transition["id"])
            + ", source: "
            + quoted(source_id)
            + ", target: "
            + quoted(target_id)
            + ", data: (kind: Graph(graph: "
            + quoted(path)
            + ", timed: Some("
            + repr(transition["render"]["duration"])
            + ")), ignore_external_events: false, reset_target_state: true))"
        )
    entry = config["state_ids"].get(f"{source_asset}:{plan['entry_target']}")
    if entry is None:
        raise MaterializationError("source FSM entry state is unavailable")
    positions = ", ".join(
        f"{quoted(item['id'])}: ({item['position'][0]!r}, {item['position'][1]!r})"
        for item in [*config["actual"], *config["aliases"]]
    )
    text = "(\n" + "\n".join(
        (
            "    states: [",
            *[f"        {row}," for row in state_rows],
            "    ],",
            "    transitions: [",
            *[f"        {row}," for row in transitions],
            "    ],",
            f"    start_state: {quoted(entry)},",
            "    node_spec: (input_times: [], input_data: {}, output_time: false, output_data: { \"Pose\": Pose }, input_order: {}, output_order: { Data(\"Pose\"): 0 }),",
            f"    editor_metadata: (states: {{ {positions} }}),",
            ")",
            "",
        )
    )
    return text, files


def materialized_reference_issues(files):
    issues = []
    for path, text in files.items():
        if not path.endswith((".animgraph.ron", ".fsm.ron")):
            continue
        for reference in re.findall(r"(?:graph|fsm):\s*\"([^\"]+)\"", text):
            if reference.endswith((".animgraph.ron", ".fsm.ron")) and reference not in files:
                issues.append(f"{path}: missing generated reference {reference}")
    return sorted(set(issues))


def materialize_files(source, ir, registry):
    source_asset = source.get("asset_path")
    if source_asset != ir.get("source_asset"):
        raise MaterializationError("source IR asset is invalid")
    graph_entries = {
        entry["source_graph"]: entry
        for entry in ir.get("graphs", [])
        if entry.get("kind") == "graph"
    }
    graph_paths = {
        suffix: graph_path(source_asset, suffix) for suffix in graph_entries
    }
    fsm_entries = [
        entry
        for entry in ir.get("graphs", [])
        if entry.get("kind") == "state_machine" and isinstance(entry.get("fsm_renderer"), dict)
    ]
    fsm_configs = {}
    for entry in fsm_entries:
        plan = entry["fsm_renderer"]
        fsm_configs[plan["source_graph"]] = fsm_config(source, plan, registry, graph_paths)
    files = {}
    for entry in fsm_entries:
        plan = entry["fsm_renderer"]
        config = fsm_configs[plan["source_graph"]]
        text, extras = fsm_text(source, plan, config, graph_paths)
        files[config["path"]] = text
        files.update(extras)
    for suffix, entry in graph_entries.items():
        try:
            text = render_pose_graph(
                contract(source, suffix),
                entry,
                ir["animation_assets"],
                registry["sync_group_phase_keys"],
                fsm_configs,
            )
        except PoseRenderError as error:
            raise MaterializationError(f"source pose graph is not materializable: {suffix}") from error
        files[graph_paths[suffix]] = text
    condition_paths = {}
    for entry in ir.get("graphs", []):
        if entry.get("kind") != "transition":
            continue
        suffix = entry["source_graph"]
        path = f"conditions/{stable_id(source_asset, 'condition', suffix)}.animgraph.ron"
        files[path] = (
            transition_renderer.graph_text(contract(source, suffix))
        )
        condition_paths[suffix] = path
    issues = materialized_reference_issues(files)
    if issues:
        raise MaterializationError("; ".join(issues))
    manifest = {
        "format": "ue_animgraph_materialization_manifest_v1",
        "root_graph": graph_paths.get(ir["root_source_graph"]),
        "graphs": graph_paths,
        "fsms": {key: value["path"] for key, value in fsm_configs.items()},
        "conditions": condition_paths,
        "fsm_state_label_keys": {
            key: value["state_label_key"] for key, value in fsm_configs.items()
        },
        "lifecycle_dispatch": ir.get("lifecycle_dispatch"),
    }
    files["materialization.manifest.json"] = json.dumps(
        manifest, ensure_ascii=False, indent=2
    ) + "\n"
    return files


def run_self_test():
    standard = {
        "id": "10000000-0000-4000-8000-000000000001",
        "title": "Standard",
        "render": {
            "logic": "profiled_blend",
            "duration": 0.2,
            "blend_mode": "HermiteCubic",
            "blend_profile": {"bone_scales": []},
        },
    }
    inertial = {
        "id": "10000000-0000-4000-8000-000000000002",
        "title": "Inertial",
        "render": {
            "logic": "inertialization",
            "duration": 0.1,
            "blend_mode": "HermiteCubic",
            "blend_profile": {"bone_scales": []},
        },
    }
    assert "ProfiledBlendNode" in transition_graph_text("/Asset", standard)
    assert "InertializationNode" in transition_graph_text("/Asset", inertial)
    assert graph_path("/Asset", "AnimGraph") == graph_path("/Asset", "AnimGraph")
    assert graph_path("/Asset", "AnimGraph") != fsm_path("/Asset", "AnimGraph")
    assert materialized_reference_issues(
        {"graphs/root.animgraph.ron": '(fsm: "fsms/missing.fsm.ron")'}
    )
    return {
        "standard_and_inertial_transitions_materialized": "PASS",
        "source_paths_are_deterministic": "PASS",
        "missing_generated_reference_rejected": "PASS",
    }


if __name__ == "__main__":
    print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
