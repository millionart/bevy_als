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


def asset_namespace(value):
    if value is None or value == "":
        return ""
    if not isinstance(value, str):
        raise MaterializationError("asset namespace is invalid")
    parts = value.split("/")
    if not all(re.fullmatch(r"[A-Za-z0-9_-]+", part) for part in parts):
        raise MaterializationError("asset namespace is invalid")
    return "/".join(parts)


def namespaced_path(namespace, path):
    namespace = asset_namespace(namespace)
    return f"{namespace}/{path}" if namespace else path


def source_path_segment(value):
    if not isinstance(value, str) or not value:
        raise MaterializationError("source path segment is invalid")
    value = re.sub(r"[^A-Za-z0-9_-]+", "_", value).strip("_")
    if not value:
        raise MaterializationError("source path segment is invalid")
    return value


def source_asset_directory(source_asset):
    if not isinstance(source_asset, str) or not source_asset.startswith("/"):
        raise MaterializationError("source asset path is invalid")
    parts = [part for part in source_asset.split("/") if part]
    if not parts:
        raise MaterializationError("source asset path is invalid")
    relative = parts[1:] if len(parts) > 1 else parts
    leaf = relative[-1].rsplit(".", 1)[0]
    if not leaf:
        raise MaterializationError("source asset path is invalid")
    return "/".join([*(source_path_segment(part) for part in relative[:-1]), source_path_segment(leaf)])


STRUCTURAL_SOURCE_NODE_PREFIXES = (
    ("AnimGraphNode_StateMachine_", "SM"),
    ("AnimStateNode_", "S"),
    ("AnimStateTransitionNode_", "T"),
    ("AnimStateConduitNode_", "C"),
    ("AnimStateAliasNode_", "A"),
)


def structural_source_node_segment(value):
    for prefix, label in STRUCTURAL_SOURCE_NODE_PREFIXES:
        suffix = value.removeprefix(prefix)
        if suffix and suffix.isdecimal():
            return label + suffix
    return None


def source_graph_parts(suffix):
    if not isinstance(suffix, str) or not suffix:
        raise MaterializationError("source graph path is invalid")
    source_parts = suffix.split(".")
    if any(not part for part in source_parts):
        raise MaterializationError("source graph path is invalid")
    parts = []
    index = 0
    while index < len(source_parts):
        node = structural_source_node_segment(source_parts[index])
        if node is not None and index + 1 < len(source_parts):
            title = structural_source_node_segment(source_parts[index + 1])
            if title is None:
                parts.append(node + "_" + source_path_segment(source_parts[index + 1]))
                index += 2
                continue
        parts.append(node or source_path_segment(source_parts[index]))
        index += 1
    return parts


def source_graph_path(source_asset, suffix, extension, namespace=""):
    if not isinstance(extension, str) or not extension.startswith("."):
        raise MaterializationError("source graph extension is invalid")
    parts = source_graph_parts(suffix)
    base = namespaced_path(namespace, source_asset_directory(source_asset))
    return "/".join([base, *parts[:-1], parts[-1] + extension])


def graph_path(source_asset, suffix, namespace=""):
    return source_graph_path(source_asset, suffix, ".animgraph.ron", namespace)


def fsm_path(source_asset, suffix, namespace=""):
    return source_graph_path(source_asset, suffix, ".fsm.ron", namespace)


def transition_path(source_asset, transition, namespace="", variant=None):
    if not isinstance(transition, dict):
        raise MaterializationError("source transition is invalid")
    source_node = transition.get("source_node")
    prefix = source_asset + ":"
    if not isinstance(source_node, str) or not source_node.startswith(prefix):
        raise MaterializationError("source transition node is invalid")
    parts = source_graph_parts(source_node[len(prefix) :])
    base = namespaced_path(namespace, source_asset_directory(source_asset))
    suffix = "Transition"
    if variant is not None:
        if not isinstance(variant, str) or not variant:
            raise MaterializationError("source transition variant is invalid")
        suffix += "_" + source_path_segment(variant)
    return "/".join([base, *parts, suffix + ".transition.animgraph.ron"])


def condition_path(source_asset, suffix, namespace=""):
    return source_graph_path(source_asset, suffix, ".condition.animgraph.ron", namespace)


