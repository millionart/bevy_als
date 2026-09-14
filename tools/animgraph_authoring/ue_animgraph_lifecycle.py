import json

from ue_exported_text import ExportedTextError, parse_bool, parse_number


FUNCTION_ENTRY_CLASS = "/Script/BlueprintGraph.K2Node_FunctionEntry"
FUNCTION_CALL_CLASS = "/Script/BlueprintGraph.K2Node_CallFunction"
STOP_FUNCTION = "StopTransitionAndTurnInPlaceAnimations"
EXTERNAL_PROVIDERS = {
    "InitializeLean": "als_locomotion_initialize_lean",
    "InitializeHead": "als_ext::SourcePropertyWriteNode",
    "RefreshHead": "als_ext::AlsSourceRefreshNode",
    "InitializeGrounded": "als_ext::SourcePropertyWriteNode",
    "RefreshGrounded": "als_ext::AlsSourceRefreshNode",
    "ResetGroundedEntryMode": "als_ext::SourcePropertyWriteNode + crate::generated_transition_notify::GeneratedTransitionNotifyPlugin",
    "RefreshGroundedMovement": "crate::generated_ab_als_provider::GeneratedAbAlsProviderPlugin",
    "InitializeStandingMovement": "als_ext::SourcePropertyWriteNode",
    "RefreshStandingMovement": "als_ext::AlsSourceRefreshNode",
    "ResetPivot": "als_ext::SourcePropertyWriteNode",
    "RefreshCrouchingMovement": "crate::generated_ab_als_provider::GeneratedAbAlsProviderPlugin",
    "RefreshDynamicTransitions": "als_ext::AlsSourceRefreshNode",
    "RefreshInAir": "als_locomotion_refresh_in_air",
    "RefreshRotateInPlace": "als_ext::AlsSourceRefreshNode",
    "InitializeTurnInPlace": "als_ext::SourcePropertyWriteNode",
    "RefreshTurnInPlace": "als_ext::AlsSourceRefreshNode",
    "SetHipsDirection": "als_ext::SourceSetHipsDirectionNode",
}
TRANSITION_FUNCTIONS = {
    "PlayStopLeftTransitionAnimation": {
        "call": "Play Transition Animation",
        "sequence": "A_Als_Stop_Left.A_Als_Stop_Left",
        "start_time": "0.400000",
        "provider": "crate::generated_transition_notify::GeneratedTransitionNotifyPlugin",
    },
    "PlayStopRightTransitionAnimation": {
        "call": "Play Transition Animation",
        "sequence": "A_Als_Stop_Right.A_Als_Stop_Right",
        "start_time": "0.400000",
        "provider": "crate::generated_transition_notify::GeneratedTransitionNotifyPlugin",
    },
    "PlayStopTransitionAnimation": {
        "call": "Play Transition Right Animation",
        "sequence": None,
        "start_time": "0.200000",
        "provider": "crate::generated_transition_notify::GeneratedTransitionNotifyPlugin",
    },
    "PlayRollToGroundedTransitionAnimation": {
        "call": "Play Transition Right Animation",
        "sequence": None,
        "start_time": "0.200000",
        "provider": "crate::generated_transition_notify::GeneratedTransitionNotifyPlugin",
    },
}
TRANSITION_NOTIFY_FUNCTIONS = {
    "AnimNotify_ActivatePivot": {
        "call": "Activate Pivot",
        "member": "ActivatePivot",
    },
    "AnimNotify_StopQuick": {
        "call": "Play Quick Stop Animation",
        "member": "PlayQuickStopAnimation",
    },
    "AnimNotify_LandToGrounded": {
        "call": "Play Transition Right Animation",
        "member": "PlayTransitionRightAnimation",
    },
}
TRANSITION_NOTIFY_PROVIDER = "crate::generated_transition_notify::GeneratedTransitionNotifyPlugin"


class LifecycleRenderError(ValueError):
    pass


