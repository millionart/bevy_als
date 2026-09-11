import argparse
import copy
import importlib.util
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools" / "import"))

from extract_ue_visible_graph_contract import contract
from apply_additive_reference_deltas import (
    asset_paths as additive_asset_paths,
    graph_clip_uses as additive_graph_clip_uses,
    patch_graph as patch_additive_graph,
    patch_sequence_player_references,
    raw_scale_summary,
    referenced_animation_names,
    source_animation_assets,
)
from ue_animation_asset_resolver import AssetResolutionError, resolve_graph_animation_assets
from ue_animgraph_pose_contract import (
    PoseContractError,
    input_is_linked,
    linked_anim_graph_contract,
    linked_anim_layer_contract,
    pose_contract,
    property_binding_paths,
)
from ue_animgraph_pose_renderer import PoseRenderError, SUPPORTED_RENDERERS, render as render_pose_graph
from ue_animgraph_reachability import traverse
from ue_animgraph_fsm_contract import FsmContractError, fsm_contract
from ue_animgraph_fsm_renderer import FsmRenderError, render_plan as render_fsm_plan
from ue_animgraph_lifecycle import LifecycleRenderError, resolve_lifecycle
from ue_control_rig_contract import ControlRigContractError, compile_contract as compile_rig_contract
from ue_control_rig_target import STRUCT_FIELDS as RIG_STRUCT_FIELDS, compile_target as compile_rig_target
from ue_animgraph_materializer import (
    MaterializationError,
    asset_namespace,
    graph_path,
    materialize_files,
    materialized_reference_issues,
    source_asset_directory,
)


IR_FORMAT = "ue_animgraph_contract_ir_v1"
REPORT_FORMAT = "ue_animgraph_conversion_report_v1"
INTEGRATION_REPORT_FORMAT = "ue_animgraph_integration_report_v1"
INSTALLATION_MANIFEST_FORMAT = "ue_animgraph_installation_manifest_v1"
RUNTIME_PROVIDER_CONTRACT_FORMAT = "als_runtime_provider_contract_v1"
RUNTIME_PROVIDER_CONTRACT_PATH = ROOT / "tools" / "animgraph_authoring" / "als_runtime_provider_contract.json"
FORBIDDEN_REGISTRY_KEYS = {"asset", "asset_name", "asset_path", "source_asset", "node_guid"}
IR_ONLY_RENDERERS = {"root_output", "comment_group"}
TRANSITION_RENDERERS = {
    "transition_result",
    "anim_getter",
    "property_access",
    "boolean_operator",
    "comparison_operator",
    "gameplay_tag_equal",
    "boolean_to_float",
    "boolean_not",
    "absolute",
    "gameplay_tag_valid",
}
FSM_RENDERERS = {
    "state_machine",
    "fsm_entry",
    "fsm_state",
    "fsm_alias",
    "fsm_conduit",
    "fsm_transition",
}
POSE_SCALAR_RENDERERS = {
    "property_access",
    "boolean_operator",
    "comparison_operator",
    "gameplay_tag_equal",
    "boolean_to_float",
}
_TRANSITION_RENDERER = None


class ConversionError(RuntimeError):
    pass


def transition_renderer():
    global _TRANSITION_RENDERER
    if _TRANSITION_RENDERER is None:
        path = ROOT / "tools" / "import" / "generate_ue_transition_expression_graph.py"
        specification = importlib.util.spec_from_file_location("ue_transition_renderer", path)
        if specification is None or specification.loader is None:
            raise ConversionError("transition renderer is unavailable")
        module = importlib.util.module_from_spec(specification)
        specification.loader.exec_module(module)
        _TRANSITION_RENDERER = module
    return _TRANSITION_RENDERER


def render_transition_contract(graph_contract):
    return transition_renderer().graph_text(graph_contract, validate_only=True)


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def runtime_provider_contract():
    try:
        value = read_json(RUNTIME_PROVIDER_CONTRACT_PATH)
    except (OSError, json.JSONDecodeError) as error:
        raise ConversionError("runtime provider contract is unavailable") from error
    expected = {
        "format",
        "runtime_inputs",
        "sync_phase_providers",
        "slot_providers",
        "gameplay_tag_providers",
    }
    if not isinstance(value, dict) or set(value) != expected or value.get("format") != RUNTIME_PROVIDER_CONTRACT_FORMAT:
        raise ConversionError("runtime provider contract is invalid")
    for category in expected - {"format"}:
        providers = value.get(category)
        if not isinstance(providers, dict) or any(
            not isinstance(key, str)
            or not key
            or not isinstance(provider, str)
            or not provider
            for key, provider in providers.items()
        ):
            raise ConversionError("runtime provider contract is invalid")
    return value


RUNTIME_PROVIDER_CONTRACT = runtime_provider_contract()


def runtime_provider(category, key):
    value = RUNTIME_PROVIDER_CONTRACT[category].get(key, "")
    return value if isinstance(value, str) else ""


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def content_root_for_uasset(uasset, configured_root):
    if configured_root is not None:
        root = configured_root.resolve()
        if not root.is_dir():
            raise ConversionError("content root is unavailable")
        return root
    for parent in [uasset.parent, *uasset.parents]:
        if parent.name.lower() == "content":
            return parent
    raise ConversionError("content root cannot be inferred from uasset")


def package_path_for_uasset(uasset, content_root, mount):
    if uasset.suffix.lower() != ".uasset" or not uasset.is_file():
        raise ConversionError("uasset input is unavailable")
    if not isinstance(mount, str) or not mount.startswith("/") or mount == "/":
        raise ConversionError("mount is invalid")
    try:
        relative = uasset.resolve().relative_to(content_root.resolve())
    except ValueError as error:
        raise ConversionError("uasset is outside content root") from error
    asset_name = relative.stem
    package = relative.with_suffix("").as_posix()
    return f"{mount.rstrip('/')}/{package}.{asset_name}"


def source_reader_failure(report_path, exit_code):
    if report_path.is_file():
        try:
            report = read_json(report_path)
        except (OSError, json.JSONDecodeError):
            report = None
        if isinstance(report, dict):
            reason = report.get("exit_reason")
            if isinstance(reason, str) and reason:
                return f"native UE source reader failed: {reason}"
    return f"native UE source reader failed with exit code {exit_code}"


def read_uasset_source(uasset, content_root, mount, source_reader, timeout_seconds, no_progress_seconds):
    if not source_reader.is_file():
        raise ConversionError("native UE source reader is unavailable")
    if timeout_seconds <= 0 or no_progress_seconds <= 0:
        raise ConversionError("native UE source reader timeout is invalid")
    resolved_uasset = uasset.resolve()
    resolved_content = content_root_for_uasset(resolved_uasset, content_root)
    package_path = package_path_for_uasset(resolved_uasset, resolved_content, mount)
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as directory:
        intermediate = Path(directory) / "source.ue.json"
        reader_report = Path(str(intermediate) + ".reader.report.json")
        process = subprocess.run(
            [
                "pwsh",
                "-NoProfile",
                "-File",
                str(source_reader),
                "-Asset",
                package_path,
                "-Output",
                str(intermediate),
                "-TimeoutSeconds",
                str(timeout_seconds),
                "-NoProgressSeconds",
                str(no_progress_seconds),
            ],
            cwd=ROOT,
            capture_output=True,
            text=True,
            check=False,
        )
        if process.returncode != 0 or not intermediate.is_file():
            raise ConversionError(source_reader_failure(reader_report, process.returncode))
        source = read_json(intermediate)
    if source.get("asset_path") != package_path:
        raise ConversionError("native UE source reader returned an unexpected asset")
    return source, {
        "uasset_name": resolved_uasset.name,
        "package_path": package_path,
    }


def source_signature(node):
    value = node.get("title")
    if not isinstance(value, str) or not value:
        return ""
    return value.splitlines()[0]


def source_property_key(source_path):
    if not isinstance(source_path, str) or not source_path:
        raise ConversionError("source property path is invalid")
    if source_path.startswith("GetParent."):
        source_path = source_path[len("GetParent.") :]
    parts = source_path.split(".")
    if any(not re.fullmatch(r"[A-Za-z][A-Za-z0-9_]*", part) for part in parts):
        raise ConversionError("source property path is invalid")
    return ".".join(
        re.sub(r"(?<=[a-z0-9])(?=[A-Z])", "_", re.sub(r"([A-Z]+)([A-Z][a-z])", r"\1_\2", part)).lower()
        for part in parts
    )


def source_node_guid(source, object_path):
    matches = [
        node
        for graph in source.get("graphs", [])
        for node in graph.get("nodes", [])
        if node.get("object_path") == object_path
    ]
    if len(matches) != 1:
        raise ConversionError("source animation graph node is missing or ambiguous")
    guid = matches[0].get("node_guid")
    if not isinstance(guid, str) or re.fullmatch(
        r"[0-9A-Fa-f]{8}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{12}", guid
    ) is None:
        raise ConversionError("source animation graph node identifier is invalid")
    return guid.lower()


def animation_getter_machine_plans(source, fsm_plans):
    plans = {}
    for suffix, plan in fsm_plans.items():
        machine = f"{source['asset_path']}:{suffix.rsplit('.', 1)[0]}"
        state_ids = {
            item["source_node"]: item["id"]
            for item in [*plan["states"], *plan["aliases"]]
            if isinstance(item.get("source_node"), str) and isinstance(item.get("id"), str)
        }
        if not state_ids:
            raise ConversionError("source animation state machine states are unavailable")
        plans[machine] = {
            **plan,
            "fsm_node_id": source_node_guid(source, machine),
            "state_ids": state_ids,
        }
    return plans


def bind_animation_getter(semantic, state_machine_plans):
    if not isinstance(semantic, dict):
        raise ConversionError("source animation getter semantic is unavailable")
    function = semantic.get("source_function")
    machine = semantic.get("source_machine")
    state = semantic.get("source_state")
    if function not in {"GetInstanceStateWeight", "GetInstanceMachineWeight"}:
        raise ConversionError("source animation getter function is unsupported")
    if not isinstance(machine, str) or not machine:
        raise ConversionError("source animation getter machine is unavailable")
    plan = state_machine_plans.get(machine)
    if not isinstance(plan, dict) or not isinstance(plan.get("fsm_node_id"), str):
        raise ConversionError("source animation getter machine target is unavailable")
    if not isinstance(state, str):
        raise ConversionError("source animation getter state is invalid")
    semantic["target_fsm_node"] = plan["fsm_node_id"]
    if function == "GetInstanceStateWeight":
        target_state = plan.get("state_ids", {}).get(state)
        if not isinstance(target_state, str) or not target_state:
            raise ConversionError("source animation getter state target is unavailable")
        semantic["target_state_id"] = target_state
    elif state:
        raise ConversionError("source machine weight getter unexpectedly references a state")


def validate_registry(value):
    if not isinstance(value, dict) or value.get("format") != "als_ref_to_bevy_animgraph_mapping_registry_v1":
        raise ConversionError("mapping registry format is invalid")
    if set(value) - {
        "format",
        "node_classes",
        "function_rules",
        "enum_entry_values",
    }:
        raise ConversionError("mapping registry contains unsupported configuration")
    node_classes = value.get("node_classes")
    functions = value.get("function_rules")
    enum_entry_values = value.get("enum_entry_values", {})
    if not isinstance(node_classes, dict) or not node_classes:
        raise ConversionError("mapping registry node classes are invalid")
    if not isinstance(functions, list):
        raise ConversionError("mapping registry function rules are invalid")
    if not isinstance(enum_entry_values, dict):
        raise ConversionError("mapping registry enum entry values are invalid")
    for key, mapping in node_classes.items():
        if not isinstance(key, str) or not key.startswith("/Script/"):
            raise ConversionError("mapping registry source class is invalid")
        if not isinstance(mapping, dict) or set(mapping) != {"renderer"}:
            raise ConversionError("mapping registry node class mapping is invalid")
        if not isinstance(mapping["renderer"], str) or not mapping["renderer"]:
            raise ConversionError("mapping registry renderer is invalid")
    seen = set()
    for rule in functions:
        if not isinstance(rule, dict) or set(rule) != {"source_class", "signature", "renderer"}:
            raise ConversionError("mapping registry function rule is invalid")
        if any(not isinstance(rule[key], str) or not rule[key] for key in rule):
            raise ConversionError("mapping registry function rule value is invalid")
        key = (rule["source_class"], rule["signature"])
        if key in seen:
            raise ConversionError("mapping registry function rule is duplicated")
        seen.add(key)
    for enum_name, entries in enum_entry_values.items():
        if not isinstance(enum_name, str) or not enum_name or not isinstance(entries, dict) or not entries:
            raise ConversionError("mapping registry enum entry values are invalid")
        for entry, entry_value in entries.items():
            if (
                not isinstance(entry, str)
                or not entry
                or not isinstance(entry_value, (int, float))
                or isinstance(entry_value, bool)
            ):
                raise ConversionError("mapping registry enum entry value is invalid")
    def visit(item):
        if isinstance(item, dict):
            forbidden = FORBIDDEN_REGISTRY_KEYS & set(item)
            if forbidden:
                raise ConversionError("mapping registry contains an asset-specific selector")
            for child in item.values():
                visit(child)
        elif isinstance(item, list):
            for child in item:
                visit(child)
    visit(value)
    return value


def mapping_for(node, registry):
    class_path = node.get("class_path")
    source_node = node.get("object_path")
    title = node.get("title")
    if not isinstance(class_path, str) or not isinstance(source_node, str):
        raise ConversionError("source node identity is invalid")
    class_mapping = registry["node_classes"].get(class_path)
    if class_mapping is None:
        return None, {
            "kind": "unregistered_source_node",
            "source_node": source_node,
            "source_class": class_path,
            "source_title": title,
        }
    renderer = class_mapping["renderer"]
    if renderer not in {"anim_call", "blueprint_call"}:
        return renderer, None
    signature = source_signature(node)
    matches = [
        rule
        for rule in registry["function_rules"]
        if rule["source_class"] == class_path and rule["signature"] == signature
    ]
    if len(matches) == 1:
        return matches[0]["renderer"], None
    return None, {
        "kind": "unregistered_source_function",
        "source_node": source_node,
        "source_class": class_path,
        "source_signature": signature,
        "source_title": title,
    }


def resolve_bound_keys(node, renderer, pins, paths):
    if (
        not isinstance(paths, list)
        or len(pins) != len(paths)
        or not all(isinstance(pin, str) and isinstance(path, str) for pin, path in zip(pins, paths))
    ):
        return [], [{"kind": "source_property_binding_unsupported", "source_node": node.get("object_path")}]
    keys = []
    issues = []
    for pin, path in zip(pins, paths):
        if not path:
            keys.append("")
            continue
        try:
            keys.append(source_property_key(path))
        except ConversionError as error:
            issues.append(
                {
                    "kind": "source_property_binding_unsupported",
                    "source_node": node.get("object_path"),
                    "source_class": node.get("class_path"),
                    "renderer": renderer,
                    "source_pin": pin,
                    "source_property_path": path,
                    "detail": str(error),
                }
            )
            keys.append("")
    return keys, issues


def record_resolved_bindings(semantic, pins, paths, keys):
    if not isinstance(semantic, dict):
        return
    rows = semantic.setdefault("resolved_property_bindings", [])
    if not isinstance(rows, list):
        raise ConversionError("resolved source property bindings are invalid")
    for pin, path, key in zip(pins, paths, keys):
        if path and key:
            rows.append(
                {
                    "source_pin": pin,
                    "source_property_path": path,
                    "target_key": key,
                }
            )


def resolve_blendspace_bindings(node, renderer, semantic, pins):
    try:
        bindings = property_binding_paths(node)
    except PoseContractError as error:
        return [
            {
                "kind": "source_property_binding_unsupported",
                "source_node": node.get("object_path"),
                "source_class": node.get("class_path"),
                "renderer": renderer,
                "detail": str(error),
            }
        ]
    paths = [bindings.get(pin, "") for pin in pins]
    keys, issues = resolve_bound_keys(
        node,
        renderer,
        pins,
        paths,
    )
    semantic["input_binding_paths"] = paths
    semantic["input_binding_keys"] = keys
    record_resolved_bindings(semantic, pins, paths, keys)
    return [
        *issues,
        *[
            {
                "kind": "source_property_binding_link_unsupported",
                "source_node": node.get("object_path"),
                "source_class": node.get("class_path"),
                "renderer": renderer,
                "source_pin": pin,
            }
            for pin, path in zip(pins, paths)
            if path and input_is_linked(node, pin)
        ],
    ]