def synthetic_state_path(source_asset, state, namespace=""):
    if not isinstance(state, dict):
        raise MaterializationError("source synthetic state is invalid")
    source_node = state.get("source_node")
    title = state.get("title")
    if not isinstance(source_node, str) or not isinstance(title, str):
        raise MaterializationError("source synthetic state is invalid")
    stem = source_path_segment(source_node.rsplit(".", 1)[-1]) + "_" + source_path_segment(title)
    return namespaced_path(
        namespace,
        source_asset_directory(source_asset) + "/synthetic_states/" + stem + ".animgraph.ron",
    )


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


def structural_order(source, plan, entry_target):
    nodes = graph_nodes_by_path(source, plan["source_graph"])
    actual = list(plan["states"])
    aliases = plan["aliases"]
    if any(item["source_node"] not in nodes for item in [*actual, *aliases, *plan["conduits"]]):
        raise MaterializationError("source FSM structural node is missing")
    actual.sort(key=lambda item: nodes[item["source_node"]].get("index", -1))
    aliases.sort(key=lambda item: nodes[item["source_node"]].get("index", -1))
    entry = f"{source['asset_path']}:{entry_target}"
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
            f"    editor_metadata: (node_positions: {{ {quoted(node)}: (0.0, 0.0) }}, input_position: (0.0, 0.0), output_position: (0.0, 0.0), input_param_order: {{}}, input_time_order: {{}}, output_data_order: {{}}, output_pose_order: {{}}),",
            ")",
            "",
        )
    )