def graph_for_suffix(source, suffix):
    graph_path = f"{source['asset_path']}:{suffix}"
    matches = [graph for graph in source.get("graphs", []) if graph.get("object_path") == graph_path]
    if len(matches) != 1:
        raise LifecycleRenderError("source lifecycle graph is missing or ambiguous")
    return matches[0]


def signature(node):
    title = node.get("title")
    return title.splitlines()[0].strip() if isinstance(title, str) else ""


def linked(node, pin, target, target_pin):
    return any(
        item.get("node_object_path") == target and item.get("pin_name") == target_pin
        for candidate in node.get("pins", [])
        if candidate.get("name") == pin and candidate.get("direction") == "Output"
        for item in candidate.get("linked_to", [])
    )


def input_default(node, pin):
    values = [
        item.get("default_value")
        for item in node.get("pins", [])
        if item.get("name") == pin and item.get("direction") == "Input"
    ]
    if len(values) != 1 or not isinstance(values[0], str):
        raise LifecycleRenderError("source lifecycle input is missing or ambiguous")
    return values[0]


def input_object(node, pin):
    values = [
        item.get("default_object")
        for item in node.get("pins", [])
        if item.get("name") == pin and item.get("direction") == "Input"
    ]
    if len(values) != 1 or not isinstance(values[0], str):
        raise LifecycleRenderError("source lifecycle object input is missing or ambiguous")
    return values[0]


def has_member(node, member):
    return any(
        f'MemberName="{member}"' in str(property.get("exported_text", ""))
        for property in node.get("properties", [])
    )


def validate_stop_function(source, suffix):
    graph = graph_for_suffix(source, suffix)
    nodes = graph.get("nodes", [])
    entry = [node for node in nodes if node.get("class_path") == FUNCTION_ENTRY_CLASS]
    calls = [node for node in nodes if node.get("class_path") == FUNCTION_CALL_CLASS]
    if len(nodes) != 3 or len(entry) != 1 or len(calls) != 2:
        raise LifecycleRenderError("source stop lifecycle graph shape is unsupported")
    stop = [node for node in calls if signature(node) == "Stop Transition and Turn in Place Animations"]
    parent = [node for node in calls if signature(node) == "Get Parent"]
    if len(stop) != 1 or len(parent) != 1:
        raise LifecycleRenderError("source stop lifecycle call chain is unsupported")
    entry = entry[0]
    stop = stop[0]
    parent = parent[0]
    if not linked(entry, "then", stop["object_path"], "execute"):
        raise LifecycleRenderError("source stop lifecycle execution link is unsupported")
    if not linked(parent, "ReturnValue", stop["object_path"], "self"):
        raise LifecycleRenderError("source stop lifecycle parent link is unsupported")
    if input_default(stop, "BlendOutDuration") != "-1.000000":
        raise LifecycleRenderError("source stop lifecycle blend-out default is unsupported")
    return [entry["object_path"], parent["object_path"], stop["object_path"]]


