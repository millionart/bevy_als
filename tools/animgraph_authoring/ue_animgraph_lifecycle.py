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
        "existing_runtime_lifecycle_providers_mapped": "PASS",
    }


if __name__ == "__main__":
    print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
