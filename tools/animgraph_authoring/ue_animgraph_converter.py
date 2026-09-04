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
from ue_animation_asset_resolver import AssetResolutionError, resolve_graph_animation_assets
from ue_animgraph_pose_contract import (
    PoseContractError,
    input_is_linked,
    pose_contract,
    property_binding_paths,
)
from ue_animgraph_pose_renderer import PoseRenderError, SUPPORTED_RENDERERS, render as render_pose_graph
from ue_animgraph_reachability import traverse
from ue_animgraph_fsm_contract import FsmContractError, fsm_contract
from ue_animgraph_fsm_renderer import FsmRenderError, render_plan as render_fsm_plan
from ue_animgraph_lifecycle import LifecycleRenderError, resolve_lifecycle
from ue_animgraph_materializer import MaterializationError, materialize_files


IR_FORMAT = "ue_animgraph_contract_ir_v1"
REPORT_FORMAT = "ue_animgraph_conversion_report_v1"
INTEGRATION_REPORT_FORMAT = "ue_animgraph_integration_report_v1"
FORBIDDEN_REGISTRY_KEYS = {"asset", "asset_name", "asset_path", "source_asset", "node_guid"}
IR_ONLY_RENDERERS = {"root_output", "comment_group"}
TRANSITION_RENDERERS = {
    "transition_result",
    "property_access",
    "boolean_operator",
    "comparison_operator",
    "gameplay_tag_equal",
    "boolean_to_float",
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
    return transition_renderer().graph_text(graph_contract)


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


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
    with tempfile.TemporaryDirectory() as directory:
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


def validate_registry(value):
    if not isinstance(value, dict) or value.get("format") != "als_ref_to_bevy_animgraph_mapping_registry_v1":
        raise ConversionError("mapping registry format is invalid")
    node_classes = value.get("node_classes")
    functions = value.get("function_rules")
    property_binding_rules = value.get("property_binding_rules", [])
    enum_entry_values = value.get("enum_entry_values", {})
    sync_group_phase_keys = value.get("sync_group_phase_keys", {})
    fsm_state_label_keys = value.get("fsm_state_label_keys", {})
    if not isinstance(node_classes, dict) or not node_classes:
        raise ConversionError("mapping registry node classes are invalid")
    if not isinstance(functions, list):
        raise ConversionError("mapping registry function rules are invalid")
    if not isinstance(property_binding_rules, list):
        raise ConversionError("mapping registry property binding rules are invalid")
    if not isinstance(enum_entry_values, dict):
        raise ConversionError("mapping registry enum entry values are invalid")
    if not isinstance(sync_group_phase_keys, dict):
        raise ConversionError("mapping registry sync group phase keys are invalid")
    if not isinstance(fsm_state_label_keys, dict):
        raise ConversionError("mapping registry FSM state label keys are invalid")
    for group, key in sync_group_phase_keys.items():
        if not isinstance(group, str) or not group or not isinstance(key, str) or not key:
            raise ConversionError("mapping registry sync group phase key is invalid")
    for name, key in fsm_state_label_keys.items():
        if not isinstance(name, str) or not name or not isinstance(key, str) or not key:
            raise ConversionError("mapping registry FSM state label key is invalid")
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
    seen = set()
    for rule in property_binding_rules:
        if not isinstance(rule, dict) or set(rule) != {"source_path", "target_key"}:
            raise ConversionError("mapping registry property binding rule is invalid")
        source_path = rule["source_path"]
        target_key = rule["target_key"]
        if not isinstance(source_path, str) or not source_path or not isinstance(target_key, str) or not target_key:
            raise ConversionError("mapping registry property binding rule value is invalid")
        if source_path.startswith("GetParent.") or "{" in source_path or "}" in source_path:
            raise ConversionError("mapping registry property binding source path is invalid")
        if target_key.replace("{asset_layer}", "").find("{") >= 0 or "}" in target_key.replace("{asset_layer}", ""):
            raise ConversionError("mapping registry property binding target key is invalid")
        if source_path in seen:
            raise ConversionError("mapping registry property binding rule is duplicated")
        seen.add(source_path)
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


def source_asset_layer(source_asset):
    if not isinstance(source_asset, str):
        raise ConversionError("source property binding asset is invalid")
    name = source_asset.rsplit(".", 1)[-1]
    match = re.fullmatch(r"AB_Als_([A-Za-z0-9_]+)", name)
    if match is None:
        raise ConversionError("source property binding layer is unsupported")
    return match.group(1).lower()


def resolve_bound_keys(source_asset, node, renderer, pins, paths, registry):
    if (
        not isinstance(paths, list)
        or len(pins) != len(paths)
        or not all(isinstance(pin, str) and isinstance(path, str) for pin, path in zip(pins, paths))
    ):
        return [], [{"kind": "source_property_binding_unsupported", "source_node": node.get("object_path")}]
    rules = {rule["source_path"]: rule["target_key"] for rule in registry.get("property_binding_rules", [])}
    keys = []
    issues = []
    for pin, path in zip(pins, paths):
        if not path:
            keys.append("")
            continue
        template = rules.get(path)
        if template is None:
            issues.append(
                {
                    "kind": "source_property_binding_unresolved",
                    "source_node": node.get("object_path"),
                    "source_class": node.get("class_path"),
                    "renderer": renderer,
                    "source_pin": pin,
                    "source_property_path": path,
                }
            )
            keys.append("")
            continue
        try:
            key = (
                template.replace("{asset_layer}", source_asset_layer(source_asset))
                if "{asset_layer}" in template
                else template
            )
        except ConversionError as error:
            issues.append(
                {
                    "kind": "source_property_binding_context_unsupported",
                    "source_node": node.get("object_path"),
                    "source_class": node.get("class_path"),
                    "renderer": renderer,
                    "source_property_path": path,
                    "detail": str(error),
                }
            )
            keys.append("")
            continue
        keys.append(key)
    return keys, issues


def resolve_property_bindings(source_asset, node, semantic, registry):
    if not isinstance(semantic, dict):
        return []
    renderer = semantic.get("renderer")
    if renderer == "multi_way_blend":
        paths = semantic.get("desired_alpha_paths")
        pins = [f"DesiredAlphas_{index}" for index in range(semantic.get("pose_count", 0))]
        keys, issues = resolve_bound_keys(source_asset, node, renderer, pins, paths, registry)
        semantic["desired_alpha_keys"] = keys
        return issues
    if renderer == "modify_curve":
        paths = semantic.get("curve_value_paths")
        pins = [f"CurveValues_{index}" for index in range(len(semantic.get("curve_names", [])))]
        keys, issues = resolve_bound_keys(source_asset, node, renderer, pins, paths, registry)
        semantic["curve_value_keys"] = keys
        alpha_keys, alpha_issues = resolve_bound_keys(
            source_asset,
            node,
            renderer,
            ["Alpha"],
            [semantic.get("alpha_path", "")],
            registry,
        )
        semantic["alpha_key"] = alpha_keys[0] if len(alpha_keys) == 1 else ""
        return [*issues, *alpha_issues]
    if renderer == "two_way_blend":
        keys, issues = resolve_bound_keys(
            source_asset,
            node,
            renderer,
            ["Alpha"],
            [semantic.get("alpha_path", "")],
            registry,
        )
        semantic["alpha_key"] = keys[0] if len(keys) == 1 else ""
        return issues
    if renderer == "sequence_player":
        keys, issues = resolve_bound_keys(
            source_asset,
            node,
            renderer,
            ["PlayRate", "bLoopAnimation"],
            [
                semantic.get("play_rate_path", ""),
                semantic.get("loop_animation_path", ""),
            ],
            registry,
        )
        semantic["play_rate_key"] = keys[0] if len(keys) == 2 else ""
        semantic["loop_animation_key"] = keys[1] if len(keys) == 2 else ""
        return issues
    if renderer == "blendspace_evaluator":
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
        paths = [bindings.get("X", ""), bindings.get("Y", "")]
        keys, issues = resolve_bound_keys(
            source_asset,
            node,
            renderer,
            ["X", "Y"],
            paths,
            registry,
        )
        semantic["input_binding_paths"] = paths
        semantic["input_binding_keys"] = keys
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
                for pin, path in zip(("X", "Y"), paths)
                if path and input_is_linked(node, pin)
            ],
        ]
    if renderer == "enum_pose_blend":
        keys, issues = resolve_bound_keys(
            source_asset,
            node,
            renderer,
            ["ActiveEnumValue"],
            [semantic.get("active_enum_path", "")],
            registry,
        )
        semantic["active_key"] = keys[0] if len(keys) == 1 else ""
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
    if renderer == "two_way_blend":
        return {"Alpha"} if semantic.get("alpha_path") else set()
    if renderer == "sequence_player":
        handled = set()
        if semantic.get("play_rate_path"):
            handled.add("PlayRate")
        if semantic.get("loop_animation_path"):
            handled.add("bLoopAnimation")
        return handled
    if renderer == "blendspace_evaluator":
        paths = semantic.get("input_binding_paths")
        keys = semantic.get("input_binding_keys")
        if not isinstance(paths, list) or not isinstance(keys, list) or len(paths) != 2 or len(keys) != 2:
            return set()
        return {
            pin
            for pin, path, key in zip(("X", "Y"), paths, keys)
            if isinstance(path, str) and path and isinstance(key, str)
        }
    if renderer == "enum_pose_blend":
        return {"ActiveEnumValue"} if semantic.get("active_enum_path") else set()
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
        if semantic is None and renderer == "blendspace_evaluator":
            asset = animation_by_node.get(node.get("object_path"))
            if (
                not isinstance(asset, dict)
                or asset.get("category") != "blendspace"
                or not isinstance(asset.get("blendspace"), dict)
                or not isinstance(asset.get("evaluator"), dict)
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
                    **asset["evaluator"],
                    "blendspace": asset["blendspace"],
                }
        if semantic is None and renderer in POSE_SCALAR_RENDERERS:
            semantic = {"renderer": renderer}
        node_fsm_plan = state_machine_plans.get(node.get("object_path"))
        fsm_renderable = fsm_plan is not None or node_fsm_plan is not None
        if semantic is None and renderer in FSM_RENDERERS and fsm_renderable:
            selected = fsm_plan if fsm_plan is not None else node_fsm_plan
            semantic = {"fsm_source_graph": selected["source_graph"]}
        contract_issues.extend(
            resolve_property_bindings(
                graph_contract.get("source_asset"),
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


def pose_renderer_status(graph_contract, graph_value, animation_assets, sync_group_phase_keys):
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
            sync_group_phase_keys,
        )
    except PoseRenderError as error:
        return {"state": "blocked", "detail": str(error)}
    return {"state": "renderable"}


def supported_sync_group_nodes(graphs, animation_assets, registry):
    assets = {
        asset.get("source_node"): asset
        for asset in animation_assets
        if isinstance(asset.get("source_node"), str)
    }
    supported = set()
    for graph in graphs:
        for node in graph["nodes"]:
            semantic = node.get("semantic")
            if node.get("renderer") != "sequence_player" or not isinstance(semantic, dict):
                continue
            group = semantic.get("sync_group")
            asset = assets.get(node.get("source_node"))
            timing = asset.get("source_timing") if isinstance(asset, dict) else None
            if (
                isinstance(group, str)
                and group in registry["sync_group_phase_keys"]
                and isinstance(timing, dict)
                and timing.get("markers") == []
            ):
                supported.add(node["source_node"])
    return supported


def build_ir(
    source,
    root_graph,
    registry,
    asset_root,
    source_content_root=None,
    source_mount=None,
    source_reader=None,
):
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
            fsm_plans[entry["source_graph"]] = render_fsm_plan(
                source,
                entry["source_graph"],
                source_content_root,
                source_mount,
                source_reader,
            )
        except FsmRenderError as error:
            unsupported.append(
                {
                    "kind": "source_fsm_renderer_unsupported",
                    "source_graph": entry["source_graph"],
                    "detail": str(error),
                }
            )
    state_machine_plans = {
        f"{source['asset_path']}:{suffix.rsplit('.', 1)[0]}": plan
        for suffix, plan in fsm_plans.items()
    }
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
        )
        if entry["kind"] == "graph":
            graph_value["pose_renderer"] = pose_renderer_status(
                value,
                graph_value,
                assets,
                registry["sync_group_phase_keys"],
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
    supported_sync_nodes = supported_sync_group_nodes(graphs, assets, registry)
    unsupported = [
        issue
        for issue in unsupported
        if not (
            issue.get("kind") == "source_sync_group_unsupported"
            and issue.get("source_node") in supported_sync_nodes
        )
    ]
    for dependency in source_reachability["external_dependencies"]:
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
        "function_dependencies": source_reachability["function_dependencies"],
        "external_dependencies": source_reachability["external_dependencies"],
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
    return {
        "format": INTEGRATION_REPORT_FORMAT,
        "source": report.get("source", {}),
        "conversion_status": "ready" if report.get("pass") else "blocked",
        "handoff_action": "integrate_generated_assets"
        if report.get("pass")
        else "repair_conversion_blockers",
        "external_runtime_functions": external_functions,
        "animation_blueprint_functions": animation_blueprint_functions,
        "unhandled_property_bindings": property_bindings,
        "external_graph_dependencies": external_dependencies,
        "other_blockers": [
            issue
            for issue in report.get("unsupported", [])
            if isinstance(issue, dict)
            and issue.get("kind") != "source_property_binding_unhandled"
        ],
    }


def integration_blocker_summary(value):
    summary = []
    functions = sorted(
        {
            item["source_function"]
            for item in value["external_runtime_functions"]
            if item["status"] == "requires_target_implementation"
        }
    )
    if functions:
        summary.append("external runtime functions: " + ", ".join(functions))
    groups = {}
    for item in value["other_blockers"]:
        kind = item.get("kind")
        if not isinstance(kind, str) or not kind:
            continue
        detail = item.get("source_signature") or item.get("detail")
        groups.setdefault(kind, set()).add(str(detail) if detail else "")
    for kind, details in sorted(groups.items()):
        values = sorted(detail for detail in details if detail)
        summary.append(kind + (": " + ", ".join(values) if values else ""))
    paths = sorted(
        {
            item["source_property_path"]
            for item in value["unhandled_property_bindings"]
            if isinstance(item.get("source_property_path"), str)
        }
    )
    if paths:
        summary.append("unhandled property bindings: " + ", ".join(paths))
    return summary


def integration_report_markdown(value):
    if value["conversion_status"] == "blocked":
        lines = [
            "# Animation graph conversion blocked",
            "",
            "Do not integrate generated RON files.",
            "Implement or map every blocker below, then rerun until Conversion status: ready.",
            "",
            "## Required fixes",
            "",
        ]
        summary = integration_blocker_summary(value)
        lines.extend("- " + item for item in summary)
        if not summary:
            lines.append("- Inspect conversion.report.json for the blocking source location.")
        return "\n".join(lines) + "\n"
    lines = [
        "# Animation graph integration report",
        "",
        "Conversion status: " + value["conversion_status"],
        "",
        "## External runtime functions",
        "",
    ]
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
):
    ir, unsupported = build_ir(
        source,
        root_graph,
        registry,
        asset_root,
        source_content_root,
        source_mount,
        source_reader,
    )
    report = build_report(source_identity, ir, unsupported)
    return ir, report


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