def validate_transition_function(source, suffix, source_function):
    specification = TRANSITION_FUNCTIONS[source_function]
    graph = graph_for_suffix(source, suffix)
    nodes = graph.get("nodes", [])
    entry = [node for node in nodes if node.get("class_path") == FUNCTION_ENTRY_CLASS]
    calls = [node for node in nodes if node.get("class_path") == FUNCTION_CALL_CLASS]
    if len(nodes) != 3 or len(entry) != 1 or len(calls) != 2:
        raise LifecycleRenderError("source transition lifecycle graph shape is unsupported")
    transition = [node for node in calls if signature(node) == specification["call"]]
    parent = [node for node in calls if signature(node) == "Get Parent"]
    if len(transition) != 1 or len(parent) != 1 or signature(entry[0]) != source_function:
        raise LifecycleRenderError("source transition lifecycle call chain is unsupported")
    entry = entry[0]
    transition = transition[0]
    parent = parent[0]
    expected_inputs = {
        "execute",
        "self",
        "BlendInDuration",
        "BlendOutDuration",
        "PlayRate",
        "StartTime",
        "bFromStandingIdleOnly",
    }
    if specification["sequence"] is not None:
        expected_inputs.add("Sequence")
    if {
        pin.get("name")
        for pin in transition.get("pins", [])
        if pin.get("direction") == "Input"
    } != expected_inputs:
        raise LifecycleRenderError("source transition lifecycle inputs are unsupported")
    if not linked(entry, "then", transition["object_path"], "execute"):
        raise LifecycleRenderError("source transition lifecycle execution link is unsupported")
    if not linked(parent, "ReturnValue", transition["object_path"], "self"):
        raise LifecycleRenderError("source transition lifecycle parent link is unsupported")
    for pin, value in {
        "BlendInDuration": "0.200000",
        "BlendOutDuration": "0.200000",
        "PlayRate": "1.500000",
        "StartTime": specification["start_time"],
        "bFromStandingIdleOnly": "false",
    }.items():
        if input_default(transition, pin) != value:
            raise LifecycleRenderError("source transition lifecycle default is unsupported")
    sequence = specification["sequence"]
    if sequence is not None and not input_object(transition, "Sequence").endswith(sequence):
        raise LifecycleRenderError("source transition lifecycle sequence is unsupported")
    return [entry["object_path"], parent["object_path"], transition["object_path"]]


def validate_transition_notify_function(source, suffix, source_function):
    specification = TRANSITION_NOTIFY_FUNCTIONS.get(source_function)
    graph = graph_for_suffix(source, suffix)
    function_nodes = graph.get("nodes", [])
    entry = [
        node
        for node in function_nodes
        if node.get("class_path") == FUNCTION_ENTRY_CLASS
    ]
    bridge = [
        node
        for node in function_nodes
        if node.get("class_path") == FUNCTION_CALL_CLASS
        and signature(node).startswith("Execute Ubergraph ")
    ]
    if (
        len(function_nodes) != 2
        or len(entry) != 1
        or len(bridge) != 1
        or signature(entry[0]) != source_function
        or not linked(entry[0], "then", bridge[0]["object_path"], "execute")
    ):
        raise LifecycleRenderError("source transition notify entry is unsupported")
    event_graph = graph_for_suffix(source, "EventGraph")
    nodes = event_graph.get("nodes", [])
    event = [
        node
        for node in nodes
        if node.get("class_path") == "/Script/BlueprintGraph.K2Node_Event"
        and signature(node) == source_function
    ]
    if len(event) != 1:
        raise LifecycleRenderError("source transition notify handler is unsupported")
    event = event[0]
    target = [
        node
        for node in nodes
        if node.get("class_path") == FUNCTION_CALL_CLASS
        and linked(event, "then", node["object_path"], "execute")
    ]
    if len(target) != 1:
        raise LifecycleRenderError("source transition notify target is unsupported")
    target = target[0]
    if sum(len(pin.get("linked_to", [])) for pin in event.get("pins", []) if pin.get("direction") == "Output" and pin.get("name") == "then") != 1:
        raise LifecycleRenderError("source transition notify execution fanout is unsupported")
    parent = [
        node
        for node in nodes
        if node.get("class_path") == FUNCTION_CALL_CLASS
        and signature(node) == "Get Parent"
        and linked(node, "ReturnValue", target["object_path"], "self")
    ]
    if len(parent) != 1:
        raise LifecycleRenderError("source transition notify parent link is unsupported")
    if specification is not None and (signature(target) != specification["call"] or not has_member(target, specification["member"])):
        raise LifecycleRenderError("source transition notify target is unsupported")
    handled = [
        entry[0]["object_path"],
        bridge[0]["object_path"],
        event["object_path"],
        parent[0]["object_path"],
        target["object_path"],
    ]
    return handled, None if specification is not None else transition_animation_call(target)