def resolve_property_bindings(node, semantic, registry):
    if not isinstance(semantic, dict):
        return []
    renderer = semantic.get("renderer")
    if renderer == "property_access":
        scalar = transition_renderer()
        path = scalar.property_access_key(node)
        if not path:
            return [
                {
                    "kind": "source_property_access_unavailable",
                    "source_node": node.get("object_path"),
                    "source_class": node.get("class_path"),
                    "renderer": renderer,
                }
            ]
        outputs = scalar.AUDIT.visible_pins(node, "Output")
        if not outputs:
            return [
                {
                    "kind": "source_property_access_output_unsupported",
                    "source_node": node.get("object_path"),
                    "source_class": node.get("class_path"),
                    "renderer": renderer,
                }
            ]
        output_types = [scalar.AUDIT.source_pin_type(pin) for pin in outputs]
        if any(value not in {"F32", "Bool", "GameplayTag"} for value in output_types):
            return [
                {
                    "kind": "source_property_access_output_unsupported",
                    "source_node": node.get("object_path"),
                    "source_class": node.get("class_path"),
                    "renderer": renderer,
                }
            ]
        if all(value == "GameplayTag" for value in output_types):
            return []
        if any(value == "GameplayTag" for value in output_types):
            return [
                {
                    "kind": "source_property_access_output_unsupported",
                    "source_node": node.get("object_path"),
                    "source_class": node.get("class_path"),
                    "renderer": renderer,
                }
            ]
        pins = [scalar.source_pin_label(pin) for pin in outputs]
        keys, issues = resolve_bound_keys(node, renderer, pins, [path] * len(pins))
        semantic["property_access_path"] = path
        semantic["property_access_keys"] = keys
        record_resolved_bindings(semantic, pins, [path] * len(pins), keys)
        return issues
    if renderer == "transition_result":
        try:
            bindings = property_binding_paths(node)
        except PoseContractError as error:
            return [
                {
                    "kind": "source_property_binding_unsupported",
                    "source_node": node.get("object_path"),
                    "source_class": node.get("class_path"),
                    "renderer": renderer,
                    "detail": str(error),
                }
            ]
        path = bindings.get("bCanEnterTransition", "")
        keys, issues = resolve_bound_keys(node, renderer, ["bCanEnterTransition"], [path])
        semantic["condition_path"] = path
        semantic["condition_key"] = keys[0] if len(keys) == 1 else ""
        record_resolved_bindings(semantic, ["bCanEnterTransition"], [path], keys)
        if path and input_is_linked(node, "bCanEnterTransition"):
            issues.append(
                {
                    "kind": "source_property_binding_link_unsupported",
                    "source_node": node.get("object_path"),
                    "source_class": node.get("class_path"),
                    "renderer": renderer,
                    "source_pin": "bCanEnterTransition",
                }
            )
        return issues
    if renderer == "multi_way_blend":
        paths = semantic.get("desired_alpha_paths")
        pins = [f"DesiredAlphas_{index}" for index in range(semantic.get("pose_count", 0))]
        keys, issues = resolve_bound_keys(node, renderer, pins, paths)
        semantic["desired_alpha_keys"] = keys
        record_resolved_bindings(semantic, pins, paths, keys)
        return issues
    if renderer == "modify_curve":
        paths = semantic.get("curve_value_paths")
        pins = [f"CurveValues_{index}" for index in range(len(semantic.get("curve_names", [])))]
        keys, issues = resolve_bound_keys(node, renderer, pins, paths)
        semantic["curve_value_keys"] = keys
        alpha_keys, alpha_issues = resolve_bound_keys(
            node,
            renderer,
            ["Alpha"],
            [semantic.get("alpha_path", "")],
        )
        semantic["alpha_key"] = alpha_keys[0] if len(alpha_keys) == 1 else ""
        record_resolved_bindings(semantic, pins, paths, keys)
        record_resolved_bindings(
            semantic,
            ["Alpha"],
            [semantic.get("alpha_path", "")],
            alpha_keys,
        )
        return [*issues, *alpha_issues]
    if renderer in {"two_way_blend", "apply_additive", "apply_mesh_space_additive"}:
        keys, issues = resolve_bound_keys(
            node,
            renderer,
            ["Alpha"],
            [semantic.get("alpha_path", "")],
        )
        semantic["alpha_key"] = keys[0] if len(keys) == 1 else ""
        record_resolved_bindings(
            semantic,
            ["Alpha"],
            [semantic.get("alpha_path", "")],
            keys,
        )
        return issues
    if renderer == "layered_bone_blend":
        pins = semantic.get("blend_weight_pins")
        if not isinstance(pins, list) or not all(isinstance(pin, str) and pin for pin in pins):
            return [
                {
                    "kind": "source_layered_bone_blend_weights_unresolved",
                    "source_node": node.get("object_path"),
                    "source_class": node.get("class_path"),
                    "renderer": renderer,
                }
            ]
        try:
            bindings = property_binding_paths(node)
        except PoseContractError as error:
            return [
                {
                    "kind": "source_property_binding_unsupported",
                    "source_node": node.get("object_path"),
                    "source_class": node.get("class_path"),
                    "renderer": renderer,
                    "detail": str(error),
                }
            ]
        paths = [bindings.get(pin, "") for pin in pins]
        keys, issues = resolve_bound_keys(node, renderer, pins, paths)
        semantic["blend_weight_paths"] = paths
        semantic["blend_weight_keys"] = keys
        record_resolved_bindings(semantic, pins, paths, keys)
        return issues
    if renderer == "sequence_player":
        keys, issues = resolve_bound_keys(
            node,
            renderer,
            ["PlayRate", "bLoopAnimation"],
            [
                semantic.get("play_rate_path", ""),
                semantic.get("loop_animation_path", ""),
            ],
        )
        semantic["play_rate_key"] = keys[0] if len(keys) == 2 else ""
        semantic["loop_animation_key"] = keys[1] if len(keys) == 2 else ""
        record_resolved_bindings(
            semantic,
            ["PlayRate", "bLoopAnimation"],
            [semantic.get("play_rate_path", ""), semantic.get("loop_animation_path", "")],
            keys,
        )
        return issues
    if renderer == "blendspace_evaluator":
        inputs = semantic.get("input_pins")
        pins = [item.get("name") for item in inputs] if isinstance(inputs, list) else []
        if not pins or not all(isinstance(pin, str) and pin for pin in pins):
            return [
                {
                    "kind": "source_blendspace_evaluator_input_unresolved",
                    "source_node": node.get("object_path"),
                    "source_class": node.get("class_path"),
                    "renderer": renderer,
                }
            ]
        issues = resolve_blendspace_bindings(
            node,
            renderer,
            semantic,
            pins,
        )
        return issues
    if renderer == "blendspace_player":
        issues = resolve_blendspace_bindings(
            node,
            renderer,
            semantic,
            ["X", "Y", "PlayRate"],
        )
        keys = semantic.get("input_binding_keys")
        if not isinstance(keys, list) or len(keys) != 3 or not keys[2]:
            issues.append(
                {
                    "kind": "source_blendspace_player_play_rate_unresolved",
                    "source_node": node.get("object_path"),
                    "source_class": node.get("class_path"),
                    "renderer": renderer,
                    "source_pin": "PlayRate",
                }
            )
        return issues
    if renderer == "pose_snapshot":
        keys, issues = resolve_bound_keys(
            node,
            renderer,
            ["Snapshot"],
            [semantic.get("snapshot_path", "")],
        )
        semantic["snapshot_key"] = keys[0] if len(keys) == 1 else ""
        record_resolved_bindings(
            semantic,
            ["Snapshot"],
            [semantic.get("snapshot_path", "")],
            keys,
        )
        return issues
    if renderer == "slot":
        try:
            stem = source_property_key(semantic.get("slot_name"))
        except ConversionError as error:
            return [
                {
                    "kind": "source_slot_provider_unsupported",
                    "source_node": node.get("object_path"),
                    "source_class": node.get("class_path"),
                    "renderer": renderer,
                    "detail": str(error),
                }
            ]
        semantic["pose_key"] = stem + ".pose"
        semantic["weight_key"] = stem + ".weight"
        semantic["layer_weight_key"] = stem + ".layer_weight"
        semantic["mesh_space_additive_key"] = stem + ".mesh_space_additive"
        return []
    if renderer == "gameplay_tag_blend":
        keys, issues = resolve_bound_keys(
            node,
            renderer,
            ["ActiveTag"],
            [semantic.get("active_tag_path", "")],
        )
        semantic["active_key"] = keys[0] if len(keys) == 1 else ""
        return issues
    if renderer == "enum_pose_blend":
        keys, issues = resolve_bound_keys(
            node,
            renderer,
            ["ActiveEnumValue"],
            [semantic.get("active_enum_path", "")],
        )
        semantic["active_key"] = keys[0] if len(keys) == 1 else ""
        record_resolved_bindings(
            semantic,
            ["ActiveEnumValue"],
            [semantic.get("active_enum_path", "")],
            keys,
        )
        enum_name = semantic.get("enum_name")
        entry = semantic.get("selected_enum_entry")
        value = (
            registry.get("enum_entry_values", {}).get(enum_name, {}).get(entry)
            if isinstance(enum_name, str) and isinstance(entry, str)
            else None
        )
        if not isinstance(value, (int, float)) or isinstance(value, bool):
            issues.append(
                {
                    "kind": "source_enum_entry_unresolved",
                    "source_node": node.get("object_path"),
                    "source_class": node.get("class_path"),
                    "renderer": renderer,
                    "enum_name": enum_name,
                    "enum_entry": entry,
                }
            )
        else:
            semantic["selected_enum_value"] = float(value)
        return issues
    return []


def handled_bound_pins(prefix, paths, keys):
    if not isinstance(paths, list) or not isinstance(keys, list) or len(paths) != len(keys):
        return set()
    return {
        f"{prefix}{index}"
        for index, (path, key) in enumerate(zip(paths, keys))
        if isinstance(path, str) and path and isinstance(key, str)
    }


def handled_property_binding_pins(renderer, semantic):
    if not isinstance(semantic, dict):
        return set()
    if renderer == "multi_way_blend":
        return handled_bound_pins(
            "DesiredAlphas_",
            semantic.get("desired_alpha_paths"),
            semantic.get("desired_alpha_keys"),
        )
    if renderer == "modify_curve":
        handled = handled_bound_pins(
            "CurveValues_",
            semantic.get("curve_value_paths"),
            semantic.get("curve_value_keys"),
        )
        if semantic.get("alpha_path"):
            handled.add("Alpha")
        return handled
    if renderer == "control_rig":
        pin = semantic.get("rig_input_binding_pin")
        return {pin} if isinstance(pin, str) and pin else set()
    if renderer == "two_way_blend":
        return {"Alpha"} if semantic.get("alpha_path") else set()
    if renderer in {"apply_additive", "apply_mesh_space_additive"}:
        return {"Alpha"} if semantic.get("alpha_path") else set()
    if renderer == "layered_bone_blend":
        return handled_bound_pins(
            "BlendWeights_",
            semantic.get("blend_weight_paths"),
            semantic.get("blend_weight_keys"),
        )
    if renderer == "sequence_player":
        handled = set()
        if semantic.get("play_rate_path"):
            handled.add("PlayRate")
        if semantic.get("loop_animation_path"):
            handled.add("bLoopAnimation")
        return handled
    if renderer in {"blendspace_evaluator", "blendspace_player"}:
        paths = semantic.get("input_binding_paths")
        keys = semantic.get("input_binding_keys")
        if renderer == "blendspace_evaluator":
            inputs = semantic.get("input_pins")
            pins = (
                tuple(item.get("name") for item in inputs)
                if isinstance(inputs, list)
                else ()
            )
        else:
            pins = ("X", "Y", "PlayRate")
        if not all(isinstance(pin, str) and pin for pin in pins):
            return set()
        if not isinstance(paths, list) or not isinstance(keys, list) or len(paths) != len(pins) or len(keys) != len(pins):
            return set()
        return {
            pin
            for pin, path, key in zip(pins, paths, keys)
            if isinstance(path, str) and path and isinstance(key, str)
        }
    if renderer == "pose_snapshot":
        return {"Snapshot"} if semantic.get("snapshot_path") else set()
    if renderer == "enum_pose_blend":
        return {"ActiveEnumValue"} if semantic.get("active_enum_path") else set()
    if renderer == "gameplay_tag_blend":
        return {"ActiveTag"} if semantic.get("active_tag_path") else set()
    if renderer == "transition_result":
        return {"bCanEnterTransition"} if semantic.get("condition_path") else set()
    return set()


def unhandled_property_binding_issues(node, renderer, semantic):
    try:
        bindings = property_binding_paths(node)
    except PoseContractError as error:
        return [
            {
                "kind": "source_property_binding_unsupported",
                "source_node": node.get("object_path"),
                "source_class": node.get("class_path"),
                "renderer": renderer,
                "detail": str(error),
            }
        ]
    handled = handled_property_binding_pins(renderer, semantic)
    return [
        {
            "kind": "source_property_binding_unhandled",
            "source_node": node.get("object_path"),
            "source_class": node.get("class_path"),
            "renderer": renderer,
            "source_pin": pin,
            "source_property_path": path,
        }
        for pin, path in sorted(bindings.items())
        if pin not in handled
    ]


def normalized_pin(pin):
    return {
        "index": pin.get("index"),
        "name": pin.get("name"),
        "direction": pin.get("direction"),
        "hidden": pin.get("hidden"),
        "not_connectable": pin.get("not_connectable"),
        "default_value": pin.get("default_value"),
        "default_value_ignored": pin.get("default_value_ignored"),
    }


def normalized_node(node, renderer, semantic):
    result = {
        "source_node": node.get("object_path"),
        "source_class": node.get("class_path"),
        "source_title": node.get("title"),
        "renderer": renderer,
        "position": [node.get("position_x"), node.get("position_y")],
        "pins": [normalized_pin(pin) for pin in node.get("pins", [])],
        "properties": [
            {
                "name": value.get("name"),
                "array_index": value.get("array_index"),
                "exported_text": value.get("exported_text"),
            }
            for value in node.get("properties", [])
        ],
    }
    if semantic is not None:
        result["semantic"] = semantic
    return result


def renderer_semantic_issues(
    node,
    renderer,
    semantic,
    graph_kind,
    transition_renderable,
    fsm_renderable=False,
):
    if (
        renderer == "unsupported"
        or semantic is not None
        or renderer in IR_ONLY_RENDERERS
        or (graph_kind == "transition" and transition_renderable and renderer in TRANSITION_RENDERERS)
        or (renderer in FSM_RENDERERS and fsm_renderable)
    ):
        return []
    return [
        {
            "kind": "source_renderer_unimplemented",
            "source_node": node.get("object_path"),
            "source_class": node.get("class_path"),
            "renderer": renderer,
        }
    ]