def rich_curve_ron(keys):
    fields = (
        "interpolation",
        "tangent_mode",
        "tangent_weight_mode",
        "t",
        "v",
        "arrive_tangent",
        "arrive_weight",
        "leave_tangent",
        "leave_weight",
    )
    if not isinstance(keys, list):
        raise MaterializationError("source custom curve is invalid")
    rows = []
    for key in keys:
        if not isinstance(key, dict) or set(key) != set(fields):
            raise MaterializationError("source custom curve key is invalid")
        if key["interpolation"] not in {"Constant", "Linear", "Cubic"}:
            raise MaterializationError("source custom curve interpolation is invalid")
        if key["tangent_mode"] not in {"Auto", "User", "Break", "None", "SmartAuto"}:
            raise MaterializationError("source custom curve tangent mode is invalid")
        if key["tangent_weight_mode"] not in {"None", "Arrive", "Leave", "Both"}:
            raise MaterializationError("source custom curve tangent weight mode is invalid")
        if not all(
            isinstance(key[field], (int, float)) and not isinstance(key[field], bool)
            for field in fields[3:]
        ):
            raise MaterializationError("source custom curve value is invalid")
        rows.append(
            "(interpolation: "
            + key["interpolation"]
            + ", tangent_mode: "
            + key["tangent_mode"]
            + ", tangent_weight_mode: "
            + key["tangent_weight_mode"]
            + ", t: "
            + repr(key["t"])
            + ", v: "
            + repr(key["v"])
            + ", arrive_tangent: "
            + repr(key["arrive_tangent"])
            + ", arrive_weight: "
            + repr(key["arrive_weight"])
            + ", leave_tangent: "
            + repr(key["leave_tangent"])
            + ", leave_weight: "
            + repr(key["leave_weight"])
            + ")"
        )
    return "[" + ", ".join(rows) + "]"


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
        custom_curve = rich_curve_ron(render.get("custom_curve", []))
        inner = (
            "(curve_names: [], bone_scales: ["
            + scale_ron
            + "], alpha_blend_mode: "
            + render["blend_mode"]
            + ", custom_curve: "
            + custom_curve
            + ")"
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


def condition_config(source, suffix, namespace):
    try:
        output = transition_renderer.root_output_name(contract(source, suffix))
    except ValueError as error:
        raise MaterializationError("source transition condition output is unavailable") from error
    return {
        "condition": condition_path(source["asset_path"], suffix, namespace),
        "output": output,
    }


def condition_context_config(source_asset, condition, scope):
    return {
        **condition,
        "context_state_id": stable_id(source_asset, "condition-context", scope),
    }


def resolved_fsm_transitions(source, plan, namespace):
    source_asset = source["asset_path"]
    conduits = {item["source_node"]: item for item in plan["conduits"]}
    transitions = sorted(
        plan["transitions"],
        key=lambda item: (item["source"], item["priority"], item["source_order"], item["id"]),
    )
    outgoing = {}
    for transition in transitions:
        outgoing.setdefault(transition["source"], []).append(transition)

    def resolve(root, current, gates, visited):
        target_node = f"{source_asset}:{current['target']}"
        conduit = conduits.get(target_node)
        if conduit is None:
            transition_id = (
                root["id"]
                if current is root
                else stable_id(
                    source_asset,
                    "conduit-transition",
                    "/".join([root["id"], current["id"], *[gate["condition"] for gate in gates]]),
                )
            )
            automatic_rule = root["render"].get("automatic_rule")
            if automatic_rule is not None and (
                not isinstance(automatic_rule, dict)
                or automatic_rule.get("source_semantics") != "relevant_asset_time_remaining"
                or not isinstance(automatic_rule.get("trigger_time"), (int, float))
            ):
                raise MaterializationError("source automatic transition rule is invalid")
            return [
                {
                    **current,
                    "id": transition_id,
                    "source": root["source"],
                    "condition_graph": root["condition_graph"],
                    "gates": gates,
                    "automatic_trigger_time": (
                        automatic_rule["trigger_time"] if automatic_rule is not None else None
                    ),
                    "automatic_transition_duration": (
                        0.0 if root["target"] != current["target"] else root["render"]["duration"]
                    ),
                }
            ]
        if target_node in visited:
            raise MaterializationError("source conduit transition cycle is unsupported")
        conduit_graph = plan["conduit_conditions"].get(target_node)
        if not isinstance(conduit_graph, str) or not conduit_graph:
            raise MaterializationError("source conduit condition is unavailable")
        conduit_gate = condition_config(source, conduit_graph, namespace)
        resolved = []
        for child in outgoing.get(current["target"], []):
            if child["render"].get("automatic_rule") is not None:
                raise MaterializationError("source conduit automatic transition is unsupported")
            child_gate = condition_config(source, child["condition_graph"], namespace)
            resolved.extend(
                resolve(
                    root,
                    child,
                    [*gates, conduit_gate, child_gate],
                    {*visited, target_node},
                )
            )
        if not resolved:
            raise MaterializationError("source conduit has no reachable content state")
        return resolved

    result = []
    for transition in transitions:
        if f"{source_asset}:{transition['source']}" in conduits:
            continue
        result.extend(resolve(transition, transition, [], set()))
    return result


def resolved_entry_conditions(source, plan, namespace):
    source_asset = source["asset_path"]
    conduits = {item["source_node"]: item for item in plan["conduits"]}
    transitions = sorted(
        plan["transitions"],
        key=lambda item: (item["source"], item["priority"], item["source_order"], item["id"]),
    )
    outgoing = {}
    for transition in transitions:
        outgoing.setdefault(transition["source"], []).append(transition)

    def resolve(target, gates, visited):
        target_node = f"{source_asset}:{target}"
        conduit = conduits.get(target_node)
        if conduit is None:
            return [{"target": target, "gates": gates}]
        if target_node in visited:
            raise MaterializationError("source entry conduit cycle is unsupported")
        conduit_graph = plan["conduit_conditions"].get(target_node)
        if not isinstance(conduit_graph, str) or not conduit_graph:
            raise MaterializationError("source entry conduit condition is unavailable")
        resolved = []
        for transition in outgoing.get(target, []):
            if transition["render"].get("automatic_rule") is not None:
                raise MaterializationError("source entry conduit automatic transition is unsupported")
            resolved.extend(
                resolve(
                    transition["target"],
                    [
                        *gates,
                        condition_config(source, conduit_graph, namespace),
                        condition_config(source, transition["condition_graph"], namespace),
                    ],
                    {*visited, target_node},
                )
            )
        if not resolved:
            raise MaterializationError("source entry conduit has no reachable content state")
        return resolved

    return resolve(plan["entry_target"], [], set())


def fsm_config(source, plan, graph_paths, namespace):
    source_asset = source["asset_path"]
    entry_conditions = resolved_entry_conditions(source, plan, namespace)
    entry_target = entry_conditions[0]["target"]
    actual, aliases = structural_order(source, plan, entry_target)
    runtime = plan.get("runtime", {})
    if not isinstance(runtime, dict):
        raise MaterializationError("source FSM runtime settings are invalid")
    max_transitions_per_frame = runtime.get("max_transitions_per_frame", 3)
    max_transition_requests = runtime.get("max_transition_requests", 32)
    skip_first_update_transition = runtime.get("skip_first_update_transition", True)
    reinitialize_on_becoming_relevant = runtime.get(
        "reinitialize_on_becoming_relevant", True
    )
    if (
        not isinstance(max_transitions_per_frame, int)
        or max_transitions_per_frame < 0
        or not isinstance(max_transition_requests, int)
        or max_transition_requests < 0
        or not isinstance(skip_first_update_transition, bool)
        or not isinstance(reinitialize_on_becoming_relevant, bool)
    ):
        raise MaterializationError("source FSM runtime settings are invalid")
    state_ids = {item["source_node"]: item["id"] for item in [*actual, *aliases]}
    reset_policies = {item["source_node"]: item["always_reset_on_entry"] for item in actual}
    entry_state_id = state_ids.get(f"{source_asset}:{entry_target}")
    if entry_state_id is None:
        raise MaterializationError("source entry target state is unavailable")
    source_entry_conditions = []
    for entry_index, entry in enumerate(entry_conditions):
        target_state_id = state_ids.get(f"{source_asset}:{entry['target']}")
        if target_state_id is None:
            raise MaterializationError("source entry condition target is unavailable")
        source_entry_conditions.append(
            {
                "target_state_id": target_state_id,
                "gates": [
                    condition_context_config(
                        source_asset,
                        gate,
                        f"{plan['source_graph']}/entry/{entry_index}/{gate_index}",
                    )
                    for gate_index, gate in enumerate(entry["gates"])
                ],
            }
        )
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
    transitions = resolved_fsm_transitions(source, plan, namespace)
    source_transition_conditions = []
    for transition in transitions:
        source_state = state_ids.get(f"{source_asset}:{transition['source']}")
        if source_state is None:
            raise MaterializationError("source transition condition state is unavailable")
        scope = f"{plan['source_graph']}/transition/{transition['id']}/{source_state}"
        main = condition_context_config(
            source_asset,
            condition_config(source, transition["condition_graph"], namespace),
            scope + "/main",
        )
        source_transition_conditions.append(
            {
                "transition_id": transition["id"],
                "source_state_id": source_state,
                **main,
                "gates": [
                    condition_context_config(
                        source_asset,
                        gate,
                        scope + f"/gate/{gate_index}",
                    )
                    for gate_index, gate in enumerate(transition["gates"])
                ],
                "notifications": transition["notifies"],
                "automatic_trigger_time": transition["automatic_trigger_time"],
                "transition_duration": transition["automatic_transition_duration"],
                "always_reset_target_on_entry": reset_policies[f"{source_asset}:{transition['target']}"],
            }
        )
    return {
        "path": fsm_path(source_asset, plan["source_graph"], namespace),
        "state_aliases": alias_bindings,
        "source_transition_conditions": source_transition_conditions,
        "source_entry_conditions": source_entry_conditions,
        "entry_state_id": entry_state_id,
        "actual": actual,
        "aliases": aliases,
        "state_ids": state_ids,
        "graph_paths": graph_paths,
        "transitions": transitions,
        "max_transitions_per_frame": max_transitions_per_frame,
        "max_transition_requests": max_transition_requests,
        "skip_first_update_transition": skip_first_update_transition,
        "reinitialize_on_becoming_relevant": reinitialize_on_becoming_relevant,
    }


def fsm_text(source, plan, config, graph_paths, namespace):
    source_asset = source["asset_path"]
    state_rows = []
    files = {}
    for item in [*config["actual"], *config["aliases"]]:
        state_path = None
        if "graph" in item:
            state_path = graph_paths.get(item["graph"])
        if state_path is None:
            state_path = synthetic_state_path(source_asset, item, namespace)
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
    for transition in config["transitions"]:
        source_node = f"{source_asset}:{transition['source']}"
        target_node = f"{source_asset}:{transition['target']}"
        source_id = config["state_ids"].get(source_node)
        target_id = config["state_ids"].get(target_node)
        if source_id is None or target_id is None:
            raise MaterializationError("source FSM transition endpoint is unavailable")
        path = transition_path(source_asset, transition, namespace)
        if path in files:
            path = transition_path(source_asset, transition, namespace, transition["id"])
        if path in files:
            raise MaterializationError("source transition output path is ambiguous")
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
    entry = config["entry_state_id"]
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


def materialized_reference_issues(files, external_references=()):
    issues = []
    external_references = set(external_references)
    for path, text in files.items():
        if not path.endswith((".animgraph.ron", ".fsm.ron")):
            continue
        for reference in re.findall(r"(?:graph|fsm|condition):\s*\"([^\"]+)\"", text):
            if (
                reference.endswith((".animgraph.ron", ".fsm.ron"))
                and reference not in files
                and reference not in external_references
            ):
                issues.append(f"{path}: missing generated reference {reference}")
    return sorted(set(issues))


def linked_layer_targets(ir):
    values = []
    for graph in ir.get("graphs", []):
        for node in graph.get("nodes", []):
            if node.get("renderer") not in {"linked_anim_layer", "linked_anim_graph"}:
                continue
            semantic = node.get("semantic")
            target = semantic.get("target_graph") if isinstance(semantic, dict) else None
            if not isinstance(target, str) or not target:
                raise MaterializationError("source linked animation layer graph is unavailable")
            values.append(target)
    return values


def animation_getter_bindings(ir):
    bindings = {}
    for graph in ir.get("graphs", []):
        for node in graph.get("nodes", []):
            if node.get("renderer") != "anim_getter":
                continue
            source_node = node.get("source_node")
            semantic = node.get("semantic")
            if not isinstance(source_node, str) or not isinstance(semantic, dict):
                raise MaterializationError("source animation getter identity is unavailable")
            function = semantic.get("source_function")
            fsm_node = semantic.get("target_fsm_node")
            state_id = semantic.get("target_state_id", "")
            if function not in {"GetInstanceStateWeight", "GetInstanceMachineWeight"}:
                raise MaterializationError("source animation getter function is unsupported")
            if not isinstance(fsm_node, str) or not fsm_node or not isinstance(state_id, str):
                raise MaterializationError("source animation getter target is unavailable")
            if (function == "GetInstanceStateWeight") != bool(state_id):
                raise MaterializationError("source animation getter state target is invalid")
            if source_node in bindings:
                raise MaterializationError("source animation getter is duplicated")
            bindings[source_node] = {
                "target_fsm_node": fsm_node,
                "target_state_id": state_id,
            }
    return bindings


def materialize_files(source, ir, external_graph_paths=(), namespace=""):
    source_asset = source.get("asset_path")
    if source_asset != ir.get("source_asset"):
        raise MaterializationError("source IR asset is invalid")
    namespace = asset_namespace(namespace)
    graph_entries = {
        entry["source_graph"]: entry
        for entry in ir.get("graphs", [])
        if entry.get("kind") == "graph"
    }
    graph_paths = {
        suffix: graph_path(source_asset, suffix, namespace) for suffix in graph_entries
    }
    available_graph_paths = set(graph_paths.values()) | set(external_graph_paths)
    if any(target not in available_graph_paths for target in linked_layer_targets(ir)):
        raise MaterializationError("source linked animation layer graph is not materialized")
    fsm_entries = [
        entry
        for entry in ir.get("graphs", [])
        if entry.get("kind") == "state_machine" and isinstance(entry.get("fsm_renderer"), dict)
    ]
    fsm_configs = {}
    for entry in fsm_entries:
        plan = entry["fsm_renderer"]
        fsm_configs[plan["source_graph"]] = fsm_config(
            source, plan, graph_paths, namespace
        )
    getter_bindings = animation_getter_bindings(ir)
    files = {}
    for entry in fsm_entries:
        plan = entry["fsm_renderer"]
        config = fsm_configs[plan["source_graph"]]
        text, extras = fsm_text(source, plan, config, graph_paths, namespace)
        files[config["path"]] = text
        files.update(extras)
    for suffix, entry in graph_entries.items():
        try:
            text = render_pose_graph(
                contract(source, suffix),
                entry,
                ir["animation_assets"],
                fsm_configs,
            )
        except PoseRenderError as error:
            raise MaterializationError(f"source pose graph is not materializable: {suffix}: {error}") from error
        files[graph_paths[suffix]] = text
    condition_paths = {}
    for entry in ir.get("graphs", []):
        if entry.get("kind") != "transition":
            continue
        suffix = entry["source_graph"]
        path = condition_path(source_asset, suffix, namespace)
        files[path] = (
            transition_renderer.graph_text(contract(source, suffix), getter_bindings)
        )
        condition_paths[suffix] = path
    issues = materialized_reference_issues(files, external_graph_paths)
    if issues:
        raise MaterializationError("; ".join(issues))
    manifest = {
        "format": "ue_animgraph_materialization_manifest_v1",
        "root_graph": graph_paths.get(ir["root_source_graph"]),
        "graphs": graph_paths,
        "fsms": {key: value["path"] for key, value in fsm_configs.items()},
        "conditions": condition_paths,
        "source_transition_conditions": {
            key: value["source_transition_conditions"] for key, value in fsm_configs.items()
        },
        "lifecycle_dispatch": ir.get("lifecycle_dispatch"),
    }
    files["materialization.manifest.json"] = json.dumps(
        manifest, ensure_ascii=False, indent=2
    ) + "\n"
    return files


def conduit_fixture():
    asset = "/Asset"
    fsm = asset + ":AnimGraph.FSM"
    source = fsm + ".Source"
    target = fsm + ".Target"
    conduit = fsm + ".Conduit"
    def graph(path):
        return {
            "object_path": path,
            "class_path": "/Script/AnimGraph.AnimationTransitionGraph",
            "nodes": [
                {
                    "object_path": path + ".Result",
                    "node_guid": stable_id(asset, "fixture", path),
                    "class_path": "/Script/AnimGraph.AnimGraphNode_TransitionResult",
                    "position_x": 0,
                    "position_y": 0,
                    "title": "Result",
                    "pins": [
                        {
                            "name": "bCanEnterTransition",
                            "direction": "Input",
                            "hidden": False,
                            "default_value": "False",
                            "linked_to": [],
                            "pin_type_properties": [
                                {"name": "PinCategory", "exported_text": "bool"}
                            ],
                        }
                    ],
                }
            ],
        }
    def transition(source_suffix, target_suffix, index, condition, automatic=False):
        return {
            "id": stable_id(asset, "fixture-transition", str(index)),
            "source_node": f"{fsm}.Transition{index}",
            "source": source_suffix,
            "target": target_suffix,
            "condition_graph": condition,
            "priority": 0,
            "source_order": index,
            "title": f"Transition {index}",
            "position": [0.0, 0.0],
            "notifies": [],
            "render": {
                "logic": "profiled_blend",
                "duration": 0.1 * (index + 1),
                "blend_mode": "Linear",
                "blend_profile": {"bone_scales": []},
                "automatic_rule": (
                    {
                        "source_semantics": "relevant_asset_time_remaining",
                        "trigger_time": -1.0,
                    }
                    if automatic
                    else None
                ),
            },
        }
    source_condition = "AnimGraph.FSM.Transition0.Rule"
    conduit_condition = "AnimGraph.FSM.Conduit.Rule"
    target_condition = "AnimGraph.FSM.Transition1.Rule"
    return {
        "asset_path": asset,
        "graphs": [
            {
                "object_path": fsm,
                "class_path": "/Script/AnimGraph.AnimationStateMachineGraph",
                "nodes": [
                    {"object_path": source, "index": 0},
                    {"object_path": target, "index": 1},
                    {"object_path": conduit, "index": 2},
                ],
            },
            graph(asset + ":" + source_condition),
            graph(asset + ":" + conduit_condition),
            graph(asset + ":" + target_condition),
        ],
    }, {
        "source_graph": "AnimGraph.FSM",
        "entry_target": "AnimGraph.FSM.Conduit",
        "states": [
            {"source_node": source, "id": "10000000-0000-4000-8000-000000000001", "title": "Source", "position": [0.0, 0.0], "graph": "StateSource", "always_reset_on_entry": False},
            {"source_node": target, "id": "10000000-0000-4000-8000-000000000002", "title": "Target", "position": [1.0, 0.0], "graph": "StateTarget", "always_reset_on_entry": True},
        ],
        "aliases": [],
        "conduits": [{"source_node": conduit, "id": "10000000-0000-4000-8000-000000000003", "title": "Conduit", "position": [0.5, 0.0], "graph": conduit_condition}],
        "conduit_conditions": {conduit: conduit_condition},
        "transitions": [
            transition("AnimGraph.FSM.Source", "AnimGraph.FSM.Conduit", 0, source_condition, True),
            transition("AnimGraph.FSM.Conduit", "AnimGraph.FSM.Target", 1, target_condition),
        ],
        "runtime": {
            "max_transitions_per_frame": 1,
            "max_transition_requests": 2,
            "skip_first_update_transition": False,
            "reinitialize_on_becoming_relevant": True,
        },
    }


def run_self_test():
    assert animation_getter_bindings(
        {
            "graphs": [
                {
                    "nodes": [
                        {
                            "source_node": "/Asset:Transition.Getter",
                            "renderer": "anim_getter",
                            "semantic": {
                                "source_function": "GetInstanceStateWeight",
                                "target_fsm_node": "10000000-0000-4000-8000-000000000001",
                                "target_state_id": "10000000-0000-4000-8000-000000000002",
                            },
                        }
                    ]
                }
            ]
        }
    ) == {
        "/Asset:Transition.Getter": {
            "target_fsm_node": "10000000-0000-4000-8000-000000000001",
            "target_state_id": "10000000-0000-4000-8000-000000000002",
        }
    }
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
    custom = {
        "id": "10000000-0000-4000-8000-000000000003",
        "title": "Custom",
        "render": {
            "logic": "profiled_blend",
            "duration": 0.5,
            "blend_mode": "Custom",
            "blend_profile": {"bone_scales": []},
            "custom_curve": [
                {
                    "interpolation": "Cubic",
                    "tangent_mode": "Auto",
                    "tangent_weight_mode": "None",
                    "t": 0.0,
                    "v": 0.0,
                    "arrive_tangent": 0.0,
                    "arrive_weight": 0.0,
                    "leave_tangent": 0.0,
                    "leave_weight": 0.0,
                }
            ],
        },
    }
    assert "alpha_blend_mode: Custom" in transition_graph_text("/Asset", custom)
    assert graph_path("/Asset", "AnimGraph") == "Asset/AnimGraph.animgraph.ron"
    assert graph_path("/ALS/ALS/Character/AB_Als.AB_Als", "AnimGraph") == (
        "ALS/Character/AB_Als/AnimGraph.animgraph.ron"
    )
    assert fsm_path(
        "/ALS/ALS/Character/AB_Als.AB_Als",
        "AnimGraph.AnimGraphNode_StateMachine_6.Standing States",
        "generated",
    ) == "generated/ALS/Character/AB_Als/AnimGraph/SM6_Standing_States.fsm.ron"
    assert condition_path(
        "/ALS/ALS/Character/AB_Als.AB_Als",
        "AnimGraph.AnimGraphNode_StateMachine_6.Standing States.AnimStateTransitionNode_8.Transition",
        "generated",
    ) == "generated/ALS/Character/AB_Als/AnimGraph/SM6_Standing_States/T8_Transition.condition.animgraph.ron"
    assert transition_path(
        "/ALS/ALS/Character/AB_Als.AB_Als",
        {
            "source_node": "/ALS/ALS/Character/AB_Als.AB_Als:AnimGraph.AnimGraphNode_StateMachine_6.Standing States.AnimStateTransitionNode_8"
        },
        "generated",
    ) == "generated/ALS/Character/AB_Als/AnimGraph/SM6_Standing_States/T8/Transition.transition.animgraph.ron"
    assert transition_path(
        "/ALS/ALS/Character/AB_Als.AB_Als",
        {
            "source_node": "/ALS/ALS/Character/AB_Als.AB_Als:AnimGraph.AnimGraphNode_StateMachine_6.Standing States.AnimStateTransitionNode_8"
        },
        "generated",
    ) != transition_path(
        "/ALS/ALS/Character/AB_Als.AB_Als",
        {
            "source_node": "/ALS/ALS/Character/AB_Als.AB_Als:AnimGraph.AnimGraphNode_StateMachine_6.Standing States.AnimStateTransitionNode_9"
        },
        "generated",
    )
    duplicate_transition = {
        "source_node": "/ALS/ALS/Character/AB_Als.AB_Als:AnimGraph.AnimGraphNode_StateMachine_6.Standing States.AnimStateTransitionNode_8",
        "id": "lowered-a",
    }
    assert transition_path(
        "/ALS/ALS/Character/AB_Als.AB_Als", duplicate_transition, "generated", "lowered-a"
    ) != transition_path(
        "/ALS/ALS/Character/AB_Als.AB_Als", duplicate_transition, "generated", "lowered-b"
    )
    assert len(transition_path(
        "/ALS/ALS/Character/AB_Als.AB_Als",
        {
            "source_node": "/ALS/ALS/Character/AB_Als.AB_Als:AnimGraph.AnimGraphNode_StateMachine_6.Standing States.AnimStateNode_4.Stop.AnimGraphNode_StateMachine_0.Stop States.AnimStateTransitionNode_21"
        },
        "generated",
    )) < 200
    assert graph_path("/Asset", "AnimGraph") != fsm_path("/Asset", "AnimGraph")
    assert graph_path("/Asset", "AnimGraph", "generated/asset") == (
        "generated/asset/" + graph_path("/Asset", "AnimGraph")
    )
    try:
        asset_namespace("generated/../asset")
    except MaterializationError:
        pass
    else:
        raise AssertionError("path traversal namespace was accepted")
    placeholder = placeholder_graph_text("/Asset", "State")
    assert 'node_positions: { "' + stable_id("/Asset", "placeholder", "State") in placeholder
    assert materialized_reference_issues(
        {"graphs/root.animgraph.ron": '(fsm: "fsms/missing.fsm.ron")'}
    )
    assert linked_layer_targets(
        {
            "graphs": [
                {
                    "nodes": [
                        {
                            "renderer": "linked_anim_layer",
                            "semantic": {"target_graph": "graphs/layer.animgraph.ron"},
                        }
                    ]
                }
            ]
        }
    ) == ["graphs/layer.animgraph.ron"]
    try:
        linked_layer_targets({"graphs": [{"nodes": [{"renderer": "linked_anim_layer"}]}]})
    except MaterializationError:
        pass
    else:
        raise AssertionError("linked animation layer without a target was accepted")
    conduit_source, conduit_plan = conduit_fixture()
    conduit_config = fsm_config(
        conduit_source,
        conduit_plan,
        {"StateSource": "states/source.animgraph.ron", "StateTarget": "states/target.animgraph.ron"},
        "",
    )
    assert len(conduit_config["actual"]) == 2
    assert len(conduit_config["transitions"]) == 1
    assert conduit_config["transitions"][0]["source"] == "AnimGraph.FSM.Source"
    assert conduit_config["transitions"][0]["target"] == "AnimGraph.FSM.Target"
    assert len(conduit_config["source_transition_conditions"][0]["gates"]) == 2
    assert conduit_config["source_transition_conditions"][0]["always_reset_target_on_entry"] is True
    assert conduit_config["entry_state_id"] == "10000000-0000-4000-8000-000000000002"
    assert len(conduit_config["source_entry_conditions"]) == 1
    assert len(conduit_config["source_entry_conditions"][0]["gates"]) == 2
    context_ids = [
        conduit_config["source_transition_conditions"][0]["context_state_id"],
        *(
            gate["context_state_id"]
            for gate in conduit_config["source_transition_conditions"][0]["gates"]
        ),
        *(
            gate["context_state_id"]
            for gate in conduit_config["source_entry_conditions"][0]["gates"]
        ),
    ]
    assert len(context_ids) == len(set(context_ids))
    assert all(uuid.UUID(value).version == 5 for value in context_ids)
    assert conduit_config["source_transition_conditions"][0]["automatic_trigger_time"] == -1.0
    assert conduit_config["source_transition_conditions"][0]["transition_duration"] == 0.0
    assert not conduit_config["skip_first_update_transition"]
    assert conduit_config["reinitialize_on_becoming_relevant"]
    conduit_text, conduit_files = fsm_text(
        conduit_source,
        conduit_plan,
        conduit_config,
        {"StateSource": "states/source.animgraph.ron", "StateTarget": "states/target.animgraph.ron"},
        "",
    )
    assert "Source conduit or alias" not in conduit_text
    assert len(conduit_files) == 1
    return {
        "standard_and_inertial_transitions_materialized": "PASS",
        "source_custom_curve_materialized": "PASS",
        "source_paths_are_deterministic": "PASS",
        "synthetic_state_node_position_materialized": "PASS",
        "missing_generated_reference_rejected": "PASS",
        "linked_layer_target_closure_checked": "PASS",
        "animation_state_getter_target_binding_materialized": "PASS",
        "conduit_guard_flattens_to_a_content_transition": "PASS",
        "condition_graph_invocations_have_stable_isolated_contexts": "PASS",
        "source_asset_and_graph_ancestry_paths_are_readable": "PASS",
    }


if __name__ == "__main__":
    print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
