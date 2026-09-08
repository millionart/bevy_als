import json
import re


FSM_CLASS = "/Script/AnimGraph.AnimationStateMachineGraph"
ENTRY_CLASS = "/Script/AnimGraph.AnimStateEntryNode"
STATE_CLASS = "/Script/AnimGraph.AnimStateNode"
ALIAS_CLASS = "/Script/AnimGraph.AnimStateAliasNode"
CONDUIT_CLASS = "/Script/AnimGraph.AnimStateConduitNode"
TRANSITION_CLASS = "/Script/AnimGraph.AnimStateTransitionNode"
STATE_MACHINE_NODE_CLASS = "/Script/AnimGraph.AnimGraphNode_StateMachine"
TRANSITION_NOTIFY_FIELDS = ("TransitionStart", "TransitionEnd", "TransitionInterrupt")


class FsmContractError(ValueError):
    pass


def source_suffix(source, object_path):
    asset = source.get("asset_path")
    prefix = f"{asset}:"
    if not isinstance(object_path, str) or not isinstance(asset, str) or not object_path.startswith(prefix):
        raise FsmContractError("source object path is invalid")
    return object_path[len(prefix) :]


def graph_for_suffix(source, suffix):
    matches = [
        graph
        for graph in source.get("graphs", [])
        if graph.get("object_path") == f"{source['asset_path']}:{suffix}"
    ]
    if len(matches) != 1:
        raise FsmContractError("source FSM graph is missing or ambiguous")
    return matches[0]


def properties(node):
    result = {}
    for value in node.get("properties", []):
        name = value.get("name")
        exported = value.get("exported_text")
        if isinstance(name, str) and isinstance(exported, str):
            if name in result:
                raise FsmContractError("source FSM property is duplicated")
            result[name] = exported
    return result


def source_node_properties(source, path):
    values = []
    for graph in source.get("graphs", []):
        values.extend(node for node in graph.get("nodes", []) if node.get("object_path") == path)
    values.extend(node for node in source.get("asset_objects", []) if node.get("object_path") == path)
    if not values:
        raise FsmContractError("source FSM node is unavailable")
    result = {}
    for value in values:
        for name, exported in properties(value).items():
            existing = result.get(name)
            if existing is not None and existing != exported:
                raise FsmContractError("source FSM node property is inconsistent")
            result[name] = exported
    return result


def required(props, name):
    value = props.get(name)
    if not isinstance(value, str):
        raise FsmContractError(f"source FSM property is missing: {name}")
    return value


def integer(value):
    try:
        return int(value)
    except (TypeError, ValueError) as error:
        raise FsmContractError("source FSM integer is invalid") from error


def number(value):
    try:
        return float(value)
    except (TypeError, ValueError) as error:
        raise FsmContractError("source FSM number is invalid") from error


def boolean(value):
    if value == "True":
        return True
    if value == "False":
        return False
    raise FsmContractError("source FSM boolean is invalid")


def struct_value(value, name, default):
    match = re.search(rf"(?:^|[,(]){re.escape(name)}=([^,()]+)", value)
    return match.group(1) if match is not None else default


def transition_notifies(props):
    values = []
    for field in TRANSITION_NOTIFY_FIELDS:
        match = re.search(r'NotifyName="([^"]+)"', props.get(field, ""))
        if match is not None:
            values.append({"phase": field, "name": match.group(1)})
    return values


def state_machine_runtime(source, suffix):
    asset = source.get("asset_path")
    if not isinstance(asset, str) or not asset or "." not in suffix:
        raise FsmContractError("source FSM runtime node is invalid")
    source_node = f"{asset}:{suffix.rsplit('.', 1)[0]}"
    props = source_node_properties(source, source_node)
    node = required(props, "Node")
    return {
        "source_node": source_node,
        "max_transitions_per_frame": integer(
            struct_value(node, "MaxTransitionsPerFrame", "3")
        ),
        "max_transition_requests": integer(
            struct_value(node, "MaxTransitionsRequests", "32")
        ),
        "skip_first_update_transition": boolean(
            struct_value(node, "bSkipFirstUpdateTransition", "True")
        ),
        "reinitialize_on_becoming_relevant": boolean(
            struct_value(node, "bReinitializeOnBecomingRelevant", "True")
        ),
        "allow_conduit_entry_states": boolean(
            struct_value(node, "bAllowConduitEntryStates", "False")
        ),
    }