def expected_materialized_files(source, ir, registry, report):
    if unsupported_failure(report):
        return {}
    try:
        return materialize_files(source, ir, registry)
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
            "node_classes": {"/Script/AnimGraph.AnimGraphNode_CallFunction": {"renderer": "anim_call"}},
            "function_rules": [
                {
                    "source_class": "/Script/AnimGraph.AnimGraphNode_CallFunction",
                    "signature": "InitializeLean",
                    "renderer": "pose_lifecycle_call",
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
    binding_registry = validate_registry(
        {
            "format": "als_ref_to_bevy_animgraph_mapping_registry_v1",
            "node_classes": {"/Script/AnimGraph.AnimGraphNode_MultiWayBlend": {"renderer": "multi_way_blend"}},
            "function_rules": [],
            "property_binding_rules": [
                {
                    "source_path": "GroundedState.VelocityBlend.ForwardAmount",
                    "target_key": "als_{asset_layer}_velocity_forward_amount",
                },
                {
                    "source_path": "GroundedState.RotationYawOffsets.ForwardAngle",
                    "target_key": "als_{asset_layer}_curve_rotation_yaw_offset",
                },
                {
                    "source_path": "InAirState.GroundPredictionAmount",
                    "target_key": "als_{asset_layer}_ground_prediction",
                },
                {
                    "source_path": "InAirState.JumpPlayRate",
                    "target_key": "als_{asset_layer}_jump_play_rate",
                },
                {
                    "source_path": "LeanState.RightAmount",
                    "target_key": "als_{asset_layer}_air_lean_right",
                },
                {
                    "source_path": "LeanState.ForwardAmount",
                    "target_key": "als_{asset_layer}_air_lean_forward",
                },
                {
                    "source_path": "GroundedState.HipsDirection",
                    "target_key": "als_{asset_layer}_hips_direction",
                }
            ],
            "enum_entry_values": {"EAlsHipsDirection": {"LeftBackward": 3.0}},
        }
    )
    binding_node = {
        "object_path": "/Asset:AnimGraph.Multi",
        "class_path": "/Script/AnimGraph.AnimGraphNode_MultiWayBlend",
        "pins": [
            {"name": "X", "direction": "Input", "hidden": False, "linked_to": []},
            {"name": "Y", "direction": "Input", "hidden": False, "linked_to": []},
        ],
    }
    binding_semantic = {
        "renderer": "multi_way_blend",
        "pose_count": 2,
        "desired_alpha_paths": ["GroundedState.VelocityBlend.ForwardAmount", ""],
    }
    assert not resolve_property_bindings(
        "/ALS/ALS/Character/AnimationInstances/Stances/AB_Als_Standing.AB_Als_Standing",
        binding_node,
        binding_semantic,
        binding_registry,
    )
    assert binding_semantic["desired_alpha_keys"] == [
        "als_standing_velocity_forward_amount",
        "",
    ]
    curve_semantic = {
        "renderer": "modify_curve",
        "curve_names": ["RotationYawOffset"],
        "curve_value_paths": ["GroundedState.RotationYawOffsets.ForwardAngle"],
    }
    assert not resolve_property_bindings(
        "/ALS/ALS/Character/AnimationInstances/Stances/AB_Als_Standing.AB_Als_Standing",
        binding_node,
        curve_semantic,
        binding_registry,
    )
    assert curve_semantic["curve_value_keys"] == [
        "als_standing_curve_rotation_yaw_offset"
    ]
    curve_semantic["alpha_path"] = "InAirState.GroundPredictionAmount"
    assert not resolve_property_bindings(
        "/ALS/ALS/Character/AnimationInstances/AB_Als_Locomotion.AB_Als_Locomotion",
        binding_node,
        curve_semantic,
        binding_registry,
    )
    assert curve_semantic["alpha_key"] == "als_locomotion_ground_prediction"
    two_way_semantic = {
        "renderer": "two_way_blend",
        "alpha_path": "InAirState.GroundPredictionAmount",
    }
    assert not resolve_property_bindings(
        "/ALS/ALS/Character/AnimationInstances/AB_Als_Locomotion.AB_Als_Locomotion",
        binding_node,
        two_way_semantic,
        binding_registry,
    )
    assert two_way_semantic["alpha_key"] == "als_locomotion_ground_prediction"
    sequence_semantic = {
        "renderer": "sequence_player",
        "play_rate_path": "InAirState.JumpPlayRate",
        "loop_animation_path": "",
    }
    assert not resolve_property_bindings(
        "/ALS/ALS/Character/AnimationInstances/AB_Als_Locomotion.AB_Als_Locomotion",
        binding_node,
        sequence_semantic,
        binding_registry,
    )
    assert sequence_semantic["play_rate_key"] == "als_locomotion_jump_play_rate"
    enum_semantic = {
        "renderer": "enum_pose_blend",
        "active_enum_path": "GroundedState.HipsDirection",
        "enum_name": "EAlsHipsDirection",
        "selected_enum_entry": "LeftBackward",
    }
    assert not resolve_property_bindings(
        "/ALS/ALS/Character/AnimationInstances/Stances/AB_Als_Standing.AB_Als_Standing",
        binding_node,
        enum_semantic,
        binding_registry,
    )
    assert enum_semantic["active_key"] == "als_standing_hips_direction"
    assert enum_semantic["selected_enum_value"] == 3.0
    binding_semantic["desired_alpha_paths"] = ["GroundedState.Unknown", ""]
    assert resolve_property_bindings(
        "/ALS/ALS/Character/AnimationInstances/Stances/AB_Als_Standing.AB_Als_Standing",
        binding_node,
        binding_semantic,
        binding_registry,
    )[0]["kind"] == "source_property_binding_unresolved"
    binding_node["binding"] = {
        "properties": [
            {
                "name": "PropertyBindings",
                "exported_text": '(("X", (PropertyName="X",PathAsText="GetParent.LeanState.RightAmount",bIsBound=True)),("Y", (PropertyName="Y",PathAsText="GetParent.LeanState.ForwardAmount",bIsBound=True)))',
            }
        ]
    }
    blendspace_semantic = {"renderer": "blendspace_evaluator"}
    assert not resolve_property_bindings(
        "/ALS/ALS/Character/AnimationInstances/AB_Als_Locomotion.AB_Als_Locomotion",
        binding_node,
        blendspace_semantic,
        binding_registry,
    )
    assert blendspace_semantic["input_binding_keys"] == [
        "als_locomotion_air_lean_right",
        "als_locomotion_air_lean_forward",
    ]
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
            "desired_alpha_keys": ["als_standing_velocity_forward_amount"],
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
    blocked_markdown = integration_report_markdown(integration)
    assert "CustomActorFunction" in blocked_markdown
    assert "Do not integrate generated RON files." in blocked_markdown
    assert "## External graph dependencies" not in blocked_markdown
    ready = build_integration_report(
        integration_ir,
        {"source": {"asset": "/Asset"}, "pass": True, "unsupported": []},
    )
    assert ready["conversion_status"] == "ready"
    assert ready["handoff_action"] == "integrate_generated_assets"
    assert "## External graph dependencies" in integration_report_markdown(ready)
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
        "crlf_function_signature_normalized": "PASS",
        "source_property_binding_keys_resolved_or_rejected": "PASS",
        "unhandled_source_property_bindings_rejected": "PASS",
        "source_pin_link_position_property_drift_rejected": "PASS",
        "source_pose_contract_blocker_rejected": "PASS",
        "unimplemented_registered_renderer_rejected": "PASS",
        "transition_renderer_is_callable_from_converter": "PASS",
        "deterministic_ir_drift_rejected": "PASS",
        "unsupported_source_blocks_generation": "PASS",
        "core_does_not_emit_partial_bevy_graphs": "PASS",
        "integration_dependencies_are_reported": "PASS",
        "integration_report_drift_rejected": "PASS",
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
    ir, report = expected_outputs(
        source,
        source_identity,
        args.root_graph,
        registry,
        args.asset_root,
        source_content_root,
        args.mount,
        args.source_reader,
    )
    materialized_files = expected_materialized_files(source, ir, registry, report)
    integration = build_integration_report(ir, report)
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
    else:
        drift = check_outputs(args.output_root, ir, report, materialized_files)
        if drift:
            report["drift"] = drift
            print(json.dumps(report, ensure_ascii=False, indent=2))
            return 2
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