def transition_animation_call(target):
    functions = [name for name in ("PlayTransitionLeftAnimation", "PlayTransitionRightAnimation") if has_member(target, name)]
    if len(functions) != 1 or not any('MemberParent="/Script/CoreUObject.Class\'/Script/ALS.AlsAnimationInstance\'"' in value.get("exported_text", "") for value in target.get("properties", [])):
        raise LifecycleRenderError("source transition notify native signature is unsupported")
    names = ("BlendInDuration", "BlendOutDuration", "PlayRate", "StartTime", "bFromStandingIdleOnly")
    inputs = [pin for pin in target.get("pins", []) if pin.get("direction") == "Input"]
    if {pin.get("name") for pin in inputs} != {"execute", "self", *names} or len(inputs) != len(names) + 2:
        raise LifecycleRenderError("source transition notify arguments are unsupported")
    if any(pin.get("linked_to") or pin.get("default_value_ignored") for pin in inputs if pin.get("name") in names):
        raise LifecycleRenderError("source transition notify arguments must be constant")
    if any(pin.get("linked_to") for pin in target.get("pins", []) if pin.get("direction") == "Output" and pin.get("name") == "then"):
        raise LifecycleRenderError("source transition notify continuation is unsupported")
    try:
        arguments = [{"property":name, "type":"Bool" if name == "bFromStandingIdleOnly" else "F32", "value":parse_bool(input_default(target, name).capitalize()) if name == "bFromStandingIdleOnly" else parse_number(input_default(target, name))} for name in names]
    except ExportedTextError as error:
        raise LifecycleRenderError("source transition notify argument value is invalid") from error
    return {"function":functions[0], "arguments":arguments}


def resolve_lifecycle(source, function_dependencies):
    dispatches = []
    handled_nodes = set()
    for dependency in function_dependencies:
        source_function = dependency.get("source_function")
        source_node = dependency.get("source_node")
        resolution = dependency.get("resolution")
        if not isinstance(source_function, str) or not isinstance(source_node, str):
            raise LifecycleRenderError("source lifecycle dependency is invalid")
        provider = EXTERNAL_PROVIDERS.get(source_function)
        if resolution == "external" and provider is not None:
            dispatches.append(
                {
                    "source_function": source_function,
                    "call_site": dependency.get("call_site"),
                    "source_node": source_node,
                    "provider": provider,
                }
            )
            continue
        if resolution == "graph" and source_function == STOP_FUNCTION:
            graph = dependency.get("source_graph")
            if not isinstance(graph, str):
                raise LifecycleRenderError("source stop lifecycle graph is missing")
            handled_nodes.add(source_node)
            handled_nodes.update(validate_stop_function(source, graph))
            dispatches.append(
                {
                    "source_function": source_function,
                    "call_site": dependency.get("call_site"),
                    "source_node": source_node,
                    "provider": TRANSITION_NOTIFY_PROVIDER,
                }
            )
            continue
        if resolution == "graph" and source_function in TRANSITION_FUNCTIONS:
            graph = dependency.get("source_graph")
            if not isinstance(graph, str):
                raise LifecycleRenderError("source transition lifecycle graph is missing")
            handled_nodes.add(source_node)
            handled_nodes.update(validate_transition_function(source, graph, source_function))
            dispatches.append(
                {
                    "source_function": source_function,
                    "call_site": dependency.get("call_site"),
                    "source_node": source_node,
                    "provider": TRANSITION_FUNCTIONS[source_function]["provider"],
                }
            )
            continue
        if resolution == "graph" and source_function.startswith("AnimNotify_"):
            graph = dependency.get("source_graph")
            if not isinstance(graph, str):
                raise LifecycleRenderError("source transition notify graph is missing")
            handled_nodes.add(source_node)
            nodes, native_call = validate_transition_notify_function(source, graph, source_function)
            handled_nodes.update(nodes)
            dispatches.append(
                {
                    "source_function": source_function,
                    "call_site": dependency.get("call_site"),
                    "source_node": source_node,
                    "provider": TRANSITION_NOTIFY_PROVIDER,
                    **({"native_call": native_call} if native_call is not None else {}),
                }
            )
    return {
        "format": "ue_animgraph_lifecycle_dispatch_v1",
        "dispatches": sorted(dispatches, key=lambda item: item["source_node"]),
        "handled_nodes": sorted(handled_nodes),
    }