def graph_ir(
    graph_contract,
    registry,
    reachable_nodes,
    graph_kind="graph",
    fsm_plan=None,
    state_machine_plans=None,
    animation_assets=None,
    linked_graph_targets=None,
):
    included = set(reachable_nodes)
    for node in graph_contract["source_graph"]["nodes"]:
        if node.get("class_path") == "/Script/UnrealEd.EdGraphNode_Comment":
            included.add(node.get("object_path"))
    nodes = []
    unsupported = []
    transition_renderable = False
    state_machine_plans = state_machine_plans or {}
    animation_by_node = {
        asset.get("source_node"): asset
        for asset in animation_assets or []
        if isinstance(asset.get("source_node"), str)
    }
    linked_graph_targets = linked_graph_targets or {}
    if graph_kind == "transition":
        try:
            render_transition_contract(graph_contract)
            transition_renderable = True
        except (ConversionError, ValueError) as error:
            unsupported.append(
                {
                    "kind": "source_transition_renderer_unsupported",
                    "source_graph": graph_contract["source_graph_suffix"],
                    "detail": str(error),
                }
            )
    for node in graph_contract["source_graph"]["nodes"]:
        if node.get("object_path") not in included:
            continue
        renderer, issue = mapping_for(node, registry)
        if issue is not None:
            unsupported.append(issue)
            renderer = "unsupported"
        semantic, contract_issues = pose_contract(node, renderer)
        if semantic is None and renderer == "reroute":
            semantic = {"renderer": renderer}
        if semantic is None and renderer in {"blendspace_evaluator", "blendspace_player"}:
            asset = animation_by_node.get(node.get("object_path"))
            contract_name = "evaluator" if renderer == "blendspace_evaluator" else "player"
            if (
                not isinstance(asset, dict)
                or asset.get("category") != "blendspace"
                or not isinstance(asset.get("blendspace"), dict)
                or not isinstance(asset.get(contract_name), dict)
            ):
                contract_issues.append(
                    {
                        "kind": "source_pose_renderer_unsupported",
                        "source_node": node.get("object_path"),
                        "source_class": node.get("class_path"),
                        "renderer": renderer,
                        "detail": "blendspace asset binding is unavailable",
                    }
                )
            else:
                semantic = {
                    "renderer": renderer,
                    **asset[contract_name],
                    "blendspace": asset["blendspace"],
                }
        if semantic is None and renderer in POSE_SCALAR_RENDERERS:
            semantic = {"renderer": renderer}
        if semantic is None and renderer == "transition_result":
            semantic = {"renderer": renderer}
        if semantic is None and renderer == "anim_getter":
            try:
                semantic = {"renderer": renderer, **transition_renderer().anim_getter_config(node)}
                bind_animation_getter(semantic, state_machine_plans)
            except ValueError as error:
                contract_issues.append(
                    {
                        "kind": "source_anim_getter_unsupported",
                        "source_node": node.get("object_path"),
                        "source_class": node.get("class_path"),
                        "detail": str(error),
                    }
                )
            except ConversionError as error:
                contract_issues.append(
                    {
                        "kind": "source_anim_getter_target_unavailable",
                        "source_node": node.get("object_path"),
                        "source_class": node.get("class_path"),
                        "detail": str(error),
                    }
                )
        if renderer in {"linked_anim_graph", "linked_anim_layer"} and semantic is not None:
            target = linked_graph_targets.get(node.get("object_path"))
            if not isinstance(target, str) or not target:
                contract_issues.append(
                    {
                        "kind": "source_linked_animation_graph_unresolved",
                        "source_node": node.get("object_path"),
                        "source_class": node.get("class_path"),
                        "renderer": renderer,
                    }
                )
            else:
                semantic["target_graph"] = target
                semantic["external_instance"] = (
                    renderer == "linked_anim_graph" or bool(semantic.get("instance_class"))
                )
        node_fsm_plan = state_machine_plans.get(node.get("object_path"))
        fsm_renderable = fsm_plan is not None or node_fsm_plan is not None
        if semantic is None and renderer in FSM_RENDERERS and fsm_renderable:
            selected = fsm_plan if fsm_plan is not None else node_fsm_plan
            semantic = {"fsm_source_graph": selected["source_graph"]}
        contract_issues.extend(
            resolve_property_bindings(
                node,
                semantic,
                registry,
            )
        )
        contract_issues.extend(unhandled_property_binding_issues(node, renderer, semantic))
        unsupported.extend(contract_issues)
        unsupported.extend(
            renderer_semantic_issues(
                node,
                renderer,
                semantic,
                graph_kind,
                transition_renderable,
                fsm_renderable,
            )
        )
        nodes.append(normalized_node(node, renderer, semantic))
    return {
        "source_graph": graph_contract["source_graph_suffix"],
        "nodes": nodes,
        "links": [
            link
            for link in graph_contract["links"]
            if link["source"]["node_object_path"] in included
            and link["target"]["node_object_path"] in included
        ],
    }, unsupported


def pose_renderer_status(graph_contract, graph_value, animation_assets):
    if not all(
        node.get("renderer") in SUPPORTED_RENDERERS
        or node.get("renderer") == "comment_group"
        for node in graph_value["nodes"]
    ):
        return {"state": "deferred"}
    try:
        render_pose_graph(
            graph_contract,
            graph_value,
            animation_assets,
        )
    except PoseRenderError as error:
        return {"state": "blocked", "detail": str(error)}
    return {"state": "renderable"}


def source_identifier_key(value):
    if not isinstance(value, str) or not value:
        raise ConversionError("source identifier is invalid")
    value = re.sub(r"[^A-Za-z0-9]+", "_", value).strip("_")
    if not value:
        raise ConversionError("source identifier is invalid")
    return source_property_key(value)


def sync_timing(value):
    if not isinstance(value, dict):
        raise ConversionError("source sync group timing is unavailable")
    duration = value.get("duration")
    markers = value.get("markers")
    if (
        not isinstance(duration, (int, float))
        or isinstance(duration, bool)
        or duration <= 0.0
        or not isinstance(markers, list)
        or len(markers) == 1
    ):
        raise ConversionError("source sync group timing is invalid")
    result = []
    previous = -1.0
    for marker in markers:
        if (
            not isinstance(marker, dict)
            or set(marker) != {"name", "time"}
            or not isinstance(marker["name"], str)
            or not marker["name"]
            or not isinstance(marker["time"], (int, float))
            or isinstance(marker["time"], bool)
            or not 0.0 <= marker["time"] <= duration
            or marker["time"] <= previous
        ):
            raise ConversionError("source sync group marker is invalid")
        result.append({"name": marker["name"], "time": float(marker["time"])})
        previous = marker["time"]
    return {"duration": float(duration), "markers": result}


def sync_candidate_timing(node, semantic, assets):
    asset = assets.get(node.get("source_node"))
    if node.get("renderer") == "sequence_player":
        return sync_timing(asset.get("source_timing") if isinstance(asset, dict) else None), None
    if node.get("renderer") == "blendspace_player":
        blendspace = semantic.get("blendspace")
        samples = blendspace.get("samples") if isinstance(blendspace, dict) else None
        if not isinstance(samples, list) or not samples:
            raise ConversionError("source BlendSpace sync timing is unavailable")
        return sync_timing(samples[0].get("source_timing") if isinstance(samples[0], dict) else None), 0
    raise ConversionError("source sync group renderer is unsupported")


def sync_candidate_play_rate(semantic):
    if semantic.get("renderer") == "sequence_player":
        return (
            semantic.get("play_rate_path", ""),
            semantic.get("play_rate_key", ""),
            semantic.get("play_rate"),
        )
    inputs = semantic.get("input_pins")
    paths = semantic.get("input_binding_paths")
    keys = semantic.get("input_binding_keys")
    if isinstance(inputs, list) and isinstance(paths, list) and isinstance(keys, list):
        for input_value, path, key in zip(inputs, paths, keys):
            if isinstance(input_value, dict) and input_value.get("name") == "PlayRate":
                return path, key, semantic.get("configured_play_rate")
    return "", "", semantic.get("configured_play_rate")


def build_sync_group_plans(source_asset, graphs, animation_assets):
    assets = {
        asset.get("source_node"): asset
        for asset in animation_assets
        if isinstance(asset.get("source_node"), str)
    }
    grouped = {}
    for graph in graphs:
        for node in graph["nodes"]:
            semantic = node.get("semantic")
            if node.get("renderer") not in {"sequence_player", "blendspace_player"} or not isinstance(semantic, dict):
                continue
            group = semantic.get("sync_group")
            if group == "None":
                continue
            if not isinstance(group, str) or not group or semantic.get("sync_method") != "SyncGroup":
                continue
            grouped.setdefault(group, {})[node["source_node"]] = (graph, node, semantic)
    asset_name = source_asset.rsplit("/", 1)[-1].split(".", 1)[0]
    plans = []
    issues = []
    supported = set()
    for group, candidates in sorted(grouped.items()):
        rows = []
        failed = False
        for source_node, (graph, node, semantic) in sorted(candidates.items()):
            try:
                timing, basis_sample = sync_candidate_timing(node, semantic, assets)
            except ConversionError as error:
                issues.append(
                    {
                        "kind": "source_sync_group_timing_unsupported",
                        "source_node": source_node,
                        "source_class": node.get("source_class"),
                        "renderer": node.get("renderer"),
                        "detail": str(error),
                    }
                )
                failed = True
                continue
            looped = semantic.get("loop_animation", semantic.get("loop"))
            start = semantic.get("start_position")
            role = semantic.get("sync_group_role")
            path, key, rate = sync_candidate_play_rate(semantic)
            if (
                not isinstance(looped, bool)
                or not isinstance(start, (int, float))
                or isinstance(start, bool)
                or not 0.0 <= start <= 1.0
                or not isinstance(role, str)
                or not isinstance(path, str)
                or not isinstance(key, str)
            ):
                issues.append(
                    {
                        "kind": "source_sync_group_contract_unsupported",
                        "source_node": source_node,
                        "source_class": node.get("source_class"),
                        "renderer": node.get("renderer"),
                    }
                )
                failed = True
                continue
            rows.append(
                {
                    "source_node": source_node,
                    "source_graph": graph["source_graph"],
                    "source_class": node.get("source_class"),
                    "renderer": node.get("renderer"),
                    "source_role": role,
                    "source_method": semantic["sync_method"],
                    "source_start_position": float(start),
                    "source_looped": looped,
                    "source_duration": timing["duration"],
                    "source_markers": timing["markers"],
                    "source_play_rate_path": path,
                    "target_play_rate_key": key,
                    "source_play_rate": rate,
                    "source_phase_basis_sample": basis_sample,
                }
            )
        if failed or not rows:
            continue
        if len({row["source_looped"] for row in rows}) != 1:
            issues.extend(
                {
                    "kind": "source_sync_group_loop_mode_unsupported",
                    "source_node": row["source_node"],
                    "source_class": row["source_class"],
                    "renderer": row["renderer"],
                }
                for row in rows
            )
            continue
        basis = rows[0]
        key = "sync_groups." + source_identifier_key(asset_name) + "." + source_identifier_key(group) + ".phase"
        plan = {
            "source_sync_group": group,
            "target_phase_key": key,
            "target_delta_seconds_key": "animation.delta_seconds",
            "phase_basis_source_node": basis["source_node"],
            "phase_duration": basis["source_duration"],
            "phase_markers": basis["source_markers"],
            "phase_looped": basis["source_looped"],
            "candidates": rows,
            "status": "graph_owned",
        }
        for row in rows:
            _, node, semantic = candidates[row["source_node"]]
            semantic["sync_phase_key"] = key
            semantic["sync_phase_duration"] = plan["phase_duration"]
            semantic["sync_phase_markers"] = plan["phase_markers"]
            semantic["sync_phase_looped"] = plan["phase_looped"]
            semantic["delta_seconds_key"] = plan["target_delta_seconds_key"]
            supported.add(node["source_node"])
        plans.append(plan)
    return plans, supported, issues


def resolve_control_rig_program(semantic, content_root, mount, source_reader):
    if semantic.get("rig_input_binding_path") != "GetControlRigInput":
        raise ConversionError("control rig input function is unsupported")
    source_path = semantic["rig_asset"]
    uasset = uasset_for_generated_anim_blueprint(source_path, content_root, mount)
    source, _ = read_uasset_source(uasset, content_root, mount, source_reader, 300, 150)
    contract = compile_rig_contract(source)
    target = compile_rig_target(contract)
    external = [row for row in contract["program"]["memories"]["External"] if row["name"] == "RigInput"]
    if len(external) != 1 or external[0]["cpp_type"] != "FAlsControlRigInput":
        raise ConversionError("control rig input type does not match its source function")
    bindings = [{"external": external[0]["name"], "path": [index],
        "property": source_property_key("ControlRigInput." + field.split(":")[0])}
        for index, field in enumerate(RIG_STRUCT_FIELDS["FAlsControlRigInput"].split())]
    return {"definition": target, "input_bindings": bindings}


def build_ir(
    source,
    root_graph,
    registry,
    asset_root,
    source_content_root=None,
    source_mount=None,
    source_reader=None,
    external_linked_graph_targets=None,
    output_namespace="",
):
    output_namespace = asset_namespace(output_namespace)
    source_reachability = traverse(source, root_graph)
    graphs = []
    unsupported = list(source_reachability["issues"])
    lifecycle_dispatch = None
    try:
        lifecycle_dispatch = resolve_lifecycle(
            source,
            source_reachability["function_dependencies"],
        )
    except LifecycleRenderError as error:
        unsupported.append(
            {
                "kind": "source_lifecycle_dispatch_unsupported",
                "detail": str(error),
            }
        )
    try:
        assets = resolve_graph_animation_assets(
            asset_root,
            source,
            source_content_root,
            source_mount,
            source_reader,
        )
    except AssetResolutionError as error:
        unsupported.append(
            {
                "kind": "unresolved_source_animation_asset",
                "message": str(error),
            }
        )
        assets = []
    fsm_plans = {}
    for entry in source_reachability["graphs"]:
        if entry["kind"] != "state_machine":
            continue
        try:
            plan = render_fsm_plan(
                source,
                entry["source_graph"],
                source_content_root,
                source_mount,
                source_reader,
            )
            fsm_plans[entry["source_graph"]] = plan
            unsupported.extend(plan.get("unsupported", []))
        except FsmRenderError as error:
            unsupported.append(
                {
                    "kind": "source_fsm_renderer_unsupported",
                    "source_graph": entry["source_graph"],
                    "detail": str(error),
                }
            )
    try:
        state_machine_plans = animation_getter_machine_plans(source, fsm_plans)
    except ConversionError as error:
        unsupported.append(
            {
                "kind": "source_anim_getter_target_unavailable",
                "detail": str(error),
            }
        )
        state_machine_plans = {}
    linked_graph_targets = {
        value["source_node"]: graph_path(
            source["asset_path"], value["source_graph"], output_namespace
        )
        for value in source_reachability.get("linked_layer_graphs", [])
        if isinstance(value, dict)
        and isinstance(value.get("source_node"), str)
        and isinstance(value.get("source_graph"), str)
    }
    linked_graph_targets.update(external_linked_graph_targets or {})
    for entry in source_reachability["graphs"]:
        value = contract(source, entry["source_graph"])
        fsm_plan = fsm_plans.get(entry["source_graph"])
        graph_value, graph_unsupported = graph_ir(
            value,
            registry,
            entry["reachable_nodes"],
            entry["kind"],
            fsm_plan,
            state_machine_plans,
            assets,
            linked_graph_targets,
        )
        for node in graph_value["nodes"]:
            if node.get("renderer") == "control_rig" and node.get("semantic") is not None:
                try:
                    node["semantic"].update(resolve_control_rig_program(
                        node["semantic"], source_content_root, source_mount, source_reader))
                except (ConversionError, ControlRigContractError, KeyError, TypeError, AttributeError) as error:
                    graph_unsupported.append({"kind": "source_control_rig_program_unsupported",
                        "source_node": node["source_node"], "detail": str(error)})
        if entry["kind"] == "transition":
            try:
                graph_value["gameplay_tag_providers"] = transition_renderer().gameplay_tag_providers(value)
            except ValueError as error:
                unsupported.append(
                    {
                        "kind": "source_gameplay_tag_adapter_unsupported",
                        "source_graph": entry["source_graph"],
                        "detail": str(error),
                    }
                )
        if entry["kind"] == "state_machine":
            try:
                graph_value["fsm_contract"] = fsm_contract(source, entry["source_graph"])
            except FsmContractError as error:
                unsupported.append(
                    {
                        "kind": "source_fsm_contract_unsupported",
                        "source_graph": entry["source_graph"],
                        "detail": str(error),
                    }
                )
            if fsm_plan is not None:
                graph_value["fsm_renderer"] = fsm_plan
        graphs.append({**entry, **graph_value})
        unsupported.extend(graph_unsupported)
    sync_group_plans, supported_sync_nodes, sync_issues = build_sync_group_plans(
        source["asset_path"],
        graphs,
        assets,
    )
    unsupported.extend(sync_issues)
    unsupported = [
        issue
        for issue in unsupported
        if not (
            issue.get("kind") == "source_sync_group_unsupported"
            and issue.get("source_node") in supported_sync_nodes
        )
    ]
    for graph in graphs:
        if graph["source_graph"] == root_graph and sync_group_plans:
            for node in graph["nodes"]:
                if node.get("renderer") in {"root_output", "state_result"}:
                    node["semantic"] = {**(node.get("semantic") or {}), "synchronize_groups": True}
        if graph["kind"] == "graph":
            graph["pose_renderer"] = pose_renderer_status(
                contract(source, graph["source_graph"]),
                graph,
                assets,
            )
    handled_external_nodes = {
        node["source_node"]
        for graph in graphs
        for node in graph["nodes"]
        if node.get("renderer") == "control_rig"
    }
    external_dependencies = [
        dependency
        for dependency in source_reachability["external_dependencies"]
        if dependency.get("source_node") not in linked_graph_targets
        and dependency.get("source_node") not in handled_external_nodes
    ]
    for dependency in external_dependencies:
        unsupported.append({"kind": "source_external_dependency_unresolved", **dependency})
    handled_lifecycle_nodes = set(
        lifecycle_dispatch["handled_nodes"] if lifecycle_dispatch is not None else []
    )
    unsupported = [
        issue
        for issue in unsupported
        if not (
            issue.get("source_node") in handled_lifecycle_nodes
            and issue.get("kind")
            in {
                "source_lifecycle_unsupported",
                "unregistered_source_node",
                "unregistered_source_function",
            }
        )
    ]
    return {
        "format": IR_FORMAT,
        "source_asset": source["asset_path"],
        "root_source_graph": root_graph,
        "graphs": graphs,
        "animation_assets": assets,
        "sync_group_plans": sync_group_plans,
        "function_dependencies": source_reachability["function_dependencies"],
        "external_dependencies": external_dependencies,
        "lifecycle_dispatch": lifecycle_dispatch,
    }, unsupported


