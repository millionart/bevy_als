import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
IMPORT_ROOT = ROOT / "tools" / "import"
sys.path.insert(0, str(IMPORT_ROOT))

from extract_ue_visible_graph_contract import contract
import generate_ue_transition_expression_graph as transition_renderer
from ue_animation_asset_resolver import (
    AssetResolutionError,
    source_blend_profile_wrapper,
    source_curve_float,
    source_object_path,
)
from ue_animgraph_fsm_contract import (
    ALIAS_CLASS,
    CONDUIT_CLASS,
    ENTRY_CLASS,
    FSM_CLASS,
    STATE_CLASS,
    FsmContractError,
    fsm_contract,
    graph_for_suffix,
)


class FsmRenderError(ValueError):
    pass


def visible_input_pins(node):
    return [
        pin
        for pin in node.get("pins", [])
        if pin.get("direction") == "Input" and not pin.get("hidden")
    ]


def is_bool_pin(pin):
    return any(
        value.get("name") == "PinCategory" and value.get("exported_text") == "bool"
        for value in pin.get("pin_type_properties", [])
        if isinstance(value, dict)
    )


def bool_condition_contract(source, suffix):
    value = contract(source, suffix)
    roots = [
        node
        for node in value["source_graph"]["nodes"]
        if node.get("class_path") == "/Script/AnimGraph.AnimGraphNode_TransitionResult"
    ]
    if len(roots) != 1:
        raise FsmRenderError("source FSM condition root is missing or ambiguous")
    pins = visible_input_pins(roots[0])
    if len(pins) != 1 or not is_bool_pin(pins[0]):
        raise FsmRenderError("source FSM condition result is not Bool")
    try:
        transition_renderer.graph_text(value, validate_only=True)
    except ValueError as error:
        raise FsmRenderError("source FSM condition renderer is unsupported") from error
    return value


def ordered_outgoing(transitions):
    values = {}
    for transition in transitions:
        values.setdefault(transition["source"], []).append(transition)
    for entries in values.values():
        entries.sort(key=lambda item: (item["priority"], item["source_order"], item["id"]))
    return {
        source: [entry["id"] for entry in entries]
        for source, entries in sorted(values.items())
    }


def profile_plan(transition, source_content_root, source_mount, source_reader=None, curve_cache=None):
    if transition["blend_mode"] not in {
        "Linear",
        "Cubic",
        "HermiteCubic",
        "QuadraticInOut",
        "Custom",
    }:
        raise FsmRenderError("source transition blend mode is unsupported")
    if transition["logic_type"] not in {"TLT_StandardBlend", "TLT_Inertialization"}:
        raise FsmRenderError("source transition logic is unsupported")
    if transition["only_evaluate_when_active"]:
        raise FsmRenderError("source active-only transition rule is unsupported")
    if transition["sync_group_requirement"] != "None":
        raise FsmRenderError("source transition sync marker requirement is unsupported")
    if transition["min_time_before_reentry"] >= 0.0:
        raise FsmRenderError("source transition reentry debounce is unsupported")
    if transition["allow_inertialization_self_transition"]:
        raise FsmRenderError("source transition self inertialization is unsupported")
    try:
        profile = source_blend_profile_wrapper(
            source_content_root,
            source_mount,
            transition["blend_profile_wrapper"],
        )
    except AssetResolutionError as error:
        raise FsmRenderError(str(error)) from error
    custom_curve = []
    source_custom_curve = None
    if transition["blend_mode"] == "Custom":
        if transition["logic_type"] != "TLT_StandardBlend":
            raise FsmRenderError("source custom inertial transition is unsupported")
        reference = transition["source_properties"].get("CustomBlendCurve")
        if not isinstance(reference, str) or source_content_root is None or source_mount is None:
            raise FsmRenderError("source custom blend curve is unavailable")
        try:
            source_custom_curve = source_object_path(reference)
            curve_cache = curve_cache if curve_cache is not None else {}
            curve = curve_cache.get(source_custom_curve)
            if curve is None:
                curve = source_curve_float(
                    source_content_root,
                    source_mount,
                    source_custom_curve,
                    source_reader,
                )
                curve_cache[source_custom_curve] = curve
            custom_curve = curve["keys"]
        except AssetResolutionError as error:
            raise FsmRenderError(str(error)) from error
    return {
        "logic": "inertialization"
        if transition["logic_type"] == "TLT_Inertialization"
        else "profiled_blend",
        "duration": transition["crossfade_duration"],
        "blend_mode": transition["blend_mode"],
        "blend_profile": profile,
        "source_custom_curve": source_custom_curve,
        "custom_curve": custom_curve,
        "automatic_rule": (
            {
                "source_semantics": "relevant_asset_time_remaining",
                "trigger_time": transition["automatic_trigger_time"],
            }
            if transition["automatic"]
            else None
        ),
    }