def fixture_source():
    asset = "/Asset"
    graph = asset + ":StopTransitionAndTurnInPlaceAnimations"
    entry = graph + ".Entry"
    stop = graph + ".Stop"
    parent = graph + ".Parent"
    def node(path, class_path, title, pins):
        return {"object_path": path, "class_path": class_path, "title": title, "pins": pins}
    def output(name, target, target_pin):
        return {"name": name, "direction": "Output", "linked_to": [{"node_object_path": target, "pin_name": target_pin}]}
    return {
        "asset_path": asset,
        "graphs": [
            {
                "object_path": graph,
                "nodes": [
                    node(entry, FUNCTION_ENTRY_CLASS, STOP_FUNCTION, [output("then", stop, "execute")]),
                    node(stop, FUNCTION_CALL_CLASS, "Stop Transition and Turn in Place Animations", [{"name": "BlendOutDuration", "direction": "Input", "default_value": "-1.000000"}]),
                    node(parent, FUNCTION_CALL_CLASS, "Get Parent", [output("ReturnValue", stop, "self")]),
                ],
            }
        ],
    }, entry, stop, parent


def fixture_transition_source():
    asset = "/Asset"
    function = "PlayStopLeftTransitionAnimation"
    graph = asset + ":" + function
    entry = graph + ".Entry"
    transition = graph + ".Transition"
    parent = graph + ".Parent"
    def node(path, class_path, title, pins):
        return {"object_path": path, "class_path": class_path, "title": title, "pins": pins}
    def output(name, target, target_pin):
        return {"name": name, "direction": "Output", "linked_to": [{"node_object_path": target, "pin_name": target_pin}]}
    def input(name, value="", object_value="None"):
        return {"name": name, "direction": "Input", "default_value": value, "default_object": object_value}
    return {
        "asset_path": asset,
        "graphs": [
            {
                "object_path": graph,
                "nodes": [
                    node(entry, FUNCTION_ENTRY_CLASS, function, [output("then", transition, "execute")]),
                    node(
                        transition,
                        FUNCTION_CALL_CLASS,
                        "Play Transition Animation",
                        [
                            input("execute"),
                            input("self"),
                            input("Sequence", object_value="/ALS/ALS/Animations/Transitions/A_Als_Stop_Left.A_Als_Stop_Left"),
                            input("BlendInDuration", "0.200000"),
                            input("BlendOutDuration", "0.200000"),
                            input("PlayRate", "1.500000"),
                            input("StartTime", "0.400000"),
                            input("bFromStandingIdleOnly", "false"),
                        ],
                    ),
                    node(parent, FUNCTION_CALL_CLASS, "Get Parent", [output("ReturnValue", transition, "self")]),
                ],
            }
        ],
    }, function, entry, parent, transition