def build_report(source_identity, ir, unsupported):
    return {
        "format": REPORT_FORMAT,
        "source": {**source_identity, "asset": ir["source_asset"]},
        "function_dependencies": ir["function_dependencies"],
        "unsupported": unsupported,
        "pass": not unsupported,
    }


def integration_blockers(report, source_node):
    return sorted(
        {
            issue["kind"]
            for issue in report.get("unsupported", [])
            if isinstance(issue, dict)
            and issue.get("source_node") == source_node
            and isinstance(issue.get("kind"), str)
        }
    )


def integration_provider_by_node(ir):
    lifecycle = ir.get("lifecycle_dispatch")
    dispatches = lifecycle.get("dispatches", []) if isinstance(lifecycle, dict) else []
    return {
        dispatch["source_node"]: dispatch["provider"]
        for dispatch in dispatches
        if isinstance(dispatch, dict)
        and isinstance(dispatch.get("source_node"), str)
        and isinstance(dispatch.get("provider"), str)
    }


def integration_sync_phase_providers(ir):
    plans = ir.get("sync_group_plans", [])
    if not isinstance(plans, list):
        return []
    rows = [
        {
            **plan,
            "target_provider": "als_ext::SourcePosePassthroughNode",
            "status": "mapped",
        }
        for plan in plans
        if isinstance(plan, dict)
        and all(
            isinstance(plan.get(field), str) and plan[field]
            for field in (
                "source_sync_group",
                "target_phase_key",
                "target_delta_seconds_key",
                "phase_basis_source_node",
                "status",
            )
        )
        and isinstance(plan.get("phase_duration"), (int, float))
        and isinstance(plan.get("phase_markers"), list)
        and isinstance(plan.get("phase_looped"), bool)
        and isinstance(plan.get("candidates"), list)
    ]
    return sorted(rows, key=lambda item: item["target_phase_key"])


def integration_runtime_inputs(ir):
    rows = []
    for graph in ir.get("graphs", []):
        for node in graph.get("nodes", []):
            if not isinstance(node, dict) or not isinstance(node.get("semantic"), dict):
                continue
            bindings = node["semantic"].get("resolved_property_bindings", [])
            if not isinstance(bindings, list):
                continue
            for binding in bindings:
                if (
                    not isinstance(binding, dict)
                    or not all(
                        isinstance(binding.get(field), str) and binding[field]
                        for field in ("source_pin", "source_property_path", "target_key")
                    )
                ):
                    continue
                provider = runtime_provider("runtime_inputs", binding["target_key"])
                rows.append(
                    {
                        "source_node": node.get("source_node"),
                        "renderer": node.get("renderer"),
                        **binding,
                        "target_provider": provider,
                        "status": "mapped" if provider else "requires_target_provider",
                    }
                )
    return sorted(rows, key=lambda item: json.dumps(item, ensure_ascii=False, sort_keys=True))


def integration_anim_getter_providers(ir):
    rows = []
    for graph in ir.get("graphs", []):
        for node in graph.get("nodes", []):
            semantic = node.get("semantic") if isinstance(node, dict) else None
            if node.get("renderer") != "anim_getter" or not isinstance(semantic, dict):
                continue
            if not all(
                isinstance(semantic.get(field), str) and semantic[field]
                for field in ("source_function", "source_machine", "target_key")
            ):
                continue
            state = semantic.get("source_state", "")
            if not isinstance(state, str):
                continue
            target_fsm_node = semantic.get("target_fsm_node", "")
            target_state_id = semantic.get("target_state_id", "")
            target_fsm_node = target_fsm_node if isinstance(target_fsm_node, str) else ""
            target_state_id = target_state_id if isinstance(target_state_id, str) else ""
            rows.append(
                {
                    "source_node": node.get("source_node"),
                    "source_function": semantic["source_function"],
                    "source_machine": semantic["source_machine"],
                    "source_state": state,
                    "target_key": semantic["target_key"],
                    "target_fsm_node": target_fsm_node,
                    "target_state_id": target_state_id,
                    "provider_api": "als_ext::SourceAnimationStateGetterNode",
                    "status": "mapped"
                    if target_fsm_node
                    and (
                        semantic["source_function"] == "GetInstanceMachineWeight"
                        or target_state_id
                    )
                    else "requires_animation_state_provider",
                }
            )
    return sorted(rows, key=lambda item: item["source_node"])


def integration_slot_providers(ir):
    rows = []
    for graph in ir.get("graphs", []):
        for node in graph.get("nodes", []):
            if not isinstance(node, dict) or node.get("renderer") != "slot":
                continue
            semantic = node.get("semantic")
            if not isinstance(semantic, dict) or not all(
                isinstance(semantic.get(field), str) and semantic[field]
                for field in ("slot_name", "group_name", "pose_key", "weight_key", "layer_weight_key", "mesh_space_additive_key")
            ):
                continue
            provider = runtime_provider("slot_providers", semantic["slot_name"])
            rows.append(
                {
                    "source_node": node.get("source_node"),
                    "slot_name": semantic["slot_name"],
                    "group_name": semantic["group_name"],
                    "pose_key": semantic["pose_key"],
                    "weight_key": semantic["weight_key"],
                    "layer_weight_key": semantic["layer_weight_key"],
                    "mesh_space_additive_key": semantic["mesh_space_additive_key"],
                    "target_provider": provider,
                    "status": "mapped" if provider else "requires_target_provider",
                }
            )
    return sorted(rows, key=lambda item: item["source_node"])


def integration_gameplay_tag_providers(ir):
    rows = []
    for graph in ir.get("graphs", []):
        providers = graph.get("gameplay_tag_providers", []) if isinstance(graph, dict) else []
        if not isinstance(providers, list):
            continue
        for provider in providers:
            if not isinstance(provider, dict) or not all(
                isinstance(provider.get(field), str) and provider[field]
                for field in ("source_node", "source_property_path", "target_key", "operation", "provider_api", "player_entity")
            ):
                continue
            target_provider = runtime_provider("gameplay_tag_providers", provider["target_key"])
            rows.append(
                {
                    **provider,
                    "target_provider": target_provider,
                    "status": "mapped"
                    if target_provider
                    else "requires_target_provider",
                }
            )
        for node in graph.get("nodes", []):
            if not isinstance(node, dict):
                continue
            semantic = node.get("semantic")
            if (
                node.get("renderer") != "gameplay_tag_blend"
                or not isinstance(semantic, dict)
                or not isinstance(semantic.get("active_tag_path"), str)
                or not semantic["active_tag_path"]
                or not isinstance(semantic.get("active_key"), str)
                or not semantic["active_key"]
            ):
                continue
            target_provider = runtime_provider("gameplay_tag_providers", semantic["active_key"])
            rows.append(
                {
                    "source_node": node.get("source_node"),
                    "source_property_path": semantic["active_tag_path"],
                    "target_key": semantic["active_key"],
                    "operation": "select_exact_child",
                    "expected_tag": "",
                    "provider_api": "als_ext::set_gameplay_tag_input",
                    "player_entity": "AnimationGraphPlayer entity",
                    "target_provider": target_provider,
                    "status": "mapped"
                    if target_provider
                    else "requires_target_provider",
                }
            )
    return sorted(rows, key=lambda item: item["source_node"])


def integration_control_rig_providers(ir):
    rows = []
    for graph in ir.get("graphs", []):
        for node in graph.get("nodes", []):
            semantic = node.get("semantic") if isinstance(node, dict) else None
            if node.get("renderer") != "control_rig" or not isinstance(semantic, dict):
                continue
            if not all(
                isinstance(semantic.get(field), str) and semantic[field]
                for field in ("rig_class", "rig_input_binding_pin", "rig_input_binding_path")
            ):
                continue
            rows.append(
                {
                    "source_node": node.get("source_node"),
                    "source_rig_class": semantic["rig_class"],
                    "source_pin": semantic["rig_input_binding_pin"],
                    "source_function": semantic["rig_input_binding_path"],
                    "target_provider": "crate::control_rig_apply::ControlRigApplyPlugin (AlsControlRigInput)",
                    "status": "mapped",
                }
            )
    return sorted(rows, key=lambda item: item["source_node"])


def integration_is_ready(report, value):
    if not report.get("pass"):
        return False
    for field in (
        "external_runtime_functions",
        "animation_blueprint_functions",
        "sync_phase_providers",
        "runtime_inputs",
        "animation_state_getter_providers",
        "slot_providers",
        "gameplay_tag_providers",
        "control_rig_providers",
    ):
        if any(item.get("status") != "mapped" for item in value[field]):
            return False
    return not any(
        value[field]
        for field in (
            "unhandled_property_bindings",
            "external_graph_dependencies",
            "other_blockers",
        )
    )


def build_integration_report(ir, report):
    providers = integration_provider_by_node(ir)
    external_functions = []
    animation_blueprint_functions = []
    for dependency in ir.get("function_dependencies", []):
        if not isinstance(dependency, dict):
            continue
        source_node = dependency.get("source_node")
        function = dependency.get("source_function")
        call_site = dependency.get("call_site")
        resolution = dependency.get("resolution")
        if not all(isinstance(value, str) and value for value in (source_node, function, call_site, resolution)):
            continue
        provider = providers.get(source_node)
        row = {
            "source_node": source_node,
            "source_function": function,
            "call_site": call_site,
            "target_provider": provider,
            "blockers": integration_blockers(report, source_node),
        }
        if resolution == "external":
            external_functions.append(
                {
                    **row,
                    "status": "mapped" if provider is not None else "requires_target_implementation",
                }
            )
        elif resolution == "graph":
            animation_blueprint_functions.append(
                {
                    **row,
                    "source_graph": dependency.get("source_graph"),
                    "status": "mapped" if provider is not None else "requires_function_graph_lowering",
                }
            )
    property_bindings = [
        {
            "source_node": issue.get("source_node"),
            "source_class": issue.get("source_class"),
            "renderer": issue.get("renderer"),
            "source_pin": issue.get("source_pin"),
            "source_property_path": issue.get("source_property_path"),
            "status": "requires_target_provider",
        }
        for issue in report.get("unsupported", [])
        if isinstance(issue, dict) and issue.get("kind") == "source_property_binding_unhandled"
    ]
    property_bindings.sort(key=lambda item: json.dumps(item, ensure_ascii=False, sort_keys=True))
    external_dependencies = [
        dependency
        for dependency in ir.get("external_dependencies", [])
        if isinstance(dependency, dict)
    ]
    external_dependencies.sort(key=lambda item: json.dumps(item, ensure_ascii=False, sort_keys=True))
    external_functions.sort(key=lambda item: item["source_node"])
    animation_blueprint_functions.sort(key=lambda item: item["source_node"])
    value = {
        "format": INTEGRATION_REPORT_FORMAT,
        "source": report.get("source", {}),
        "conversion_status": "blocked",
        "handoff_action": "repair_conversion_blockers",
        "external_runtime_functions": external_functions,
        "animation_blueprint_functions": animation_blueprint_functions,
        "sync_phase_providers": integration_sync_phase_providers(ir),
        "runtime_inputs": integration_runtime_inputs(ir),
        "animation_state_getter_providers": integration_anim_getter_providers(ir),
        "slot_providers": integration_slot_providers(ir),
        "gameplay_tag_providers": integration_gameplay_tag_providers(ir),
        "control_rig_providers": integration_control_rig_providers(ir),
        "unhandled_property_bindings": property_bindings,
        "external_graph_dependencies": external_dependencies,
        "other_blockers": [
            issue
            for issue in report.get("unsupported", [])
            if isinstance(issue, dict)
            and issue.get("kind") != "source_property_binding_unhandled"
        ],
    }
    if integration_is_ready(report, value):
        value["conversion_status"] = "ready"
        value["handoff_action"] = "integrate_generated_assets"
    return value


def integration_tree_results(result, seen=None):
    seen = set() if seen is None else seen
    source = result.get("source", {})
    asset = source.get("asset_path") if isinstance(source, dict) else None
    if not isinstance(asset, str) or not asset or asset in seen:
        return []
    seen.add(asset)
    values = [result]
    for child in result.get("children", []):
        values.extend(integration_tree_results(child, seen))
    return values


def merge_integration_rows(reports, field):
    rows = {}
    for report in reports:
        for row in report.get(field, []):
            if isinstance(row, dict):
                rows[json.dumps(row, ensure_ascii=False, sort_keys=True)] = row
    return [rows[key] for key in sorted(rows)]


def build_tree_integration_report(result):
    results = integration_tree_results(result)
    reports = [build_integration_report(item["ir"], item["report"]) for item in results]
    if not reports:
        raise ConversionError("integration source tree is unavailable")
    root = reports[0]
    list_fields = (
        "external_runtime_functions",
        "animation_blueprint_functions",
        "sync_phase_providers",
        "runtime_inputs",
        "animation_state_getter_providers",
        "slot_providers",
        "gameplay_tag_providers",
        "control_rig_providers",
        "unhandled_property_bindings",
        "external_graph_dependencies",
        "other_blockers",
    )
    merged = {
        **root,
        "conversion_status": "ready"
        if all(report["conversion_status"] == "ready" for report in reports)
        else "blocked",
        "handoff_action": "integrate_generated_assets"
        if all(report["conversion_status"] == "ready" for report in reports)
        else "repair_conversion_blockers",
        "source_closure": [
            {
                "source_asset": item["source"]["asset_path"],
                "conversion_status": report["conversion_status"],
            }
            for item, report in zip(results, reports, strict=True)
        ],
    }
    for field in list_fields:
        merged[field] = merge_integration_rows(reports, field)
    return merged