def object_references(value):
    return re.findall(r"'(/[^']+)'", value) if isinstance(value, str) else []


def graph_links(graph):
    links = []
    for node in graph.get("nodes", []):
        source = node.get("object_path")
        for pin in node.get("pins", []):
            if pin.get("direction") != "Output":
                continue
            for target in pin.get("linked_to", []):
                links.append(
                    {
                        "source": source,
                        "source_pin": pin.get("name"),
                        "target": target.get("node_object_path"),
                        "target_pin": target.get("pin_name"),
                    }
                )
    return links


def outgoing_transition_order(links, nodes):
    result = {}
    for link in links:
        source = link["source"]
        target = link["target"]
        target_node = nodes.get(target)
        if (
            source not in nodes
            or target_node is None
            or target_node.get("class_path") != TRANSITION_CLASS
            or link["target_pin"] != "In"
        ):
            continue
        result.setdefault(source, []).append(target)
    for values in result.values():
        if len(values) != len(set(values)):
            raise FsmContractError("source FSM transition order is duplicated")
    return result


def bound_graph(source, props):
    references = object_references(required(props, "BoundGraph"))
    if len(references) != 1:
        raise FsmContractError("source FSM bound graph reference is invalid")
    return source_suffix(source, references[0])


def node_base(source, node):
    path = node.get("object_path")
    guid = node.get("node_guid")
    title = node.get("title")
    if not isinstance(path, str) or not isinstance(guid, str) or not isinstance(title, str):
        raise FsmContractError("source FSM node identity is invalid")
    return {
        "source_node": path,
        "id": guid.lower(),
        "title": title,
        "position": [node.get("position_x"), node.get("position_y")],
    }


def endpoint(links, node_path, pin, direction):
    if direction == "source":
        values = [link["target"] for link in links if link["source"] == node_path and link["source_pin"] == pin]
    else:
        values = [link["source"] for link in links if link["target"] == node_path and link["target_pin"] == pin]
    if len(values) != 1:
        raise FsmContractError("source FSM transition endpoint is missing or ambiguous")
    return values[0]