def render_plan(source, suffix, source_content_root=None, source_mount=None, source_reader=None):
    try:
        value = fsm_contract(source, suffix)
        graph = graph_for_suffix(source, suffix)
    except FsmContractError as error:
        raise FsmRenderError(str(error)) from error
    if graph.get("class_path") != FSM_CLASS:
        raise FsmRenderError("source graph is not an FSM")
    source_nodes = {node.get("object_path"): node for node in graph.get("nodes", [])}
    structural = {
        item["source_node"]: "state"
        for item in value["states"]
    }
    structural.update({item["source_node"]: "alias" for item in value["aliases"]})
    structural.update({item["source_node"]: "conduit" for item in value["conduits"]})
    if set(structural) != {
        path
        for path, node in source_nodes.items()
        if node.get("class_path") in {STATE_CLASS, ALIAS_CLASS, CONDUIT_CLASS}
    }:
        raise FsmRenderError("source FSM structural nodes are inconsistent")
    entry_nodes = [node for node in source_nodes.values() if node.get("class_path") == ENTRY_CLASS]
    if len(entry_nodes) != 1:
        raise FsmRenderError("source FSM entry is invalid")
    conditions = {}
    conduit_conditions = {}
    curve_cache = {}
    for conduit in value["conduits"]:
        bool_condition_contract(source, conduit["graph"])
        conduit_conditions[conduit["source_node"]] = conduit["graph"]
    transitions = []
    for transition in value["transitions"]:
        source_path = f"{source['asset_path']}:{transition['source']}"
        target_path = f"{source['asset_path']}:{transition['target']}"
        if source_path not in structural or target_path not in structural:
            raise FsmRenderError("source FSM transition endpoint is unavailable")
        bool_condition_contract(source, transition["condition_graph"])
        conditions[transition["id"]] = transition["condition_graph"]
        transitions.append(
            {
                **transition,
                "render": profile_plan(
                    transition,
                    source_content_root,
                    source_mount,
                    source_reader,
                    curve_cache,
                ),
            }
        )
    return {
        "format": "ue_animgraph_fsm_render_plan_v1",
        "source_graph": suffix,
        "entry_target": value["entry_target"],
        "states": value["states"],
        "aliases": value["aliases"],
        "conduits": value["conduits"],
        "conduit_conditions": conduit_conditions,
        "conditions": conditions,
        "transitions": transitions,
        "outgoing_order": ordered_outgoing(transitions),
    }


def run_self_test():
    transitions = [
        {"id": "late", "source": "State", "priority": 2, "source_order": 0},
        {"id": "first", "source": "State", "priority": 1, "source_order": 1},
        {"id": "second", "source": "State", "priority": 1, "source_order": 2},
    ]
    assert ordered_outgoing(transitions) == {"State": ["first", "second", "late"]}
    standard = {
        "blend_mode": "Cubic",
        "logic_type": "TLT_StandardBlend",
        "only_evaluate_when_active": False,
        "sync_group_requirement": "None",
        "min_time_before_reentry": -1.0,
        "allow_inertialization_self_transition": False,
        "blend_profile_wrapper": "(bIsSkeletonBlendProfile=True)",
        "crossfade_duration": 0.25,
        "automatic": True,
        "automatic_trigger_time": 0.0,
    }
    plan = profile_plan(standard, None, None)
    assert plan["logic"] == "profiled_blend"
    assert plan["automatic_rule"]["source_semantics"] == "relevant_asset_time_remaining"
    return {
        "source_transition_priority_order_preserved": "PASS",
        "source_automatic_rule_semantic_preserved": "PASS",
        "empty_source_blend_profile_preserved": "PASS",
    }


if __name__ == "__main__":
    import json

    print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