def integration_report_markdown(value):
    if value["conversion_status"] == "blocked":
        return (
            "# Animation graph conversion blocked\n\n"
            "Do not integrate generated RON files.\n"
            "Repair every conversion blocker and rerun until Conversion status: ready.\n"
        )
    lines = [
        "# Animation graph integration report",
        "",
        "Conversion status: " + value["conversion_status"],
        "",
        "## Source closure",
        "",
    ]
    closure = value.get("source_closure", [])
    if closure:
        lines.extend(
            "- `" + item["source_asset"] + "`: " + item["conversion_status"]
            for item in closure
        )
    else:
        lines.append("- Root source only")
    lines.extend([
        "",
        "## External runtime functions",
        "",
    ])
    functions = value["external_runtime_functions"]
    if functions:
        lines.extend(
            "- `"
            + item["source_function"]
            + "` at `"
            + item["source_node"]
            + "` ("
            + item["call_site"]
            + "): "
            + item["status"]
            + (" via `" + item["target_provider"] + "`" if item["target_provider"] else "")
            for item in functions
        )
    else:
        lines.append("- None")
    lines.extend(["", "## Control Rig providers", ""])
    control_rig_providers = value["control_rig_providers"]
    if control_rig_providers:
        lines.extend(
            "- `"
            + item["source_rig_class"]
            + "` / `"
            + item["source_function"]
            + "` at `"
            + str(item["source_node"])
            + "` pin `"
            + item["source_pin"]
            + "` → `"
            + item["target_provider"]
            + "`: "
            + item["status"]
            for item in control_rig_providers
        )
    else:
        lines.append("- None")
    lines.extend(["", "## Slot providers", ""])
    slot_providers = value["slot_providers"]
    if slot_providers:
        lines.extend(
            "- `"
            + item["slot_name"]
            + "` / `"
            + item["group_name"]
            + "` at `"
            + str(item["source_node"])
            + "`: pose `"
            + item["pose_key"]
            + "`, weight `"
            + item["weight_key"]
            + "`, layer weight `"
            + item["layer_weight_key"]
            + "`: "
            + item["status"]
            for item in slot_providers
        )
    else:
        lines.append("- None")
    lines.extend(["", "## Gameplay tag providers", ""])
    tag_providers = value["gameplay_tag_providers"]
    if tag_providers:
        lines.extend(
            "- `"
            + item["source_property_path"]
            + "` at `"
            + item["source_node"]
            + "` → `"
            + item["target_key"]
            + "` via `"
            + item["provider_api"]
            + "` on `"
            + item["player_entity"]
            + "`: "
            + item["status"]
            for item in tag_providers
        )
    else:
        lines.append("- None")
    lines.extend(["", "## Animation state getter providers", ""])
    getter_providers = value["animation_state_getter_providers"]
    if getter_providers:
        lines.extend(
            "- `"
            + item["source_function"]
            + "` at `"
            + str(item["source_node"])
            + "` → `"
            + item["target_key"]
            + "`: "
            + item["status"]
            for item in getter_providers
        )
    else:
        lines.append("- None")
    lines.extend(["", "## Runtime property inputs", ""])
    runtime_inputs = value["runtime_inputs"]
    if runtime_inputs:
        lines.extend(
            "- `"
            + item["source_property_path"]
            + "` at `"
            + str(item["source_node"])
            + "` pin `"
            + item["source_pin"]
            + "` → `"
            + item["target_key"]
            + "`: "
            + item["status"]
            for item in runtime_inputs
        )
    else:
        lines.append("- None")
    lines.extend(["", "## Runtime sync phase providers", ""])
    phase_providers = value["sync_phase_providers"]
    if phase_providers:
        lines.extend(
            "- `"
            + item["source_sync_group"]
            + "`: phase `"
            + item["target_phase_key"]
            + "`, delta `"
            + item["target_delta_seconds_key"]
            + "`, basis `"
            + item["phase_basis_source_node"]
            + "` ("
            + str(len(item["candidates"]))
            + " source candidates): "
            + item["status"]
            for item in phase_providers
        )
    else:
        lines.append("- None")
    lines.extend(["", "## Unhandled property bindings", ""])
    bindings = value["unhandled_property_bindings"]
    if bindings:
        lines.extend(
            "- `"
            + item["source_property_path"]
            + "` at `"
            + item["source_node"]
            + "` pin `"
            + item["source_pin"]
            + "`: "
            + item["status"]
            for item in bindings
        )
    else:
        lines.append("- None")
    lines.extend(["", "## Animation Blueprint function graphs", ""])
    functions = value["animation_blueprint_functions"]
    if functions:
        lines.extend(
            "- `"
            + item["source_function"]
            + "` at `"
            + item["source_node"]
            + "`: "
            + item["status"]
            for item in functions
        )
    else:
        lines.append("- None")
    lines.extend(["", "## External graph dependencies", ""])
    dependencies = value["external_graph_dependencies"]
    if dependencies:
        lines.extend(
            "- `"
            + str(item.get("kind", "unknown"))
            + "` at `"
            + str(item.get("source_node", "unknown"))
            + "`: requires target integration"
            for item in dependencies
        )
    else:
        lines.append("- None")
    lines.extend(["", "## Other conversion blockers", ""])
    blockers = value["other_blockers"]
    if blockers:
        lines.extend(
            "- `"
            + str(item.get("kind", "unknown"))
            + "` at `"
            + str(item.get("source_node", "unknown"))
            + "`"
            for item in blockers
        )
    else:
        lines.append("- None")
    return "\n".join(lines) + "\n"


def write_integration_report(output_root, value):
    write_json(output_root / "integration.report.json", value)
    (output_root / "integration.report.md").write_text(
        integration_report_markdown(value), encoding="utf-8"
    )


def integration_report_drift(output_root, value):
    expected = {
        "integration.report.json": json.dumps(value, ensure_ascii=False, indent=2) + "\n",
        "integration.report.md": integration_report_markdown(value),
    }
    return [
        {"kind": "generated_output_drift", "path": name}
        for name, text in expected.items()
        if not (output_root / name).is_file()
        or (output_root / name).read_text(encoding="utf-8") != text
    ]


def expected_outputs(
    source,
    source_identity,
    root_graph,
    registry,
    asset_root,
    source_content_root=None,
    source_mount=None,
    source_reader=None,
    external_linked_graph_targets=None,
    output_namespace="",
):
    ir, unsupported = build_ir(
        source,
        root_graph,
        registry,
        asset_root,
        source_content_root,
        source_mount,
        source_reader,
        external_linked_graph_targets,
        output_namespace,
    )
    report = build_report(source_identity, ir, unsupported)
    return ir, report


def source_nodes(source):
    values = {}
    for graph in source.get("graphs", []):
        for node in graph.get("nodes", []):
            path = node.get("object_path")
            if isinstance(path, str) and path:
                values[path] = node
    return values


def uasset_for_generated_anim_blueprint(asset, content_root, mount):
    prefix = mount.rstrip("/") + "/"
    if not isinstance(asset, str) or not asset.startswith(prefix):
        raise ConversionError("linked animation generated class is outside the configured mount")
    package, separator, name = asset.rpartition(".")
    if not separator or package.rsplit("/", 1)[-1] != name:
        raise ConversionError("linked animation generated class is invalid")
    uasset = content_root / (package[len(prefix) :] + ".uasset")
    if not uasset.is_file():
        raise ConversionError("linked animation source uasset is unavailable")
    return uasset


def linked_instance_specs(source, root_graph):
    reachability = traverse(source, root_graph)
    nodes = source_nodes(source)
    values = []
    issues = []
    for dependency in reachability.get("external_dependencies", []):
        kind = dependency.get("kind") if isinstance(dependency, dict) else None
        node_path = dependency.get("source_node") if isinstance(dependency, dict) else None
        if kind not in {"linked_anim_graph", "linked_anim_layer"} or node_path not in nodes:
            continue
        node = nodes[node_path]
        try:
            semantic, contract_issues = (
                linked_anim_graph_contract(node)
                if kind == "linked_anim_graph"
                else linked_anim_layer_contract(node)
            )
        except (PoseContractError, ValueError) as error:
            issues.append(
                {
                    "kind": "source_linked_animation_graph_unsupported",
                    "source_node": node_path,
                    "source_class": node.get("class_path"),
                    "detail": str(error),
                }
            )
            continue
        if contract_issues or not semantic.get("instance_class"):
            issues.append(
                {
                    "kind": "source_linked_animation_graph_unsupported",
                    "source_node": node_path,
                    "source_class": node.get("class_path"),
                    "detail": (
                        contract_issues[0].get("detail", contract_issues[0].get("kind"))
                        if contract_issues
                        else "linked animation instance class is unavailable"
                    ),
                }
            )
            continue
        values.append(
            {
                "source_node": node_path,
                "kind": kind,
                "source_asset": semantic["instance_class"],
            }
        )
    return values, issues


def merge_generated_files(destination, source):
    for path, text in source.items():
        existing = destination.get(path)
        if existing is not None and existing != text:
            raise ConversionError("linked animation closures generated conflicting target paths")
        destination[path] = text


def linked_manifest_path(source_asset):
    return "linked_manifests/" + source_identifier_key(source_asset) + ".json"


def collect_linked_sources(result, output_namespace):
    values = []
    for child in result["children"]:
        values.append(
            {
                "source_asset": child["source"]["asset_path"],
                "root_graph": graph_path(
                    child["source"]["asset_path"], "AnimGraph", output_namespace
                ),
                "links": child["links"],
            }
        )
        values.extend(collect_linked_sources(child, output_namespace))
    return values


def collect_tree_files(result, root=True):
    files = {}
    for child in result["children"]:
        merge_generated_files(files, collect_tree_files(child, False))
    own_files = dict(result["files"])
    manifest = own_files.pop("materialization.manifest.json", None)
    if manifest is not None:
        path = "materialization.manifest.json" if root else linked_manifest_path(result["source"]["asset_path"])
        own_files[path] = manifest
    merge_generated_files(files, own_files)
    return files


def convert_source_tree(
    source,
    source_identity,
    root_graph,
    registry,
    asset_root,
    content_root,
    mount,
    source_reader,
    timeout_seconds,
    no_progress_seconds,
    output_namespace,
    cache,
    active_assets,
):
    source_asset = source["asset_path"]
    if source_asset in active_assets:
        raise ConversionError("linked animation graph cycle is unsupported")
    if source_asset in cache:
        return cache[source_asset]
    active_assets.add(source_asset)
    links, link_issues = linked_instance_specs(source, root_graph)
    children = []
    external_targets = {}
    for link in links:
        try:
            child_uasset = uasset_for_generated_anim_blueprint(
                link["source_asset"], content_root, mount
            )
            child_source, child_identity = read_uasset_source(
                child_uasset,
                content_root,
                mount,
                source_reader,
                timeout_seconds,
                no_progress_seconds,
            )
            child = convert_source_tree(
                child_source,
                child_identity,
                "AnimGraph",
                registry,
                asset_root,
                content_root,
                mount,
                source_reader,
                timeout_seconds,
                no_progress_seconds,
                output_namespace,
                cache,
                active_assets,
            )
        except ConversionError as error:
            link_issues.append(
                {
                    "kind": "source_linked_animation_graph_unsupported",
                    "source_node": link["source_node"],
                    "source_asset": link["source_asset"],
                    "detail": str(error),
                }
            )
            continue
        children.append({**child, "links": [link]})
        external_targets[link["source_node"]] = graph_path(
            child_source["asset_path"], "AnimGraph", output_namespace
        )
    ir, report = expected_outputs(
        source,
        source_identity,
        root_graph,
        registry,
        asset_root,
        content_root,
        mount,
        source_reader,
        external_targets,
        output_namespace,
    )
    for child in children:
        if child["report"].get("pass"):
            continue
        link_issues.append(
            {
                "kind": "source_linked_animation_graph_child_blocked",
                "source_asset": child["source"]["asset_path"],
                "detail": child["report"].get("unsupported", []),
            }
        )
    if link_issues:
        report["unsupported"].extend(link_issues)
        report["pass"] = False
    result = {
        "source": source,
        "source_identity": source_identity,
        "ir": ir,
        "report": report,
        "children": children,
        "links": [],
        "files": {},
    }
    if report["pass"]:
        result["files"] = materialize_files(
            source,
            ir,
            external_targets.values(),
            output_namespace,
        )
    active_assets.remove(source_asset)
    cache[source_asset] = result
    return result


def materialize_additive_reference_deltas(
    files, content_root, mount, asset_root, source_reader
):
    source_root = (Path(content_root) / mount.strip("/")).resolve()
    if not source_root.is_dir():
        raise ConversionError("source additive asset root is unavailable")
    target_paths = additive_asset_paths(asset_root)
    with tempfile.TemporaryDirectory() as directory:
        graph_root = Path(directory)
        for name, value in files.items():
            if not name.endswith(".ron"):
                continue
            path = graph_root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(value, encoding="utf-8")
        source_assets = source_animation_assets(
            source_root,
            referenced_animation_names(graph_root),
            f"{mount.rstrip('/')}/{source_root.name}",
            source_reader,
        )
        grouped = {}
        for row in additive_graph_clip_uses(graph_root):
            metadata = source_assets.get(row["clip"])
            if metadata is None or metadata["additive_type"] == "AAT_None":
                continue
            reference = source_assets.get(metadata["reference_asset"])
            if reference is None:
                raise ConversionError(
                    f"source additive reference is unavailable: {row['clip']}"
                )
            gltf_paths = list(asset_root.rglob(f"{row['clip']}.gltf"))
            summary = raw_scale_summary(gltf_paths[0]) if len(gltf_paths) == 1 else {
                "scale_keys": 0,
                "scale_min": "Missing",
                "scale_max": "Missing",
                "unit": False,
            }
            merged = metadata | {
                "reference_sequence_length": reference["sequence_length"],
                "reference_sample_rate": reference["sample_rate"],
                "reference_frame_count": reference["frame_count"],
            }
            grouped.setdefault(row["path"], []).append(row | {"metadata": merged} | summary)
        rows = []
        for path, graph_rows in grouped.items():
            rows.extend(
                row | {"kind": "clip", "requires_single_pose_consumer": True}
                for row in patch_additive_graph(
                    path, graph_rows[0]["graph"], graph_rows, target_paths, True
                )
            )
        for path in sorted(graph_root.rglob("*.ron")):
            rows.extend(
                patch_sequence_player_references(
                    path,
                    path.relative_to(graph_root).as_posix(),
                    source_assets,
                    target_paths,
                    asset_root,
                    True,
                )
            )
        if any(
            not row["converted"]
            or not row["unit"]
            or (row["requires_single_pose_consumer"] and row["pose_consumers"] != 1)
            for row in rows
        ):
            raise ConversionError("source additive reference delta materialization is incomplete")
        materialized = dict(files)
        for name in materialized:
            if name.endswith(".ron"):
                materialized[name] = (graph_root / name).read_text(encoding="utf-8")
    clips = [row for row in rows if row["kind"] == "clip"]
    players = [row for row in rows if row["kind"] == "sequence_player"]
    return materialized, {
        "additive_clip_uses": len(clips),
        "additive_sequence_player_uses": len(players),
        "converted_inputs": sum(row["converted"] for row in rows),
        "unit_raw_scale_inputs": sum(row["unit"] for row in rows),
        "single_pose_consumers": sum(row["pose_consumers"] == 1 for row in clips),
    }


def conversion_tree_outputs(
    source,
    source_identity,
    root_graph,
    registry,
    asset_root,
    content_root,
    mount,
    source_reader,
    timeout_seconds,
    no_progress_seconds,
    output_namespace="",
):
    output_namespace = asset_namespace(output_namespace)
    result = convert_source_tree(
        source,
        source_identity,
        root_graph,
        registry,
        asset_root,
        content_root,
        mount,
        source_reader,
        timeout_seconds,
        no_progress_seconds,
        output_namespace,
        {},
        set(),
    )
    linked_sources = collect_linked_sources(result, output_namespace)
    result["ir"]["linked_sources"] = linked_sources
    result["report"]["linked_sources"] = linked_sources
    files = collect_tree_files(result)
    if result["report"]["pass"]:
        files, additive_deltas = materialize_additive_reference_deltas(
            files, content_root, mount, asset_root, source_reader
        )
        result["report"]["additive_reference_deltas"] = additive_deltas
        issues = materialized_reference_issues(files)
        if issues:
            raise ConversionError("; ".join(issues))
        manifest = json.loads(files["materialization.manifest.json"])
        manifest["linked_sources"] = linked_sources
        files["materialization.manifest.json"] = json.dumps(
            manifest, ensure_ascii=False, indent=2
        ) + "\n"
    return result["ir"], result["report"], files, build_tree_integration_report(result)


def check_outputs(output_root, ir, report, materialized_files=None):
    expected = {
        "source.ir.json": ir,
        "conversion.report.json": report,
    }
    failures = []
    for name, value in expected.items():
        path = output_root / name
        if not path.is_file() or read_json(path) != value:
            failures.append({"kind": "generated_output_drift", "path": name})
    for name, value in (materialized_files or {}).items():
        path = output_root / name
        if not path.is_file() or path.read_text(encoding="utf-8") != value:
            failures.append({"kind": "generated_output_drift", "path": name})
    return failures


def unsupported_failure(report):
    return bool(report.get("unsupported"))


def write_outputs_if_supported(output_root, ir, report, materialized_files=None):
    if unsupported_failure(report):
        return False
    write_json(output_root / "source.ir.json", ir)
    write_json(output_root / "conversion.report.json", report)
    for name, value in (materialized_files or {}).items():
        path = output_root / name
        if path.resolve().parent != output_root.resolve() and output_root.resolve() not in path.resolve().parents:
            raise ConversionError("materialized output path is outside output root")
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(value, encoding="utf-8")
    return True


def default_install_namespace(source):
    if not isinstance(source, dict) or not isinstance(source.get("asset_path"), str):
        raise ConversionError("source installation asset is unavailable")
    return "generated"


def installation_manifest_path(source_asset, output_namespace):
    output_namespace = asset_namespace(output_namespace)
    if not output_namespace:
        raise ConversionError("installation namespace is unavailable")
    try:
        return output_namespace + "/" + source_asset_directory(source_asset) + "/installation.manifest.json"
    except MaterializationError as error:
        raise ConversionError("installation source asset is invalid") from error


def installation_files(source_identity, materialized_files, output_namespace):
    output_namespace = asset_namespace(output_namespace)
    manifest_text = materialized_files.get("materialization.manifest.json")
    if not isinstance(manifest_text, str):
        raise ConversionError("materialization manifest is unavailable")
    try:
        materialization = json.loads(manifest_text)
    except json.JSONDecodeError as error:
        raise ConversionError("materialization manifest is invalid") from error
    root_graph = materialization.get("root_graph")
    files = {
        path: text
        for path, text in materialized_files.items()
        if path.endswith((".animgraph.ron", ".fsm.ron"))
    }
    if not isinstance(root_graph, str) or root_graph not in files:
        raise ConversionError("installed root graph is unavailable")
    if any(not path.startswith(output_namespace + "/") for path in files):
        raise ConversionError("installed graph is outside its namespace")
    manifest = {
        "format": INSTALLATION_MANIFEST_FORMAT,
        "source": source_identity,
        "namespace": output_namespace,
        "root_graph": root_graph,
        "files": sorted(files),
    }
    source_asset = source_identity.get("package_path") or source_identity.get("asset")
    if not isinstance(source_asset, str) or not source_asset:
        raise ConversionError("installation source asset is unavailable")
    files[installation_manifest_path(source_asset, output_namespace)] = (
        json.dumps(manifest, ensure_ascii=False, indent=2) + "\n"
    )
    return files