def fixture_transition_notify_source():
    asset = "/Asset"
    function = "AnimNotify_ActivatePivot"
    graph = asset + ":" + function
    event_graph = asset + ":EventGraph"
    entry = graph + ".Entry"
    bridge = graph + ".Bridge"
    event = event_graph + ".Event"
    target = event_graph + ".Target"
    parent = event_graph + ".Parent"
    other_event = event_graph + ".OtherEvent"
    other_target = event_graph + ".OtherTarget"
    other_parent = event_graph + ".OtherParent"
    def output(name, target, target_pin):
        return {"name": name, "direction": "Output", "linked_to": [{"node_object_path": target, "pin_name": target_pin}]}
    def node(path, class_path, title, pins, properties=None):
        return {"object_path": path, "class_path": class_path, "title": title, "pins": pins, "properties": properties or []}
    member = lambda name: [{"exported_text": f'(MemberName="{name}")'}]
    return {
        "asset_path": asset,
        "graphs": [
            {"object_path": graph, "nodes": [
                node(entry, FUNCTION_ENTRY_CLASS, function, [output("then", bridge, "execute")]),
                node(bridge, FUNCTION_CALL_CLASS, "Execute Ubergraph Fixture", []),
            ]},
            {"object_path": event_graph, "nodes": [
                node(event, "/Script/BlueprintGraph.K2Node_Event", function, [output("then", target, "execute")]),
                node(target, FUNCTION_CALL_CLASS, "Activate Pivot", [], member("ActivatePivot")),
                node(parent, FUNCTION_CALL_CLASS, "Get Parent", [output("ReturnValue", target, "self")]),
                node(other_event, "/Script/BlueprintGraph.K2Node_Event", "AnimNotify_StopQuick", [output("then", other_target, "execute")]),
                node(other_target, FUNCTION_CALL_CLASS, "Play Quick Stop Animation", [], member("PlayQuickStopAnimation")),
                node(other_parent, FUNCTION_CALL_CLASS, "Get Parent", [output("ReturnValue", other_target, "self")]),
            ]},
        ],
    }, function, entry, bridge, event, parent, target


