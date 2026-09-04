import json


FUNCTION_ENTRY_CLASS = "/Script/BlueprintGraph.K2Node_FunctionEntry"
FUNCTION_CALL_CLASS = "/Script/BlueprintGraph.K2Node_CallFunction"
STOP_FUNCTION = "StopTransitionAndTurnInPlaceAnimations"
EXTERNAL_PROVIDERS = {
    "InitializeLean": "als_locomotion_initialize_lean",
    "InitializeGrounded": "bevy_als::grounded_layer::GroundedLayerPlugin",
    "RefreshGrounded": "bevy_als::actual_motion::AlsActualMotionPlugin + bevy_als::standing_layer::StandingLayerPlugin + bevy_als::crouching_layer::CrouchingLayerPlugin",
    "ResetGroundedEntryMode": "bevy_als::grounded_layer::GroundedLayerPlugin",
    "RefreshGroundedMovement": "bevy_als::standing_layer::StandingLayerPlugin + bevy_als::crouching_layer::CrouchingLayerPlugin",
    "InitializeStandingMovement": "bevy_als::standing_layer::StandingLayerPlugin",
    "RefreshStandingMovement": "bevy_als::standing_layer::StandingLayerPlugin",
    "ResetPivot": "bevy_als::standing_layer::StandingLayerPlugin",
    "RefreshCrouchingMovement": "bevy_als::crouching_layer::CrouchingLayerPlugin",
    "RefreshDynamicTransitions": "bevy_als::standing_layer::StandingLayerPlugin + bevy_als::crouching_layer::CrouchingLayerPlugin",
    "RefreshInAir": "als_locomotion_refresh_in_air",
    "RefreshRotateInPlace": "als_turn_layer_refresh_rotate_in_place",
    "InitializeTurnInPlace": "bevy_als::turn_layer::TurnLayerPlugin",
    "RefreshTurnInPlace": "bevy_als::turn_layer::TurnLayerPlugin",
    "SetHipsDirection": "bevy_als::standing_layer::StandingLayerPlugin + bevy_als::crouching_layer::CrouchingLayerPlugin",
}
TRANSITION_FUNCTIONS = {
    "PlayStopLeftTransitionAnimation": {
        "call": "Play Transition Animation",
        "sequence": "A_Als_Stop_Left.A_Als_Stop_Left",
        "start_time": "0.400000",
        "provider": "bevy_als::standing_layer::StandingLayerPlugin",
    },
    "PlayStopRightTransitionAnimation": {
        "call": "Play Transition Animation",
        "sequence": "A_Als_Stop_Right.A_Als_Stop_Right",
        "start_time": "0.400000",
        "provider": "bevy_als::standing_layer::StandingLayerPlugin",
    },
    "PlayStopTransitionAnimation": {
        "call": "Play Transition Right Animation",
        "sequence": None,
        "start_time": "0.200000",
        "provider": "bevy_als::crouching_layer::CrouchingLayerPlugin",
    },
    "PlayRollToGroundedTransitionAnimation": {
        "call": "Play Transition Right Animation",
        "sequence": None,
        "start_time": "0.200000",
        "provider": "bevy_als::grounded_layer::GroundedLayerPlugin",
    },
}


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
                    "provider": "als_locomotion_exit_stop",
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
    assert providers == {"als_locomotion_exit_stop", "als_locomotion_refresh_in_air"}
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
            "provider": "bevy_als::standing_layer::StandingLayerPlugin",
        }
    ]
    assert set(transition_plan["handled_nodes"]) == {
        "/Asset:State.PlantLeft",
        transition_entry,
        transition_parent,
        transition,
    }
    assert {
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
        "existing_runtime_lifecycle_providers_mapped": "PASS",
    }


if __name__ == "__main__":
    print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