def namespace_materialized_files(files, namespace):
    namespace = asset_namespace(namespace)
    if not namespace:
        return dict(files)
    graph_paths = {
        path
        for path in files
        if path.endswith((".animgraph.ron", ".fsm.ron"))
    }
    if not graph_paths or any(path.startswith(namespace + "/") for path in graph_paths):
        raise ConversionError("materialized namespace relocation is invalid")

    def relocate(value):
        if isinstance(value, str):
            return f"{namespace}/{value}" if value in graph_paths else value
        if isinstance(value, list):
            return [relocate(item) for item in value]
        if isinstance(value, dict):
            return {key: relocate(item) for key, item in value.items()}
        return value

    result = {}
    for path, text in files.items():
        target_path = f"{namespace}/{path}" if path in graph_paths else path
        if path.endswith(".ron"):
            for source_path in sorted(graph_paths, key=len, reverse=True):
                text = text.replace(
                    f'"{source_path}"', f'"{namespace}/{source_path}"'
                )
        elif path == "materialization.manifest.json":
            try:
                text = json.dumps(relocate(json.loads(text)), ensure_ascii=False, indent=2) + "\n"
            except json.JSONDecodeError as error:
                raise ConversionError("materialization manifest is invalid") from error
        result[target_path] = text
    return result


def prune_empty_owned_directories(root, namespace):
    if not isinstance(namespace, str) or not namespace:
        raise ConversionError("installation namespace is unavailable")
    namespace_root = (root / namespace).resolve()
    if root not in namespace_root.parents:
        raise ConversionError("installation namespace is outside root")
    if not namespace_root.is_dir():
        return
    for path in sorted(namespace_root.rglob("*"), key=lambda value: len(value.parts), reverse=True):
        if path.is_dir() and not path.is_symlink():
            try:
                path.rmdir()
            except OSError:
                pass


def install_generated_files(install_root, files, replace=False):
    root = install_root.resolve()
    if not root.is_dir():
        raise ConversionError("installation root is unavailable")
    manifest_path = next(
        (path for path in files if path.endswith("/installation.manifest.json")),
        None,
    )
    if manifest_path is None:
        raise ConversionError("installation manifest is unavailable")
    try:
        expected = json.loads(files[manifest_path])
    except json.JSONDecodeError as error:
        raise ConversionError("installation manifest is invalid") from error
    if expected.get("format") != INSTALLATION_MANIFEST_FORMAT:
        raise ConversionError("installation manifest format is invalid")
    stale_paths = set()
    if replace:
        existing_manifest = root / manifest_path
        if existing_manifest.is_file():
            try:
                existing = read_json(existing_manifest)
            except (OSError, json.JSONDecodeError) as error:
                raise ConversionError("installed namespace ownership is invalid") from error
            if (
                existing.get("format") != INSTALLATION_MANIFEST_FORMAT
                or existing.get("source") != expected.get("source")
                or existing.get("namespace") != expected.get("namespace")
            ):
                raise ConversionError("installed namespace ownership is invalid")
            if not isinstance(existing.get("files"), list) or not all(
                isinstance(path, str) and path for path in existing["files"]
            ):
                raise ConversionError("installed namespace ownership is invalid")
            stale_paths = set(existing["files"]) - set(expected.get("files", []))
    targets = {}
    for path, text in files.items():
        target = (root / path).resolve()
        if root not in target.parents:
            raise ConversionError("installation target is outside root")
        if not replace and target.is_file() and target.read_text(encoding="utf-8") != text:
            raise ConversionError("installed generated output conflicts")
        targets[path] = target
    for path, text in files.items():
        target = targets[path]
        target.parent.mkdir(parents=True, exist_ok=True)
        if replace or not target.is_file():
            target.write_text(text, encoding="utf-8")
    namespace_root = (root / expected["namespace"]).resolve()
    for path in stale_paths:
        target = (root / path).resolve()
        if namespace_root not in target.parents or not target.is_file():
            raise ConversionError("installed namespace ownership is invalid")
        target.unlink()
    prune_empty_owned_directories(root, expected.get("namespace"))


def installation_drift(install_root, files):
    root = install_root.resolve()
    if not root.is_dir():
        return [{"kind": "installation_root_unavailable"}]
    failures = []
    for path, text in files.items():
        target = (root / path).resolve()
        if root not in target.parents:
            failures.append({"kind": "installation_target_outside_root", "path": path})
        elif not target.is_file() or target.read_text(encoding="utf-8") != text:
            failures.append({"kind": "installed_generated_output_drift", "path": path})
    return failures


def expected_materialized_files(source, ir, report, output_namespace=""):
    if unsupported_failure(report):
        return {}
    try:
        return materialize_files(source, ir, namespace=output_namespace)
    except MaterializationError as error:
        report["unsupported"].append(
            {"kind": "source_materialization_unsupported", "detail": str(error)}
        )
        report["pass"] = False
        return {}


def fixture_registry():
    return validate_registry(
        {
            "format": "als_ref_to_bevy_animgraph_mapping_registry_v1",
            "node_classes": {"/Script/AnimGraph.AnimGraphNode_Root": {"renderer": "root_output"}},
            "function_rules": [],
        }
    )