def run_self_test():
    source, entry, stop, parent = fixture_source()
    plan = resolve_lifecycle(
        source,
        [
            {
                "source_function": STOP_FUNCTION,
                "call_site": "StateExitFunction",
                "source_node": "/Asset:State.Result",
                "source_graph": "StopTransitionAndTurnInPlaceAnimations",
                "resolution": "graph",
            },
            {
                "source_function": "RefreshInAir",
                "call_site": "anim_node",
                "source_node": "/Asset:State.Refresh",
                "source_graph": None,
                "resolution": "external",
            },
        ],
    )
    providers = {item["provider"] for item in plan["dispatches"]}
    assert providers == {TRANSITION_NOTIFY_PROVIDER, "als_locomotion_refresh_in_air"}
    assert set(plan["handled_nodes"]) == {"/Asset:State.Result", entry, stop, parent}
    transition_source, transition_function, transition_entry, transition_parent, transition = fixture_transition_source()
    transition_plan = resolve_lifecycle(
        transition_source,
        [
            {
                "source_function": transition_function,
                "call_site": "StateEntryFunction",
                "source_node": "/Asset:State.PlantLeft",
                "source_graph": transition_function,
                "resolution": "graph",
            }
        ],
    )
    assert transition_plan["dispatches"] == [
        {
            "source_function": transition_function,
            "call_site": "StateEntryFunction",
            "source_node": "/Asset:State.PlantLeft",
            "provider": TRANSITION_NOTIFY_PROVIDER,
        }
    ]
    assert set(transition_plan["handled_nodes"]) == {
        "/Asset:State.PlantLeft",
        transition_entry,
        transition_parent,
        transition,
    }
    notify_source, notify_function, notify_entry, notify_bridge, notify_event, notify_parent, notify_target = fixture_transition_notify_source()
    notify_plan = resolve_lifecycle(
        notify_source,
        [{
            "source_function": notify_function,
            "call_site": "TransitionEnd",
            "source_node": "/Asset:State.Transition",
            "source_graph": notify_function,
            "resolution": "graph",
        }],
    )
    assert notify_plan["dispatches"][0]["provider"] == TRANSITION_NOTIFY_PROVIDER
    assert set(notify_plan["handled_nodes"]) == {
        "/Asset:State.Transition",
        notify_entry,
        notify_bridge,
        notify_event,
        notify_parent,
        notify_target,
    }
    import copy
    call_source = copy.deepcopy(notify_source)
    call_function = "AnimNotify_CustomTransition"
    call_source["graphs"][0]["object_path"] = "/Asset:" + call_function
    call_source["graphs"][0]["nodes"][0]["title"] = call_function
    call_source["graphs"][1]["nodes"][0]["title"] = call_function
    target = call_source["graphs"][1]["nodes"][1]
    target["title"] = "Play Transition Left Animation"
    target["properties"] = [{"exported_text": '(MemberParent="/Script/CoreUObject.Class\'/Script/ALS.AlsAnimationInstance\'",MemberName="PlayTransitionLeftAnimation")'}]
    target["pins"] = [{"name":name,"direction":"Input","default_value":value,"linked_to":[]} for name,value in (
        ("execute",""),("self",""),("BlendInDuration","0.2"),("BlendOutDuration","0.2"),("PlayRate","1.5"),("StartTime","0.3"),("bFromStandingIdleOnly","true"))]
    dependency = {"source_function":call_function,"call_site":"TransitionStart","source_node":"/Asset:State.Transition","source_graph":call_function,"resolution":"graph"}
    call_plan = resolve_lifecycle(call_source, [dependency])
    call = call_plan["dispatches"][0]["native_call"]
    assert call == {"function":"PlayTransitionLeftAnimation", "arguments":[{"property":name,"type":kind,"value":value} for name,kind,value in (
        ("BlendInDuration","F32",0.2),("BlendOutDuration","F32",0.2),("PlayRate","F32",1.5),("StartTime","F32",0.3),("bFromStandingIdleOnly","Bool",True))]}
    target["pins"][5]["default_value"] = "0.65"
    changed = resolve_lifecycle(call_source, [dependency])["dispatches"][0]["native_call"]
    assert changed["arguments"][3]["value"] == 0.65
    target["properties"][0]["exported_text"] = target["properties"][0]["exported_text"].replace("PlayTransitionLeftAnimation", "PlayTransitionRightAnimation")
    assert resolve_lifecycle(call_source, [dependency])["dispatches"][0]["native_call"]["function"] == "PlayTransitionRightAnimation"
    for corruption in ("argument_link", "wrong_parent", "extra_execution"):
        invalid = copy.deepcopy(call_source)
        target = invalid["graphs"][1]["nodes"][1]
        if corruption == "argument_link": target["pins"][5]["linked_to"] = [{"node_object_path":"/Asset:Other"}]
        elif corruption == "wrong_parent": target["properties"][0]["exported_text"] = target["properties"][0]["exported_text"].replace("/Script/ALS.AlsAnimationInstance", "/Script/Other.Instance")
        else: invalid["graphs"][1]["nodes"][0]["pins"][0]["linked_to"].append({"node_object_path":"/Asset:Other","pin_name":"execute"})
        try:
            resolve_lifecycle(invalid, [dependency])
        except LifecycleRenderError:
            pass
        else:
            raise AssertionError("An unsupported transition notify chain was accepted")
    assert {
        "InitializeHead",
        "RefreshHead",
        "InitializeGrounded",
        "RefreshGrounded",
        "ResetGroundedEntryMode",
        "RefreshGroundedMovement",
        "InitializeStandingMovement",
        "RefreshStandingMovement",
        "ResetPivot",
        "RefreshCrouchingMovement",
        "RefreshDynamicTransitions",
        "InitializeTurnInPlace",
        "RefreshTurnInPlace",
        "SetHipsDirection",
    }.issubset(EXTERNAL_PROVIDERS)
    return {
        "local_stop_function_chain_validated": "PASS",
        "state_transition_function_chain_validated": "PASS",
        "transition_notify_event_handler_chain_validated": "PASS",
        "existing_runtime_lifecycle_providers_mapped": "PASS",
    }


if __name__ == "__main__":
    print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