def fsm_contract(source, suffix):
    graph = graph_for_suffix(source, suffix)
    if graph.get("class_path") != FSM_CLASS:
        raise FsmContractError("source graph is not an FSM")
    nodes = {node.get("object_path"): node for node in graph.get("nodes", [])}
    if len(nodes) != len(graph.get("nodes", [])) or None in nodes:
        raise FsmContractError("source FSM nodes are invalid")
    links = graph_links(graph)
    transition_order = outgoing_transition_order(links, nodes)
    entries = [node for node in nodes.values() if node.get("class_path") == ENTRY_CLASS]
    if len(entries) != 1:
        raise FsmContractError("source FSM entry is missing or ambiguous")
    entry_target = endpoint(links, entries[0]["object_path"], "Entry", "source")
    states = []
    aliases = []
    conduits = []
    transitions = []
    for node in nodes.values():
        class_path = node.get("class_path")
        props = source_node_properties(source, node["object_path"])
        if class_path == STATE_CLASS:
            item = node_base(source, node)
            item["graph"] = bound_graph(source, props)
            item["always_reset_on_entry"] = boolean(props.get("bAlwaysResetOnEntry", "False"))
            states.append(item)
        elif class_path == ALIAS_CLASS:
            item = node_base(source, node)
            members = object_references(required(props, "AliasedStateNodes"))
            if not members:
                raise FsmContractError("source FSM alias members are missing")
            item["members"] = [source_suffix(source, member) for member in members]
            aliases.append(item)
        elif class_path == CONDUIT_CLASS:
            item = node_base(source, node)
            item["graph"] = bound_graph(source, props)
            conduits.append(item)
        elif class_path == TRANSITION_CLASS:
            item = node_base(source, node)
            source_path = endpoint(links, node["object_path"], "In", "target")
            order = transition_order.get(source_path, [])
            if node["object_path"] not in order:
                raise FsmContractError("source FSM transition order is missing")
            item.update(
                {
                    "source": source_suffix(source, source_path),
                    "target": source_suffix(source, endpoint(links, node["object_path"], "Out", "source")),
                    "condition_graph": bound_graph(source, props),
                    "source_order": order.index(node["object_path"]),
                    "priority": integer(required(props, "PriorityOrder")),
                    "crossfade_duration": number(required(props, "CrossfadeDuration")),
                    "blend_mode": required(props, "BlendMode"),
                    "blend_profile_wrapper": required(props, "BlendProfileWrapper"),
                    "logic_type": required(props, "LogicType"),
                    "automatic": boolean(required(props, "bAutomaticRuleBasedOnSequencePlayerInState")),
                    "automatic_trigger_time": number(required(props, "AutomaticRuleTriggerTime")),
                    "min_time_before_reentry": number(required(props, "MinTimeBeforeReentry")),
                    "sync_group_requirement": required(props, "SyncGroupNameToRequireValidMarkersRule"),
                    "allow_inertialization_self_transition": boolean(required(props, "bAllowInertializationForSelfTransitions")),
                    "only_evaluate_when_active": boolean(required(props, "bOnlyEvaluateWhenActive")),
                    "notifies": transition_notifies(props),
                    "source_properties": props,
                }
            )
            transitions.append(item)
    structural = {state["source_node"] for state in states}
    structural.update(alias["source_node"] for alias in aliases)
    structural.update(conduit["source_node"] for conduit in conduits)
    if entry_target not in structural:
        raise FsmContractError("source FSM entry target is not structural")
    for alias in aliases:
        if any(f"{source['asset_path']}:{member}" not in nodes for member in alias["members"]):
            raise FsmContractError("source FSM alias member is unavailable")
    return {
        "format": "ue_animgraph_fsm_contract_v1",
        "source_graph": suffix,
        "entry_target": source_suffix(source, entry_target),
        "states": sorted(states, key=lambda item: item["id"]),
        "aliases": sorted(aliases, key=lambda item: item["id"]),
        "conduits": sorted(conduits, key=lambda item: item["id"]),
        "transitions": sorted(transitions, key=lambda item: item["id"]),
        "runtime": state_machine_runtime(source, suffix),
    }