def run_self_test():
    registry = fixture_registry()
    assert source_property_key("StandingState.PlayRate") == "standing_state.play_rate"
    assert (
        source_property_key("GroundedState.VelocityBlend.ForwardAmount")
        == "grounded_state.velocity_blend.forward_amount"
    )
    node = {
        "object_path": "/Asset:AnimGraph.Unknown",
        "class_path": "/Script/AnimGraph.AnimGraphNode_Unknown",
        "title": "Unknown",
    }
    renderer, issue = mapping_for(node, registry)
    assert renderer is None
    assert issue["kind"] == "unregistered_source_node"
    function_node = {
        "object_path": "/Asset:AnimGraph.Function",
        "class_path": "/Script/AnimGraph.AnimGraphNode_Root",
        "title": "Output Pose\nAnimGraph",
    }
    renderer, issue = mapping_for(function_node, registry)
    assert renderer == "root_output"
    assert issue is None
    assert renderer_semantic_issues(function_node, renderer, None, "graph", False) == []
    assert renderer_semantic_issues(
        {
            "object_path": "/Asset:AnimGraph.StateMachine",
            "class_path": "/Script/AnimGraph.AnimGraphNode_StateMachine",
        },
        "state_machine",
        None,
        "graph",
        False,
    )[0]["kind"] == "source_renderer_unimplemented"
    assert 'operation: "less"' in render_transition_contract(
        transition_renderer().fixture_contract()
    )
    function_registry = validate_registry(
        {
            "format": "als_ref_to_bevy_animgraph_mapping_registry_v1",
            "node_classes": {
                "/Script/AnimGraph.AnimGraphNode_CallFunction": {"renderer": "anim_call"},
                "/Script/BlueprintGraph.K2Node_CallFunction": {"renderer": "blueprint_call"},
            },
            "function_rules": [
                {
                    "source_class": "/Script/AnimGraph.AnimGraphNode_CallFunction",
                    "signature": "InitializeLean",
                    "renderer": "pose_lifecycle_call",
                },
                {
                    "source_class": "/Script/BlueprintGraph.K2Node_CallFunction",
                    "signature": "NOT Boolean",
                    "renderer": "boolean_not",
                },
                {
                    "source_class": "/Script/BlueprintGraph.K2Node_CallFunction",
                    "signature": "Absolute (Float)",
                    "renderer": "absolute",
                },
                {
                    "source_class": "/Script/BlueprintGraph.K2Node_CallFunction",
                    "signature": "Is Gameplay Tag Valid",
                    "renderer": "gameplay_tag_valid",
                }
            ],
        }
    )
    crlf_function = {
        "object_path": "/Asset:AnimGraph.InitializeLean",
        "class_path": "/Script/AnimGraph.AnimGraphNode_CallFunction",
        "title": "InitializeLean\r\nOn Become Relevant",
    }
    renderer, issue = mapping_for(crlf_function, function_registry)
    assert renderer == "pose_lifecycle_call"
    assert issue is None
    for signature, expected in (
        ("NOT Boolean", "boolean_not"),
        ("Absolute (Float)", "absolute"),
        ("Is Gameplay Tag Valid", "gameplay_tag_valid"),
    ):
        renderer, issue = mapping_for(
            {
                "object_path": "/Asset:Transition.Function",
                "class_path": "/Script/BlueprintGraph.K2Node_CallFunction",
                "title": signature,
            },
            function_registry,
        )
        assert renderer == expected and issue is None
    binding_registry = validate_registry(
        {
            "format": "als_ref_to_bevy_animgraph_mapping_registry_v1",
            "node_classes": {
                "/Script/AnimGraph.AnimGraphNode_MultiWayBlend": {"renderer": "multi_way_blend"},
                "/Script/AnimGraph.AnimGraphNode_BlendSpacePlayer": {"renderer": "blendspace_player"},
            },
            "function_rules": [],
            "enum_entry_values": {"EAlsHipsDirection": {"LeftBackward": 3.0}},
        }
    )
    getter_machine = "/Asset:AnimGraph.Machine"
    getter_state = getter_machine + ".State"
    getter_plans = animation_getter_machine_plans(
        {
            "asset_path": "/Asset",
            "graphs": [
                {
                    "nodes": [
                        {
                            "object_path": getter_machine,
                            "node_guid": "10000000-0000-4000-8000-000000000001",
                        }
                    ]
                }
            ],
        },
        {
            "AnimGraph.Machine.Fsm": {
                "states": [{"source_node": getter_state, "id": "10000000-0000-4000-8000-000000000002"}],
                "aliases": [],
            }
        },
    )
    getter_semantic = {
        "source_function": "GetInstanceStateWeight",
        "source_machine": getter_machine,
        "source_state": getter_state,
    }
    bind_animation_getter(getter_semantic, getter_plans)
    assert getter_semantic["target_fsm_node"] == "10000000-0000-4000-8000-000000000001"
    assert getter_semantic["target_state_id"] == "10000000-0000-4000-8000-000000000002"
    try:
        validate_registry(
            {
                **binding_registry,
                "sync_group_phase_keys": {"Movement": "manual.phase"},
            }
        )
    except ConversionError:
        pass
    else:
        raise AssertionError("static sync group registry was accepted")
    binding_node = {
        "object_path": "/Asset:AnimGraph.Multi",
        "class_path": "/Script/AnimGraph.AnimGraphNode_MultiWayBlend",
        "pins": [
            {"name": "X", "direction": "Input", "hidden": False, "linked_to": []},
            {"name": "Y", "direction": "Input", "hidden": False, "linked_to": []},
            {"name": "PlayRate", "direction": "Input", "hidden": False, "linked_to": []},
        ],
    }
    property_node = {
        "object_path": "/Asset:AnimGraph.Property",
        "class_path": "/Script/PropertyAccessNode.K2Node_PropertyAccess",
        "properties": [{"name": "TextPath", "exported_text": "GetParent.PoseState.GroundedAmount"}],
        "pins": [
            {
                "name": "Value",
                "direction": "Output",
                "hidden": False,
                "pin_type_properties": [
                    {"name": "PinCategory", "exported_text": "real"},
                    {"name": "PinSubCategory", "exported_text": "float"},
                ],
            }
        ],
    }
    property_semantic = {"renderer": "property_access"}
    assert not resolve_property_bindings(property_node, property_semantic, binding_registry)
    assert property_semantic["resolved_property_bindings"] == [
        {
            "source_pin": "Value",
            "source_property_path": "GetParent.PoseState.GroundedAmount",
            "target_key": "pose_state.grounded_amount",
        }
    ]
    assert integration_runtime_inputs(
        {
            "graphs": [
                {
                    "nodes": [
                        {
                            "source_node": property_node["object_path"],
                            "renderer": "property_access",
                            "semantic": property_semantic,
                        }
                    ]
                }
            ]
        }
    )[0]["target_key"] == "pose_state.grounded_amount"
    property_node["properties"][0]["exported_text"] = ""
    assert resolve_property_bindings(property_node, {"renderer": "property_access"}, binding_registry)[0][
        "kind"
    ] == "source_property_access_unavailable"
    binding_semantic = {
        "renderer": "multi_way_blend",
        "pose_count": 2,
        "desired_alpha_paths": ["GroundedState.VelocityBlend.ForwardAmount", ""],
    }
    assert not resolve_property_bindings(
        binding_node,
        binding_semantic,
        binding_registry,
    )
    assert binding_semantic["desired_alpha_keys"] == [
        "grounded_state.velocity_blend.forward_amount",
        "",
    ]
    curve_semantic = {
        "renderer": "modify_curve",
        "curve_names": ["RotationYawOffset"],
        "curve_value_paths": ["GroundedState.RotationYawOffsets.ForwardAngle"],
    }
    assert not resolve_property_bindings(
        binding_node,
        curve_semantic,
        binding_registry,
    )
    assert curve_semantic["curve_value_keys"] == [
        "grounded_state.rotation_yaw_offsets.forward_angle"
    ]
    curve_semantic["alpha_path"] = "InAirState.GroundPredictionAmount"
    assert not resolve_property_bindings(
        binding_node,
        curve_semantic,
        binding_registry,
    )
    assert curve_semantic["alpha_key"] == "in_air_state.ground_prediction_amount"
    two_way_semantic = {
        "renderer": "two_way_blend",
        "alpha_path": "InAirState.GroundPredictionAmount",
    }
    assert not resolve_property_bindings(
        binding_node,
        two_way_semantic,
        binding_registry,
    )
    assert two_way_semantic["alpha_key"] == "in_air_state.ground_prediction_amount"
    sequence_semantic = {
        "renderer": "sequence_player",
        "play_rate_path": "InAirState.JumpPlayRate",
        "loop_animation_path": "",
    }
    assert not resolve_property_bindings(
        binding_node,
        sequence_semantic,
        binding_registry,
    )
    assert sequence_semantic["play_rate_key"] == "in_air_state.jump_play_rate"
    transition_node = {
        "object_path": "/Asset:Transition.Result",
        "class_path": "/Script/AnimGraph.AnimGraphNode_TransitionResult",
        "pins": [
            {
                "name": "bCanEnterTransition",
                "direction": "Input",
                "hidden": False,
                "linked_to": [],
            }
        ],
        "binding": {
            "properties": [
                {
                    "name": "PropertyBindings",
                    "exported_text": '(("bCanEnterTransition", (PropertyName="bCanEnterTransition",PathAsText="RotateInPlaceState.bRotatingLeft",bIsBound=True)))',
                }
            ]
        },
    }
    transition_semantic = {"renderer": "transition_result"}
    assert not resolve_property_bindings(transition_node, transition_semantic, binding_registry)
    assert transition_semantic["condition_key"] == "rotate_in_place_state.b_rotating_left"
    assert handled_property_binding_pins("transition_result", transition_semantic) == {"bCanEnterTransition"}
    assert not unhandled_property_binding_issues(
        transition_node,
        "transition_result",
        transition_semantic,
    )
    sync_graphs = [
        {
            "source_graph": "AnimGraph",
            "nodes": [
                {
                    "source_node": "/Asset:AnimGraph.PlayerA",
                    "source_class": "/Script/AnimGraph.AnimGraphNode_SequencePlayer",
                    "renderer": "sequence_player",
                    "semantic": {
                        "renderer": "sequence_player",
                        "sync_group": "Second Pivot",
                        "sync_group_role": "CanBeLeader",
                        "sync_method": "SyncGroup",
                        "start_position": 0.2,
                        "loop_animation": True,
                        "play_rate": 1.0,
                        "play_rate_path": "StandingState.PlayRate",
                        "play_rate_key": "standing_state.play_rate",
                    },
                },
                {
                    "source_node": "/Asset:AnimGraph.PlayerB",
                    "source_class": "/Script/AnimGraph.AnimGraphNode_SequencePlayer",
                    "renderer": "sequence_player",
                    "semantic": {
                        "renderer": "sequence_player",
                        "sync_group": "Second Pivot",
                        "sync_group_role": "AlwaysFollower",
                        "sync_method": "SyncGroup",
                        "start_position": 0.0,
                        "loop_animation": True,
                        "play_rate": 1.0,
                        "play_rate_path": "",
                        "play_rate_key": "",
                    },
                },
            ],
        }
    ]
    sync_assets = [
        {
            "source_node": "/Asset:AnimGraph.PlayerA",
            "source_timing": {
                "duration": 1.0,
                "markers": [{"name": "Left", "time": 0.0}, {"name": "Right", "time": 0.5}],
            },
        },
        {
            "source_node": "/Asset:AnimGraph.PlayerB",
            "source_timing": {
                "duration": 0.8,
                "markers": [{"name": "Left", "time": 0.1}, {"name": "Right", "time": 0.4}],
            },
        },
    ]
    sync_plans, sync_nodes, sync_issues = build_sync_group_plans(
        "/ALS/AnimationInstances/AB_Als_Test.AB_Als_Test",
        sync_graphs,
        sync_assets,
    )
    assert not sync_issues
    assert sync_nodes == {"/Asset:AnimGraph.PlayerA", "/Asset:AnimGraph.PlayerB"}
    assert sync_plans[0]["target_phase_key"] == "sync_groups.ab_als_test.second_pivot.phase"
    assert sync_plans[0]["phase_basis_source_node"] == "/Asset:AnimGraph.PlayerA"
    assert sync_graphs[0]["nodes"][1]["semantic"]["sync_phase_key"] == sync_plans[0]["target_phase_key"]
    mismatched_sync_graphs = copy.deepcopy(sync_graphs)
    mismatched_sync_graphs[0]["nodes"][1]["semantic"]["loop_animation"] = False
    _, _, mismatch_issues = build_sync_group_plans(
        "/ALS/AnimationInstances/AB_Als_Test.AB_Als_Test",
        mismatched_sync_graphs,
        sync_assets,
    )
    assert {issue["kind"] for issue in mismatch_issues} == {"source_sync_group_loop_mode_unsupported"}
    enum_semantic = {
        "renderer": "enum_pose_blend",
        "active_enum_path": "GroundedState.HipsDirection",
        "enum_name": "EAlsHipsDirection",
        "selected_enum_entry": "LeftBackward",
    }
    assert not resolve_property_bindings(
        binding_node,
        enum_semantic,
        binding_registry,
    )
    assert enum_semantic["active_key"] == "grounded_state.hips_direction"
    assert enum_semantic["selected_enum_value"] == 3.0
    binding_semantic["desired_alpha_paths"] = ["GroundedState.Unknown", ""]
    assert not resolve_property_bindings(
        binding_node,
        binding_semantic,
        binding_registry,
    )
    assert binding_semantic["desired_alpha_keys"] == ["grounded_state.unknown", ""]
    binding_semantic["desired_alpha_paths"] = ["GroundedState..Unknown", ""]
    assert resolve_property_bindings(
        binding_node,
        binding_semantic,
        binding_registry,
    )[0]["kind"] == "source_property_binding_unsupported"
    binding_node["binding"] = {
        "properties": [
            {
                "name": "PropertyBindings",
                "exported_text": '(("X", (PropertyName="X",PathAsText="GetParent.LeanState.RightAmount",bIsBound=True)),("Y", (PropertyName="Y",PathAsText="GetParent.LeanState.ForwardAmount",bIsBound=True)))',
            }
        ]
    }
    assert resolve_property_bindings(
        binding_node,
        {"renderer": "blendspace_evaluator"},
        binding_registry,
    )[0]["kind"] == "source_blendspace_evaluator_input_unresolved"
    blendspace_semantic = {
        "renderer": "blendspace_evaluator",
        "input_pins": [{"name": "X"}, {"name": "Y"}],
    }
    assert not resolve_property_bindings(
        binding_node,
        blendspace_semantic,
        binding_registry,
    )
    assert blendspace_semantic["input_binding_keys"] == [
        "lean_state.right_amount",
        "lean_state.forward_amount",
    ]
    binding_node["binding"]["properties"][0]["exported_text"] = '(("X", (PropertyName="X",PathAsText="GetParent.StandingState.StrideBlendAmount",bIsBound=True)),("Y", (PropertyName="Y",PathAsText="GetParent.StandingState.WalkRunBlendAmount",bIsBound=True)),("PlayRate", (PropertyName="PlayRate",PathAsText="GetParent.StandingState.PlayRate",bIsBound=True)))'
    player_semantic = {
        "renderer": "blendspace_player",
        "sync_group": "Movement",
        "loop": True,
        "start_position": 0.2,
    }
    assert not resolve_property_bindings(
        binding_node,
        player_semantic,
        binding_registry,
    )
    assert player_semantic["input_binding_keys"] == [
        "standing_state.stride_blend_amount",
        "standing_state.walk_run_blend_amount",
        "standing_state.play_rate",
    ]
    assert "sync_phase_key" not in player_semantic
    player_semantic["start_position"] = 0.0
    assert not resolve_property_bindings(
        binding_node,
        player_semantic,
        binding_registry,
    )
    binding_node["binding"]["properties"][0]["exported_text"] = '(("Alpha", (PropertyName="Alpha",PathAsText="GetParent.InAirState.GroundPredictionAmount",bIsBound=True)))'
    assert unhandled_property_binding_issues(
        binding_node,
        "two_way_blend",
        {"renderer": "two_way_blend"},
    )[0]["kind"] == "source_property_binding_unhandled"
    binding_node["binding"]["properties"][0]["exported_text"] = '(("DesiredAlphas_0", (PropertyName="DesiredAlphas_0",PathAsText="GetParent.GroundedState.VelocityBlend.ForwardAmount",bIsBound=True)))'
    assert not unhandled_property_binding_issues(
        binding_node,
        "multi_way_blend",
        {
            "renderer": "multi_way_blend",
            "desired_alpha_paths": ["GroundedState.VelocityBlend.ForwardAmount"],
            "desired_alpha_keys": ["grounded_state.velocity_blend.forward_amount"],
        },
    )
    binding_node["binding"]["properties"][0]["exported_text"] = '(("CurveValues_0", (PropertyName="CurveValues_0",PathAsText="GetParent.GroundedState.RotationYawOffsets.ForwardAngle",bIsBound=True)))'
    assert not unhandled_property_binding_issues(
        binding_node,
        "modify_curve",
        curve_semantic,
    )
    source_contract = {
        "source_graph_suffix": "AnimGraph",
        "source_graph": {
            "nodes": [
                {
                    "object_path": "/Asset:AnimGraph.Root",
                    "class_path": "/Script/AnimGraph.AnimGraphNode_Root",
                    "title": "Output Pose\nAnimGraph",
                    "position_x": 0,
                    "position_y": 0,
                    "pins": [
                        {
                            "index": 0,
                            "name": "Result",
                            "direction": "Input",
                            "hidden": False,
                            "not_connectable": False,
                            "default_value": "default",
                            "default_value_ignored": False,
                        }
                    ],
                    "properties": [
                        {"name": "Node", "array_index": 0, "exported_text": "fixture"}
                    ],
                }
            ]
        },
        "links": [
            {
                "source": {"node_object_path": "/Asset:AnimGraph.Root", "pin_name": "Pose"},
                "target": {"node_object_path": "/Asset:AnimGraph.Root", "pin_name": "Result"},
            }
        ],
    }
    graph_value, graph_unsupported = graph_ir(
        source_contract,
        registry,
        {"/Asset:AnimGraph.Root"},
    )
    assert not graph_unsupported
    for mutate in [
        lambda value: value["source_graph"]["nodes"][0]["pins"][0].update(default_value="changed"),
        lambda value: value["source_graph"]["nodes"][0].update(position_x=1),
        lambda value: value["source_graph"]["nodes"][0]["properties"][0].update(exported_text="changed"),
        lambda value: value["links"][0]["target"].update(pin_name="Changed"),
    ]:
        changed = copy.deepcopy(source_contract)
        mutate(changed)
        changed_value, changed_unsupported = graph_ir(
            changed,
            registry,
            {"/Asset:AnimGraph.Root"},
        )
        assert not changed_unsupported
        assert graph_value != changed_value
    sequence_registry = validate_registry(
        {
            "format": "als_ref_to_bevy_animgraph_mapping_registry_v1",
            "node_classes": {
                "/Script/AnimGraph.AnimGraphNode_SequencePlayer": {
                    "renderer": "sequence_player"
                }
            },
            "function_rules": [],
        }
    )
    sequence_contract = {
        "source_graph_suffix": "AnimGraph",
        "source_graph": {
            "nodes": [
                {
                    "object_path": "/Asset:AnimGraph.Player",
                    "class_path": "/Script/AnimGraph.AnimGraphNode_SequencePlayer",
                    "title": "Sequence Player",
                    "position_x": 0,
                    "position_y": 0,
                    "pins": [
                        {
                            "index": 0,
                            "name": "PlayRate",
                            "direction": "Input",
                            "hidden": False,
                            "not_connectable": False,
                            "default_value": "1.000000",
                            "default_value_ignored": False,
                        }
                    ],
                    "properties": [
                        {
                            "name": "Node",
                            "array_index": 0,
                            "exported_text": "(GroupName=\"Movement\",Method=SyncGroup,Sequence=\"/Script/Engine.AnimSequence'/ALS/Animations/A_Test.A_Test'\",PlayRateBasis=1.000000,PlayRate=1.000000,PlayRateScaleBiasClampConstants=(),bLoopAnimation=True,PlayRateScaleBiasClampState=(),InitialUpdateFunction=(),BecomeRelevantFunction=(),UpdateFunction=())",
                        }
                    ],
                }
            ]
        },
        "links": [],
    }
    sequence_value, sequence_unsupported = graph_ir(
        sequence_contract,
        sequence_registry,
        {"/Asset:AnimGraph.Player"},
    )
    assert sequence_value["nodes"][0]["semantic"]["sync_group"] == "Movement"
    assert sequence_unsupported[0]["kind"] == "source_sync_group_unsupported"
    invalid_contract_registry = validate_registry(
        {
            "format": "als_ref_to_bevy_animgraph_mapping_registry_v1",
            "node_classes": {
                "/Script/AnimGraph.AnimGraphNode_TwoWayBlend": {
                    "renderer": "two_way_blend"
                }
            },
            "function_rules": [],
        }
    )
    invalid_contract = {
        "source_graph_suffix": "AnimGraph",
        "source_graph": {
            "nodes": [
                {
                    "object_path": "/Asset:AnimGraph.Invalid",
                    "class_path": "/Script/AnimGraph.AnimGraphNode_TwoWayBlend",
                    "title": "Two Way Blend",
                    "position_x": 0,
                    "position_y": 0,
                    "pins": [
                        {"name": "A", "direction": "Input", "hidden": False},
                        {"name": "B", "direction": "Input", "hidden": False},
                        {"name": "Alpha", "direction": "Input", "hidden": False},
                        {"name": "Pose", "direction": "Output", "hidden": False},
                    ],
                    "properties": [
                        {
                            "name": "BlendNode",
                            "exported_text": "(A=(),B=(),Unknown=True,InitialUpdateFunction=(),BecomeRelevantFunction=(),UpdateFunction=())",
                        }
                    ],
                }
            ]
        },
        "links": [],
    }
    _, invalid_contract_issues = graph_ir(
        invalid_contract,
        invalid_contract_registry,
        {"/Asset:AnimGraph.Invalid"},
    )
    assert any(issue["kind"] == "source_property_unsupported" for issue in invalid_contract_issues)
    bad = fixture_registry()
    bad["asset_name"] = "Forbidden"
    try:
        validate_registry(bad)
    except ConversionError:
        pass
    else:
        raise AssertionError("asset-specific registry selector was accepted")
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        content = root / "Content"
        uasset = content / "ALS" / "Character" / "AB_Test.uasset"
        uasset.parent.mkdir(parents=True)
        uasset.write_bytes(b"fixture")
        assert package_path_for_uasset(uasset, content, "/ALS") == "/ALS/ALS/Character/AB_Test.AB_Test"
        assert uasset_for_generated_anim_blueprint(
            "/ALS/ALS/Character/AB_Test.AB_Test", content, "/ALS"
        ) == uasset
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        ir = {"format": IR_FORMAT, "graphs": []}
        report = {"format": REPORT_FORMAT, "pass": True, "unsupported": []}
        write_json(root / "source.ir.json", ir)
        write_json(root / "conversion.report.json", report)
        assert not check_outputs(root, ir, report)
        assert not list(root.rglob("*.ron"))
        write_json(root / "source.ir.json", {"format": IR_FORMAT, "graphs": ["drift"]})
        assert check_outputs(root, ir, report)
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        blocked = {"format": REPORT_FORMAT, "pass": False, "unsupported": [issue]}
        assert unsupported_failure(blocked)
        assert not write_outputs_if_supported(root, {"format": IR_FORMAT}, blocked)
        assert not (root / "source.ir.json").exists()
    integration_ir = {
        "function_dependencies": [
            {
                "source_node": "/Asset:AnimGraph.Refresh",
                "source_function": "RefreshInAir",
                "call_site": "anim_node",
                "resolution": "external",
            },
            {
                "source_node": "/Asset:AnimGraph.Custom",
                "source_function": "CustomActorFunction",
                "call_site": "anim_node",
                "resolution": "external",
            },
            {
                "source_node": "/Asset:AnimGraph.Stop",
                "source_function": "StopTransitionAndTurnInPlaceAnimations",
                "call_site": "StateExitFunction",
                "source_graph": "StopTransitionAndTurnInPlaceAnimations",
                "resolution": "graph",
            },
        ],
        "lifecycle_dispatch": {
            "dispatches": [
                {
                    "source_node": "/Asset:AnimGraph.Refresh",
                    "source_function": "RefreshInAir",
                    "provider": "als_locomotion_refresh_in_air",
                }
            ]
        },
        "external_dependencies": [{"kind": "linked_anim_layer", "source_node": "/Asset:AnimGraph.Layer"}],
        "sync_group_plans": [
            {
                "source_sync_group": "Movement",
                "target_phase_key": "sync_groups.ab_als_standing.movement.phase",
                "target_delta_seconds_key": "animation.delta_seconds",
                "phase_basis_source_node": "/Asset:AnimGraph.Movement",
                "phase_duration": 1.0,
                "phase_markers": [{"name": "Left", "time": 0.0}, {"name": "Right", "time": 0.5}],
                "phase_looped": True,
                "candidates": [{"source_node": "/Asset:AnimGraph.Movement"}],
                "status": "requires_target_provider",
            },
            {
                "source_sync_group": "Secondary Motion",
                "target_phase_key": "sync_groups.ab_als_default.secondary_motion.phase",
                "target_delta_seconds_key": "animation.delta_seconds",
                "phase_basis_source_node": "/Asset:AnimGraph.Secondary",
                "phase_duration": 2.666667,
                "phase_markers": [],
                "phase_looped": True,
                "candidates": [{"source_node": "/Asset:AnimGraph.Secondary"}],
                "status": "requires_target_provider",
            },
        ],
        "graphs": [
            {
                "nodes": [
                    {
                        "source_node": "/Asset:AnimGraph.Movement",
                        "renderer": "blendspace_player",
                        "semantic": {
                            "sync_group": "Movement",
                            "sync_phase_key": "sync_groups.ab_als_standing.movement.phase",
                            "delta_seconds_key": "animation.delta_seconds",
                            "input_pins": [{"name": "PlayRate"}],
                            "input_binding_paths": ["StandingState.PlayRate"],
                            "input_binding_keys": ["standing_state.play_rate"],
                            "resolved_property_bindings": [
                                {
                                    "source_pin": "PlayRate",
                                    "source_property_path": "StandingState.PlayRate",
                                    "target_key": "standing_state.play_rate",
                                }
                            ],
                            "start_position": 0.2,
                            "sync_phase_duration": 1.0,
                            "sync_phase_markers": [
                                {"name": "Left", "time": 0.0},
                                {"name": "Right", "time": 0.5},
                            ],
                        },
                    },
                    {
                        "source_node": "/Asset:AnimGraph.Secondary",
                        "renderer": "sequence_player",
                        "semantic": {
                            "sync_group": "Secondary Motion",
                            "sync_phase_key": "sync_groups.ab_als_default.secondary_motion.phase",
                            "delta_seconds_key": "animation.delta_seconds",
                            "play_rate": 1.0,
                            "start_position": 0.0,
                            "sync_phase_duration": 2.666667,
                            "sync_phase_markers": [],
                        },
                    },
                    {
                        "source_node": "/Asset:Transition.Getter",
                        "renderer": "anim_getter",
                        "semantic": {
                            "source_function": "GetInstanceStateWeight",
                            "source_machine": "/Asset:AnimGraph.Machine",
                            "source_state": "/Asset:AnimGraph.Machine.State",
                            "target_key": "anim_getters.ab_als_test.get_instance_state_weight.anim_graph.machine.anim_graph.machine.state",
                            "target_fsm_node": "10000000-0000-4000-8000-000000000001",
                            "target_state_id": "10000000-0000-4000-8000-000000000002",
                        },
                    },
                    {
                        "source_node": "/Asset:AnimGraph.Slot",
                        "renderer": "slot",
                        "semantic": {
                            "slot_name": "TurnInPlaceStanding",
                            "group_name": "Grounded",
                            "pose_key": "turn_in_place_standing.pose",
                            "weight_key": "turn_in_place_standing.weight",
                            "layer_weight_key": "turn_in_place_standing.layer_weight",
                            "mesh_space_additive_key": "turn_in_place_standing.mesh_space_additive",
                        },
                    },
                    {
                        "source_node": "/Asset:AnimGraph.ControlRig",
                        "renderer": "control_rig",
                        "semantic": {
                            "rig_class": "CR_Als",
                            "rig_input_binding_pin": "__CustomProperty_RigInput",
                            "rig_input_binding_path": "GetControlRigInput",
                        },
                    },
                ],
                "gameplay_tag_providers": [
                    {
                        "source_node": "/Asset:AnimGraph.Gait",
                        "source_property_path": "GetParent.Gait",
                        "target_key": "get_parent.gait",
                        "operation": "matches_tag_exact",
                        "expected_tag": "Als.Gait.Walking",
                        "provider_api": "als_ext::set_gameplay_tag_input",
                        "player_entity": "AnimationGraphPlayer entity",
                        "status": "requires_target_provider",
                    }
                ],
            }
        ],
    }
    integration_blocked = {
        "source": {"asset": "/Asset"},
        "pass": False,
        "unsupported": [
            {
                "kind": "unregistered_source_function",
                "source_node": "/Asset:AnimGraph.Custom",
            },
            {
                "kind": "source_property_binding_unhandled",
                "source_node": "/Asset:AnimGraph.Blend",
                "source_class": "/Script/AnimGraph.AnimGraphNode_TwoWayBlend",
                "renderer": "two_way_blend",
                "source_pin": "Alpha",
                "source_property_path": "InAirState.GroundPredictionAmount",
            },
        ],
    }
    integration = build_integration_report(integration_ir, integration_blocked)
    assert integration["conversion_status"] == "blocked"
    assert integration["handoff_action"] == "repair_conversion_blockers"
    assert [item["status"] for item in integration["external_runtime_functions"]] == [
        "requires_target_implementation",
        "mapped",
    ]
    assert integration["unhandled_property_bindings"][0]["status"] == "requires_target_provider"
    assert integration["sync_phase_providers"][0]["target_phase_key"] == "sync_groups.ab_als_default.secondary_motion.phase"
    assert integration["sync_phase_providers"][0]["status"] == "mapped"
    assert any(
        item["target_phase_key"] == "sync_groups.ab_als_standing.movement.phase"
        and item["phase_basis_source_node"] == "/Asset:AnimGraph.Movement"
        for item in integration["sync_phase_providers"]
    )
    assert integration["runtime_inputs"][0]["target_key"] == "standing_state.play_rate"
    assert integration["animation_state_getter_providers"][0]["source_function"] == "GetInstanceStateWeight"
    assert integration["animation_state_getter_providers"][0]["target_fsm_node"] == "10000000-0000-4000-8000-000000000001"
    assert integration["animation_state_getter_providers"][0]["target_state_id"] == "10000000-0000-4000-8000-000000000002"
    assert integration["slot_providers"][0]["pose_key"] == "turn_in_place_standing.pose"
    assert integration["slot_providers"][0]["mesh_space_additive_key"] == "turn_in_place_standing.mesh_space_additive"
    assert integration["gameplay_tag_providers"][0]["provider_api"] == "als_ext::set_gameplay_tag_input"
    assert integration["control_rig_providers"] == [
        {
            "source_node": "/Asset:AnimGraph.ControlRig",
            "source_rig_class": "CR_Als",
            "source_pin": "__CustomProperty_RigInput",
            "source_function": "GetControlRigInput",
            "target_provider": "crate::control_rig_apply::ControlRigApplyPlugin (AlsControlRigInput)",
            "status": "mapped",
        }
    ]
    blocked_markdown = integration_report_markdown(integration)
    assert "Do not integrate generated RON files." in blocked_markdown
    assert "CustomActorFunction" not in blocked_markdown
    assert "## External graph dependencies" not in blocked_markdown
    assert "## Runtime sync phase providers" not in blocked_markdown
    assert "## Runtime property inputs" not in blocked_markdown
    assert "## Slot providers" not in blocked_markdown
    assert "## Gameplay tag providers" not in blocked_markdown
    assert "sync_groups.ab_als_standing.movement.phase" not in blocked_markdown
    assert "get_parent.gait" not in blocked_markdown
    assert build_integration_report(
        integration_ir,
        {"source": {"asset": "/Asset"}, "pass": True, "unsupported": []},
    )["conversion_status"] == "blocked"
    ready_ir = {
        "function_dependencies": [],
        "external_dependencies": [],
        "sync_group_plans": [],
        "graphs": [{"nodes": [copy.deepcopy(integration_ir["graphs"][0]["nodes"][0])]}],
    }
    ready = build_integration_report(
        ready_ir,
        {"source": {"asset": "/Asset"}, "pass": True, "unsupported": []},
    )
    assert ready["conversion_status"] == "ready"
    assert ready["handoff_action"] == "integrate_generated_assets"
    assert "## External graph dependencies" in integration_report_markdown(ready)
    assert "## Runtime sync phase providers" in integration_report_markdown(ready)
    assert "## Runtime property inputs" in integration_report_markdown(ready)
    assert "## Animation state getter providers" in integration_report_markdown(ready)
    assert "## Slot providers" in integration_report_markdown(ready)
    assert "## Gameplay tag providers" in integration_report_markdown(ready)
    assert "## Control Rig providers" in integration_report_markdown(ready)
    provider = RUNTIME_PROVIDER_CONTRACT["runtime_inputs"].pop("standing_state.play_rate")
    try:
        missing_provider = build_integration_report(
            ready_ir,
            {"source": {"asset": "/Asset"}, "pass": True, "unsupported": []},
        )
    finally:
        RUNTIME_PROVIDER_CONTRACT["runtime_inputs"]["standing_state.play_rate"] = provider
    assert missing_provider["conversion_status"] == "blocked"
    assert missing_provider["handoff_action"] == "repair_conversion_blockers"
    assert any(
        item["status"] == "requires_target_provider"
        for item in missing_provider["runtime_inputs"]
    )
    tree_integration = build_tree_integration_report(
        {
            "source": {"asset_path": "/Asset/Root.Root"},
            "ir": integration_ir,
            "report": {"source": {"asset": "/Asset/Root.Root"}, "pass": True, "unsupported": []},
            "children": [
                {
                    "source": {"asset_path": "/Asset/Child.Child"},
                    "ir": {"function_dependencies": [], "graphs": []},
                    "report": {"source": {"asset": "/Asset/Child.Child"}, "pass": True, "unsupported": []},
                    "children": [],
                }
            ],
        }
    )
    assert [item["source_asset"] for item in tree_integration["source_closure"]] == [
        "/Asset/Root.Root",
        "/Asset/Child.Child",
    ]
    assert tree_integration["control_rig_providers"] == integration["control_rig_providers"]
    assert tree_integration["conversion_status"] == "blocked"
    source_asset = "/Asset/AB_Als.AB_Als"
    namespace = default_install_namespace({"asset_path": source_asset})
    root_graph = graph_path(source_asset, "AnimGraph", namespace)
    relocated = namespace_materialized_files(
        {
            "Asset/AnimGraph.animgraph.ron": 'child: "Asset/State.fsm.ron", clip: "animations/A.anim.ron"\n',
            "Asset/State.fsm.ron": "()\n",
            "materialization.manifest.json": json.dumps(
                {
                    "root_graph": "Asset/AnimGraph.animgraph.ron",
                    "graphs": {"AnimGraph": "Asset/AnimGraph.animgraph.ron"},
                    "fsms": {"State": "Asset/State.fsm.ron"},
                }
            ),
        },
        namespace,
    )
    assert set(relocated) == {
        "generated/Asset/AnimGraph.animgraph.ron",
        "generated/Asset/State.fsm.ron",
        "materialization.manifest.json",
    }
    assert 'child: "generated/Asset/State.fsm.ron"' in relocated[
        "generated/Asset/AnimGraph.animgraph.ron"
    ]
    assert 'clip: "animations/A.anim.ron"' in relocated[
        "generated/Asset/AnimGraph.animgraph.ron"
    ]
    assert json.loads(relocated["materialization.manifest.json"])["root_graph"] == (
        "generated/Asset/AnimGraph.animgraph.ron"
    )
    installed = installation_files(
        {"asset": "/Asset/AB_Als.AB_Als"},
        {
            "materialization.manifest.json": json.dumps({"root_graph": root_graph}),
            root_graph: "()\n",
        },
        namespace,
    )
    assert installation_manifest_path(source_asset, namespace) in installed
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        install_generated_files(root, installed)
        assert not installation_drift(root, installed)
        (root / root_graph).write_text("drift\n", encoding="utf-8")
        assert installation_drift(root, installed)
        try:
            install_generated_files(root, installed)
        except ConversionError:
            pass
        else:
            raise AssertionError("installed output conflict was accepted")
        install_generated_files(root, installed, replace=True)
        assert not installation_drift(root, installed)
        stale = root / namespace / "obsolete.fsm.ron"
        stale.parent.mkdir(parents=True, exist_ok=True)
        stale.write_text("()\n", encoding="utf-8")
        manifest_path = root / installation_manifest_path(source_asset, namespace)
        manifest = read_json(manifest_path)
        manifest["files"].append(stale.relative_to(root).as_posix())
        write_json(manifest_path, manifest)
        install_generated_files(root, installed, replace=True)
        assert not stale.exists()
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        owned_empty = root / namespace / "obsolete" / "empty"
        owned_empty.mkdir(parents=True)
        owned_kept = root / namespace / "keep"
        owned_kept.mkdir(parents=True)
        (owned_kept / "record.txt").write_text("keep\n", encoding="utf-8")
        outside_empty = root / "outside" / "empty"
        outside_empty.mkdir(parents=True)
        prune_empty_owned_directories(root, namespace)
        assert not owned_empty.exists()
        assert owned_kept.is_dir()
        assert outside_empty.is_dir()
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        write_integration_report(root, integration)
        assert not integration_report_drift(root, integration)
        (root / "integration.report.md").write_text("drift\n", encoding="utf-8")
        assert integration_report_drift(root, integration)
    return {
        "unregistered_node_rejected": "PASS",
        "registry_asset_special_case_rejected": "PASS",
        "uasset_package_path_derived_without_source_json_input": "PASS",
        "generated_anim_blueprint_class_path_resolved_without_whitelist": "PASS",
        "crlf_function_signature_normalized": "PASS",
        "source_property_paths_derived_or_invalid_rejected": "PASS",
        "source_property_paths_use_dotted_rust_naming": "PASS",
        "animation_state_getter_targets_are_bound": "PASS",
        "source_blendspace_player_bindings_and_phase_contract_resolved": "PASS",
        "unhandled_source_property_bindings_rejected": "PASS",
        "source_pin_link_position_property_drift_rejected": "PASS",
        "source_pose_contract_blocker_rejected": "PASS",
        "malformed_pose_contract_is_reported_not_raised": "PASS",
        "unimplemented_registered_renderer_rejected": "PASS",
        "transition_renderer_is_callable_from_converter": "PASS",
        "deterministic_ir_drift_rejected": "PASS",
        "unsupported_source_blocks_generation": "PASS",
        "core_does_not_emit_partial_bevy_graphs": "PASS",
        "integration_dependencies_are_reported": "PASS",
        "missing_runtime_provider_blocks_integration_ready": "PASS",
        "generic_source_sync_phase_plan_is_reported": "PASS",
        "integration_report_drift_rejected": "PASS",
        "namespaced_installation_is_deterministic_and_checked": "PASS",
        "materialized_closure_namespace_relocation_preserves_non_graph_assets": "PASS",
        "owned_empty_install_directories_pruned": "PASS",
        "recursive_integration_dependencies_are_aggregated": "PASS",
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--uasset", type=Path)
    parser.add_argument("--content-root", type=Path)
    parser.add_argument("--mount", default="/ALS")
    parser.add_argument("--source-reader", type=Path, default=ROOT / "tools" / "run_ue_source_reader.ps1")
    parser.add_argument("--source-reader-timeout-seconds", type=int, default=300)
    parser.add_argument("--source-reader-no-progress-seconds", type=int, default=150)
    parser.add_argument("--root-graph", default="AnimGraph")
    parser.add_argument("--registry", type=Path, default=ROOT / "tools" / "animgraph_authoring" / "als_ref_mapping_registry.json")
    parser.add_argument("--asset-root", type=Path, default=ROOT / "assets")
    parser.add_argument("--output-root", type=Path)
    parser.add_argument("--install-root", type=Path)
    parser.add_argument("--install-namespace")
    parser.add_argument("--replace-installed", action="store_true")
    parser.add_argument("--report", type=Path)
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
        return 0
    if args.uasset is None or args.output_root is None or args.write == args.check:
        raise ConversionError("uasset, output-root, and exactly one mode are required")
    if args.install_namespace is not None and args.install_root is None:
        raise ConversionError("installation namespace requires an installation root")
    if args.replace_installed and args.install_root is None:
        raise ConversionError("installed replacement requires an installation root")
    if args.install_root is not None and args.install_root.resolve() == args.output_root.resolve():
        raise ConversionError("installation root and output root must differ")
    if args.install_root is not None and not args.install_root.is_dir():
        raise ConversionError("installation root is unavailable")
    registry = validate_registry(read_json(args.registry))
    if not args.asset_root.is_dir():
        raise ConversionError("target asset root is unavailable")
    source_content_root = content_root_for_uasset(args.uasset.resolve(), args.content_root)
    source, source_identity = read_uasset_source(
        args.uasset,
        args.content_root,
        args.mount,
        args.source_reader,
        args.source_reader_timeout_seconds,
        args.source_reader_no_progress_seconds,
    )
    output_namespace = (
        asset_namespace(args.install_namespace)
        if args.install_namespace is not None
        else default_install_namespace(source)
    ) if args.install_root is not None else ""
    try:
        ir, report, materialized_files, integration = conversion_tree_outputs(
            source,
            source_identity,
            args.root_graph,
            registry,
            args.asset_root,
            source_content_root,
            args.mount,
            args.source_reader,
            args.source_reader_timeout_seconds,
            args.source_reader_no_progress_seconds,
            output_namespace,
        )
    except Exception as error:
        report = {
            "format": REPORT_FORMAT,
            "source": source_identity,
            "function_dependencies": [],
            "unsupported": [
                {
                    "kind": "conversion_internal_failure",
                    "error_type": type(error).__name__,
                    "detail": str(error),
                }
            ],
            "pass": False,
        }
        if args.report is not None:
            write_json(args.report, report)
        if args.write:
            args.output_root.mkdir(parents=True, exist_ok=True)
            write_json(args.output_root / "conversion.failure.json", report)
        print(json.dumps(report, ensure_ascii=False, indent=2))
        return 3
    installed_files = (
        installation_files(source_identity, materialized_files, output_namespace)
        if args.install_root is not None and not unsupported_failure(report)
        else {}
    )
    if args.report is not None:
        write_json(args.report, report)
    if args.write:
        write_integration_report(args.output_root, integration)
    else:
        drift = integration_report_drift(args.output_root, integration)
        if drift:
            report["drift"] = drift
            print(json.dumps(report, ensure_ascii=False, indent=2))
            return 2
    if unsupported_failure(report):
        print(json.dumps(report, ensure_ascii=False, indent=2))
        return 3
    if args.write:
        if not write_outputs_if_supported(args.output_root, ir, report, materialized_files):
            raise ConversionError("unsupported source unexpectedly reached output generation")
        if args.install_root is not None:
            install_generated_files(
                args.install_root, installed_files, args.replace_installed
            )
    else:
        drift = check_outputs(args.output_root, ir, report, materialized_files)
        if args.install_root is not None:
            drift.extend(installation_drift(args.install_root, installed_files))
        if drift:
            report["drift"] = drift
            print(json.dumps(report, ensure_ascii=False, indent=2))
            return 2
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
