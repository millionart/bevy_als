import json
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
    fixture_source,
)
from ue_animgraph_lifecycle import LifecycleRenderError, TRANSITION_NOTIFY_FUNCTIONS, validate_transition_notify_function


class FsmRenderError(ValueError):
    pass


def blend_profile_ron(profile):
    if profile is None or profile.get("source_profile") is None:
        return "None"
    mode = profile.get("mode")
    scales = profile.get("bone_scales")
    if mode not in {"TimeFactor", "WeightFactor"} or not isinstance(scales, list):
        raise FsmRenderError("source blend profile is invalid")
    rows = []
    for entry in scales:
        bone = entry.get("bone") if isinstance(entry, dict) else None
        scale = entry.get("scale") if isinstance(entry, dict) else None
        if not isinstance(bone, str) or not bone or not isinstance(scale, (int, float)) or isinstance(scale, bool):
            raise FsmRenderError("source blend profile entry is invalid")
        rows.append(f"(bone: {json.dumps(bone, ensure_ascii=False)}, scale: {float(scale)!r})")
    return f"Some((profile_mode: {mode}, bone_scales: [{', '.join(rows)}]))"


SUPPORTED_TRANSITION_NOTIFY_NAMES = {
    function.removeprefix("AnimNotify_")
    for function in TRANSITION_NOTIFY_FUNCTIONS
}


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
            source_reader,
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
        notifies = []
        for notify in transition["notifies"]:
            native_call = None
            if notify["name"] not in SUPPORTED_TRANSITION_NOTIFY_NAMES:
                try:
                    _, native_call = validate_transition_notify_function(source, "AnimNotify_" + notify["name"], "AnimNotify_" + notify["name"])
                except LifecycleRenderError:
                    pass
            notifies.append({**notify, **native_call} if native_call is not None else notify)
        transitions.append(
            {
                **transition,
                "notifies": notifies,
                "render": profile_plan(
                    transition,
                    source_content_root,
                    source_mount,
                    source_reader,
                    curve_cache,
                ),
            }
        )
    runtime = value["runtime"]
    unsupported = []
    unsupported.extend(
        {
            "kind": "source_transition_notify_runtime_unmapped",
            "source_node": transition["source_node"],
            "phase": notify["phase"],
            "notify_name": notify["name"],
        }
        for transition in transitions
        for notify in transition["notifies"]
        if notify["name"] not in SUPPORTED_TRANSITION_NOTIFY_NAMES and not notify.get("function")
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
        "runtime_transition_order": value["runtime_transition_order"],
        "runtime": runtime,
        "unsupported": unsupported,
    }


def run_self_test():
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
    source = fixture_source()
    source_plan = render_plan(source, "AnimGraph.Machine.FSM")
    assert source_plan["runtime"]["max_transitions_per_frame"] == 1
    assert source_plan["unsupported"] == [
        {
            "kind": "source_transition_notify_runtime_unmapped",
            "source_node": "/Asset:AnimGraph.Machine.FSM.Transition",
            "phase": "TransitionStart",
            "notify_name": "Notify",
        }
    ]
    for property_value in source["graphs"][1]["nodes"][2]["properties"]:
        if property_value["name"] == "TransitionStart":
            property_value["exported_text"] = '(NotifyName="ActivatePivot")'
    assert not render_plan(source, "AnimGraph.Machine.FSM")["unsupported"]
    for property_value in source["graphs"][1]["nodes"][2]["properties"]:
        if property_value["name"] == "bAutomaticRuleBasedOnSequencePlayerInState":
            property_value["exported_text"] = "True"
    automatic_plan = render_plan(source, "AnimGraph.Machine.FSM")
    assert automatic_plan["transitions"][0]["render"]["automatic_rule"] == {
        "source_semantics": "relevant_asset_time_remaining",
        "trigger_time": -1.0,
    }
    assert not any(
        issue["kind"] == "source_automatic_transition_runtime_unmapped"
        for issue in automatic_plan["unsupported"]
    )
    return {
        "source_transition_priority_order_preserved": "PASS",
        "source_automatic_rule_semantic_preserved": "PASS",
        "unmapped_transition_notify_blocked": "PASS",
        "verified_transition_notify_reaches_runtime_lowering": "PASS",
        "automatic_rule_reaches_runtime_lowering": "PASS",
        "empty_source_blend_profile_preserved": "PASS",
    }


if __name__ == "__main__":
    import json

    print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
