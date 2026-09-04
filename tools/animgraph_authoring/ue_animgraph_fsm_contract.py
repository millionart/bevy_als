import json
import re


FSM_CLASS = "/Script/AnimGraph.AnimationStateMachineGraph"
ENTRY_CLASS = "/Script/AnimGraph.AnimStateEntryNode"
STATE_CLASS = "/Script/AnimGraph.AnimStateNode"
ALIAS_CLASS = "/Script/AnimGraph.AnimStateAliasNode"
CONDUIT_CLASS = "/Script/AnimGraph.AnimStateConduitNode"
TRANSITION_CLASS = "/Script/AnimGraph.AnimStateTransitionNode"


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
        props = properties(node)
        if class_path == STATE_CLASS:
            item = node_base(source, node)
            item["graph"] = bound_graph(source, props)
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
    }


def fixture_source():
    asset = "/Asset"
    fsm = asset + ":AnimGraph.FSM"
    entry = fsm + ".Entry"
    state = fsm + ".State"
    transition = fsm + ".Transition"
    state_graph = state + ".Idle"
    transition_graph = transition + ".Rule"
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
                "object_path": fsm,
                "class_path": FSM_CLASS,
                "nodes": [
                    node(entry, ENTRY_CLASS, "10000000-0000-4000-8000-000000000001", "Entry", {}, [output("Entry", state, "In")]),
                    node(state, STATE_CLASS, "10000000-0000-4000-8000-000000000002", "Idle", {"BoundGraph": "'/Asset:AnimGraph.FSM.State.Idle'"}, [output("Out", transition, "In")]),
                    node(transition, TRANSITION_CLASS, "10000000-0000-4000-8000-000000000003", "Idle to Idle", {"BoundGraph": "'/Asset:AnimGraph.FSM.Transition.Rule'", "PriorityOrder": "1", "CrossfadeDuration": "0.2", "BlendMode": "HermiteCubic", "BlendProfileWrapper": "(bIsSkeletonBlendProfile=True)", "LogicType": "TLT_StandardBlend", "bAutomaticRuleBasedOnSequencePlayerInState": "False", "AutomaticRuleTriggerTime": "-1.0", "MinTimeBeforeReentry": "-1.0", "SyncGroupNameToRequireValidMarkersRule": "None", "bAllowInertializationForSelfTransitions": "False", "bOnlyEvaluateWhenActive": "False"}, [output("Out", state, "In")]),
                ],
            },
            {"object_path": state_graph, "class_path": "/Script/AnimGraph.AnimationStateGraph", "nodes": []},
            {"object_path": transition_graph, "class_path": "/Script/AnimGraph.AnimationTransitionGraph", "nodes": []},
        ],
    }


def run_self_test():
    value = fsm_contract(fixture_source(), "AnimGraph.FSM")
    assert value["entry_target"] == "AnimGraph.FSM.State"
    assert value["states"][0]["graph"] == "AnimGraph.FSM.State.Idle"
    assert value["transitions"][0]["condition_graph"] == "AnimGraph.FSM.Transition.Rule"
    assert value["transitions"][0]["blend_mode"] == "HermiteCubic"
    assert value["transitions"][0]["source_order"] == 0
    return {
        "fsm_entry_state_transition_topology_preserved": "PASS",
        "fsm_child_graph_references_preserved": "PASS",
        "fsm_transition_contract_preserved": "PASS",
    }


if __name__ == "__main__":
    print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