def fixture_source():
    asset = "/Asset"
    root = asset + ":AnimGraph"
    machine = root + ".Machine"
    fsm = machine + ".FSM"
    entry = fsm + ".Entry"
    state = fsm + ".State"
    transition = fsm + ".Transition"
    state_graph = state + ".Idle"
    transition_graph = transition + ".Rule"
    bool_type = [{"name": "PinCategory", "exported_text": "bool"}]
    def node(path, class_path, guid, title, props, pins):
        return {
            "object_path": path,
            "class_path": class_path,
            "node_guid": guid,
            "title": title,
            "position_x": 0,
            "position_y": 0,
            "properties": [{"name": key, "exported_text": value} for key, value in props.items()],
            "pins": pins,
        }
    def output(name, target, target_pin):
        return {"name": name, "direction": "Output", "linked_to": [{"node_object_path": target, "pin_name": target_pin}]}
    return {
        "asset_path": asset,
        "graphs": [
            {
                "object_path": root,
                "class_path": "/Script/AnimGraph.AnimationGraph",
                "nodes": [
                    node(
                        machine,
                        STATE_MACHINE_NODE_CLASS,
                        "10000000-0000-4000-8000-000000000000",
                        "Machine",
                        {
                            "Node": "(MaxTransitionsPerFrame=1,MaxTransitionsRequests=2,bSkipFirstUpdateTransition=False,bReinitializeOnBecomingRelevant=True,bAllowConduitEntryStates=False)"
                        },
                        [],
                    )
                ],
            },
            {
                "object_path": fsm,
                "class_path": FSM_CLASS,
                "nodes": [
                    node(entry, ENTRY_CLASS, "10000000-0000-4000-8000-000000000001", "Entry", {}, [output("Entry", state, "In")]),
                    node(state, STATE_CLASS, "10000000-0000-4000-8000-000000000002", "Idle", {"BoundGraph": f"'{state_graph}'"}, [output("Out", transition, "In")]),
                    node(transition, TRANSITION_CLASS, "10000000-0000-4000-8000-000000000003", "Idle to Idle", {"BoundGraph": f"'{transition_graph}'", "PriorityOrder": "1", "CrossfadeDuration": "0.2", "BlendMode": "HermiteCubic", "BlendProfileWrapper": "(bIsSkeletonBlendProfile=True)", "LogicType": "TLT_StandardBlend", "bAutomaticRuleBasedOnSequencePlayerInState": "False", "AutomaticRuleTriggerTime": "-1.0", "MinTimeBeforeReentry": "-1.0", "SyncGroupNameToRequireValidMarkersRule": "None", "bAllowInertializationForSelfTransitions": "False", "bOnlyEvaluateWhenActive": "False", "TransitionStart": '(NotifyName="Notify")'}, [output("Out", state, "In")]),
                ],
            },
            {"object_path": state_graph, "class_path": "/Script/AnimGraph.AnimationStateGraph", "nodes": []},
            {
                "object_path": transition_graph,
                "class_path": "/Script/AnimGraph.AnimationTransitionGraph",
                "nodes": [
                    node(
                        transition_graph + ".Result",
                        "/Script/AnimGraph.AnimGraphNode_TransitionResult",
                        "10000000-0000-4000-8000-000000000004",
                        "Result",
                        {},
                        [
                            {
                                "name": "bCanEnterTransition",
                                "direction": "Input",
                                "hidden": False,
                                "default_value": "False",
                                "linked_to": [],
                                "pin_type_properties": bool_type,
                            }
                        ],
                    )
                ],
            },
        ],
    }


def run_self_test():
    value = fsm_contract(fixture_source(), "AnimGraph.Machine.FSM")
    assert value["states"][0]["always_reset_on_entry"] is False
    reset_source = fixture_source()
    reset_state = next(node for graph in reset_source["graphs"] for node in graph["nodes"]
                       if node["class_path"] == STATE_CLASS)
    reset_state["properties"].append({"name": "bAlwaysResetOnEntry", "exported_text": "True"})
    assert fsm_contract(reset_source, "AnimGraph.Machine.FSM")["states"][0]["always_reset_on_entry"] is True
    assert value["entry_target"] == "AnimGraph.Machine.FSM.State"
    assert value["states"][0]["graph"] == "AnimGraph.Machine.FSM.State.Idle"
    assert value["transitions"][0]["condition_graph"] == "AnimGraph.Machine.FSM.Transition.Rule"
    assert value["transitions"][0]["blend_mode"] == "HermiteCubic"
    assert value["transitions"][0]["source_order"] == 0
    assert value["transitions"][0]["notifies"] == [{"phase": "TransitionStart", "name": "Notify"}]
    assert value["runtime"] == {
        "source_node": "/Asset:AnimGraph.Machine",
        "max_transitions_per_frame": 1,
        "max_transition_requests": 2,
        "skip_first_update_transition": False,
        "reinitialize_on_becoming_relevant": True,
        "allow_conduit_entry_states": False,
    }
    return {
        "fsm_entry_state_transition_topology_preserved": "PASS",
        "fsm_child_graph_references_preserved": "PASS",
        "fsm_transition_contract_preserved": "PASS",
        "fsm_runtime_settings_and_transition_notifies_preserved": "PASS",
    }


if __name__ == "__main__":
    print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
