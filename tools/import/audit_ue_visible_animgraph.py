import argparse
import json
import re
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
sys.path.insert(0, str(ROOT / "tools" / "import"))

from animgraph_context_contract import parse_edges, parse_nodes
from capture_animgraph_ron_topology import parse_positions


FORMAT = "als_ue_visible_animgraph_map_v2"
SOURCE_FORMAT = "als_ue_visible_graph_contract_v2"
COMMENT_POLICIES = {"require", "ignore"}


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def source_nodes(contract):
    return contract["source_graph"]["nodes"]


def is_root(node):
    return node["class_path"].endswith(
        (
            "AnimGraphNode_Root",
            "AnimGraphNode_StateResult",
            "AnimGraphNode_TransitionResult",
        )
    )


def is_comment(node):
    return node["class_path"].endswith("EdGraphNode_Comment")


def source_visible_nodes(contract):
    return [node for node in source_nodes(contract) if not is_root(node) and not is_comment(node)]


def source_root(contract):
    values = [node for node in source_nodes(contract) if is_root(node)]
    if len(values) != 1:
        raise ValueError("source root node is missing or ambiguous")
    return values[0]


def source_property_text(node):
    values = list(node.get("properties", []))
    binding = node.get("binding")
    if isinstance(binding, dict):
        values.extend(binding.get("properties", []))
    return "\n".join(
        property_value.get("exported_text", "")
        for property_value in values
        if isinstance(property_value, dict)
    )


def root_behavior_function_names(node):
    lifecycle_properties = [
        property_value
        for property_value in node.get("properties", [])
        if isinstance(property_value, dict)
        and property_value.get("name")
        in {
            "StateEntryFunction",
            "StateFullyBlendedInFunction",
            "StateExitFunction",
            "StateFullyBlendedOutFunction",
            "InitialUpdateFunction",
            "BecomeRelevantFunction",
            "UpdateFunction",
        }
    ]
    member_names = sorted(
        {
            function_name
            for property_value in lifecycle_properties
            for function_name in re.findall(
                r'MemberName="([^"]+)"', property_value.get("exported_text", "")
            )
            if function_name
        }
    )
    if member_names:
        return member_names
    return sorted(
        {
            function_name
            for property_value in lifecycle_properties
            for function_name in re.findall(
                r'FunctionName="([^"]+)"', property_value.get("exported_text", "")
            )
            if function_name
        }
    )


def visible_pins(node, direction):
    return [
        pin
        for pin in node.get("pins", [])
        if pin.get("direction") == direction and not pin.get("hidden", False)
    ]


def source_title(node):
    return re.sub(r"\r\n?", "\n", node["title"])


def source_pin_label(pin):
    friendly = pin.get("friendly_name")
    return friendly if isinstance(friendly, str) and friendly else pin["name"]


def source_pin_type(pin):
    properties = {
        value.get("name"): value.get("exported_text")
        for value in pin.get("pin_type_properties", [])
        if isinstance(value, dict)
    }
    category = properties.get("PinCategory")
    subcategory = properties.get("PinSubCategory")
    object_path = properties.get("PinSubCategoryObject")
    if isinstance(object_path, str) and object_path.endswith("Engine.PoseLink'"):
        return "Pose"
    if category == "real" and subcategory == "float":
        return "F32"
    if category == "bool":
        return "Bool"
    if isinstance(object_path, str) and "GameplayTag" in object_path:
        return "GameplayTag"
    if isinstance(object_path, str) and object_path not in {"", "None"}:
        return object_path
    if isinstance(category, str) and category:
        return category
    return "Unknown"


def source_binding_paths(node):
    binding = node.get("binding")
    if not isinstance(binding, dict):
        return {}
    result = {}
    for prop in binding.get("properties", []):
        if prop.get("name") != "PropertyBindings" or not isinstance(prop.get("exported_text"), str):
            continue
        for pin, path in re.findall(r'\("([^"\\]+)",\s*\(.*?PathAsText="([^"]+)"', prop["exported_text"]):
            if pin in result:
                raise ValueError(f"source property binding is duplicated: {node['object_path']} {pin}")
            result[pin] = path
    return result


def target_output_position(text):
    match = re.search(
        r"(?<![A-Za-z0-9_])output_position\s*:\s*\(\s*(-?[0-9.]+)\s*,\s*(-?[0-9.]+)\s*\)",
        text,
    )
    if match is None:
        raise ValueError("target output position is missing")
    return [float(match.group(1)), float(match.group(2))]


def target_positions(text):
    parsed = parse_positions(text, "node_positions")
    if parsed["malformed"]:
        raise ValueError("target node positions are malformed")
    result = {}
    for entry in parsed["entries"]:
        if entry["id"] in result or entry["position"] is None:
            raise ValueError("target node position is invalid")
        result[entry["id"]] = [float(entry["position"][0]), float(entry["position"][1])]
    return result


def pin_map_shape_issues(value, label):
    issues = []
    if not isinstance(value, list):
        return [f"{label} must be an array"]
    sources = set()
    targets = set()
    allowed = {
        "source",
        "target",
        "type_exception",
        "display_name_exception",
        "target_binding",
        "source_default",
        "target_default_contains",
        "target_runtime_path",
        "target_root_input",
    }
    for index, pin in enumerate(value):
        item_label = f"{label}[{index}]"
        if not isinstance(pin, dict) or not set(pin).issubset(allowed):
            issues.append(f"{item_label} is invalid")
            continue
        for key in ("source", "target"):
            if not isinstance(pin.get(key), str) or not pin[key]:
                issues.append(f"{item_label} {key} is invalid")
        source = pin.get("source")
        target = pin.get("target")
        if isinstance(source, str) and source:
            if source in sources:
                issues.append(f"duplicate {label} source pin {source}")
            sources.add(source)
        if isinstance(target, str) and target:
            if target in targets:
                issues.append(f"duplicate {label} target pin {target}")
            targets.add(target)
        for key in (
            "type_exception",
            "display_name_exception",
            "target_binding",
            "source_default",
            "target_default_contains",
        ):
            if key in pin and (not isinstance(pin[key], str) or not pin[key]):
                issues.append(f"{item_label} {key} is invalid")
        if ("source_default" in pin) != ("target_default_contains" in pin):
            issues.append(f"{item_label} default assertion is incomplete")
        if "target_runtime_path" in pin and (
            not isinstance(pin["target_runtime_path"], str)
            or not pin["target_runtime_path"]
        ):
            issues.append(f"{item_label} target_runtime_path is invalid")
        if "target_root_input" in pin and (
            not isinstance(pin["target_root_input"], str)
            or not pin["target_root_input"]
        ):
            issues.append(f"{item_label} target_root_input is invalid")
    return issues


def property_assertion_issues(value, label):
    if value is None:
        return []
    if not isinstance(value, list):
        return [f"{label} must be an array"]
    issues = []
    for index, assertion in enumerate(value):
        item_label = f"{label}[{index}]"
        allowed = {
            "source_contains",
            "target_contains",
            "target_exception",
            "target_runtime_path",
        }
        if not isinstance(assertion, dict) or "source_contains" not in assertion or not set(assertion).issubset(allowed):
            issues.append(f"{item_label} is invalid")
            continue
        if not isinstance(assertion.get("source_contains"), str) or not assertion["source_contains"]:
            issues.append(f"{item_label} source_contains is invalid")
        target_contains = assertion.get("target_contains")
        target_exception = assertion.get("target_exception")
        if (isinstance(target_contains, str) and bool(target_contains)) == (
            isinstance(target_exception, str) and bool(target_exception)
        ):
            issues.append(f"{item_label} requires exactly one target assertion")
        if target_exception and "target_runtime_path" not in assertion:
            issues.append(f"{item_label} target_exception requires target_runtime_path")
        if "target_runtime_path" in assertion and (
            not isinstance(assertion["target_runtime_path"], str)
            or not assertion["target_runtime_path"]
        ):
            issues.append(f"{item_label} target_runtime_path is invalid")
    return issues


def map_shape_issues(mapping):
    issues = []
    if mapping.get("format") != FORMAT:
        issues.append("visible graph map format is invalid")
    for key in ("source_contract", "target_graph", "target_spec"):
        if not isinstance(mapping.get(key), str) or not mapping[key]:
            issues.append(f"{key} is invalid")
    nodes = mapping.get("node_map")
    if not isinstance(nodes, list):
        issues.append("node_map must be an array")
    else:
        sources = set()
        targets = set()
        required = {"source_node", "target_node", "input_pins", "output_pins"}
        allowed = required | {"property_assertions"}
        for index, value in enumerate(nodes):
            label = f"node_map[{index}]"
            if not isinstance(value, dict) or not required.issubset(value) or not set(value).issubset(allowed):
                issues.append(f"{label} is invalid")
                continue
            source = value["source_node"]
            target = value["target_node"]
            if not isinstance(source, str) or not source:
                issues.append(f"{label} source_node is invalid")
            elif source in sources:
                issues.append(f"duplicate source_node {source}")
            else:
                sources.add(source)
            if not isinstance(target, str) or not target:
                issues.append(f"{label} target_node is invalid")
            elif target in targets:
                issues.append(f"duplicate target_node {target}")
            else:
                targets.add(target)
            issues.extend(pin_map_shape_issues(value["input_pins"], f"{label}.input_pins"))
            issues.extend(pin_map_shape_issues(value["output_pins"], f"{label}.output_pins"))
            issues.extend(property_assertion_issues(value.get("property_assertions"), f"{label}.property_assertions"))
    issues.extend(pin_map_shape_issues(mapping.get("root_output_pins"), "root_output_pins"))
    root_property_assertions = mapping.get("root_property_assertions")
    if root_property_assertions is not None:
        issues.extend(
            property_assertion_issues(
                root_property_assertions, "root_property_assertions"
            )
        )
    require_root_property_assertions = mapping.get(
        "require_root_property_assertions", False
    )
    if not isinstance(require_root_property_assertions, bool):
        issues.append("require_root_property_assertions is invalid")
    elif require_root_property_assertions and root_property_assertions is None:
        issues.append("root_property_assertions is required")
    if not isinstance(mapping.get("comment_groups"), list):
        issues.append("comment_groups must be an array")
    policy = mapping.get("comment_policy", "require")
    if policy not in COMMENT_POLICIES:
        issues.append("comment_policy is invalid")
    if policy == "ignore" and (
        not isinstance(mapping.get("comment_exception"), str) or not mapping["comment_exception"]
    ):
        issues.append("comment_exception is required when comment_policy is ignore")
    return issues


def target_spec_issues(spec, target_text):
    issues = []
    if not isinstance(spec, dict):
        return ["target spec is invalid"]
    source_text = spec.get("graph_source")
    if not isinstance(source_text, str) or source_text.replace("\r\n", "\n").replace("\r", "\n") != target_text.replace("\r\n", "\n").replace("\r", "\n"):
        issues.append("target spec does not match target graph text")
    nodes = spec.get("nodes")
    if not isinstance(nodes, list):
        issues.append("target spec nodes are invalid")
    else:
        seen = set()
        for index, node in enumerate(nodes):
            if not isinstance(node, dict):
                issues.append(f"target spec node[{index}] is invalid")
                continue
            node_id = node.get("id")
            if not isinstance(node_id, str) or not node_id:
                issues.append(f"target spec node[{index}] id is invalid")
            elif node_id in seen:
                issues.append(f"target spec node id is duplicated: {node_id}")
            else:
                seen.add(node_id)
            for key in ("name", "display_name"):
                if not isinstance(node.get(key), str):
                    issues.append(f"target spec node[{index}] {key} is invalid")
            for direction in ("inputs", "outputs"):
                if not isinstance(node.get(direction), list):
                    issues.append(f"target spec node[{index}] {direction} is invalid")
    if not isinstance(spec.get("graph_outputs"), list):
        issues.append("target spec graph_outputs are invalid")
    return issues


def pin_map_by_source(pin_maps):
    return {value["source"]: value for value in pin_maps}


def spec_pin_by_name(values):
    return {value["name"]: value for value in values}


def pin_report(source_pin, target_pin, mapping):
    return {
        "source": source_pin["name"],
        "source_label": source_pin_label(source_pin),
        "source_type": source_pin_type(source_pin),
        "target": target_pin["name"],
        "target_type": target_pin.get("data_type"),
        "target_binding": mapping.get("target_binding"),
        "source_default": mapping.get("source_default"),
        "target_default_contains": mapping.get("target_default_contains"),
    }


def runtime_text(root, mapping):
    path_value = mapping.get("target_runtime_path")
    if not isinstance(path_value, str) or not path_value:
        return None
    path = root / path_value
    if not path.is_file():
        return None
    return path.read_text(encoding="utf-8")


def audit_pins(
    root,
    source,
    target_node,
    target_spec_node,
    pin_maps,
    direction,
    differences,
    allow_runtime_evidence=False,
):
    source_pins = visible_pins(source, direction)
    map_by_source = pin_map_by_source(pin_maps)
    source_by_name = {pin["name"]: pin for pin in source_pins}
    spec_key = "inputs" if direction == "Input" else "outputs"
    target_by_name = spec_pin_by_name(target_spec_node[spec_key])
    missing_source = sorted(set(source_by_name) - set(map_by_source))
    unknown_source = sorted(set(map_by_source) - set(source_by_name))
    mapped_targets = {value["target"] for value in pin_maps}
    missing_target = sorted(set(target_by_name) - mapped_targets)
    unknown_target = sorted(mapped_targets - set(target_by_name))
    prefix = "input" if direction == "Input" else "output"
    if missing_source:
        differences.append({f"missing_source_{prefix}_pins": {"node": source["object_path"], "pins": missing_source}})
    if unknown_source:
        differences.append({f"unknown_source_{prefix}_pins": {"node": source["object_path"], "pins": unknown_source}})
    if missing_target:
        differences.append({f"unmapped_target_{prefix}_pins": {"node": target_node["id"], "pins": missing_target}})
    if unknown_target:
        differences.append({f"unknown_target_{prefix}_pins": {"node": target_node["id"], "pins": unknown_target}})
    source_order = [pin["name"] for pin in source_pins]
    expected_target_order = [map_by_source[name]["target"] for name in source_order if name in map_by_source]
    target_order = [pin["name"] for pin in target_spec_node[spec_key]]
    if expected_target_order != target_order:
        differences.append(
            {
                f"{prefix}_pin_order": {
                    "node": source["object_path"],
                    "source_mapped_target_order": expected_target_order,
                    "target_order": target_order,
                }
            }
        )
    bindings = source_binding_paths(source)
    rows = []
    for source_name, value in map_by_source.items():
        source_pin = source_by_name.get(source_name)
        target_pin = target_by_name.get(value["target"])
        if source_pin is None or target_pin is None:
            continue
        row = pin_report(source_pin, target_pin, value)
        if target_pin.get("name") != source_pin_label(source_pin) and "display_name_exception" not in value:
            differences.append({f"{prefix}_pin_display_name": row})
        if target_pin.get("data_type") != source_pin_type(source_pin) and "type_exception" not in value:
            differences.append({f"{prefix}_pin_type": row})
        source_binding = bindings.get(source_name)
        if source_binding is not None:
            target_binding = value.get("target_binding")
            runtime = runtime_text(root, value) if allow_runtime_evidence else None
            binding_is_visible = (
                isinstance(target_binding, str) and target_binding in target_node["entry"]
            )
            binding_is_runtime = (
                isinstance(target_binding, str) and runtime is not None and target_binding in runtime
            )
            if target_binding is None or not (binding_is_visible or binding_is_runtime):
                differences.append({f"{prefix}_pin_binding": {**row, "source_binding": source_binding}})
        source_default = source_pin.get("default_value")
        unlinked = not source_pin.get("linked_to")
        requires_default_assertion = (
            unlinked
            and isinstance(source_default, str)
            and source_default not in {"", "None"}
        )
        mapped_default = value.get("source_default")
        if requires_default_assertion and mapped_default is None:
            differences.append({f"{prefix}_pin_default_assertion_missing": row})
        if mapped_default is not None and mapped_default != source_default:
            differences.append(
                {
                    f"{prefix}_pin_default_source": {
                        **row,
                        "source_default_actual": source_default,
                    }
                }
            )
        target_default = value.get("target_default_contains")
        runtime = runtime_text(root, value) if allow_runtime_evidence else None
        default_is_visible = isinstance(target_default, str) and target_default in target_node["entry"]
        default_is_runtime = isinstance(target_default, str) and runtime is not None and target_default in runtime
        if requires_default_assertion and target_default is None:
            differences.append({f"{prefix}_pin_default_target_assertion_missing": row})
        if target_default is not None and not (default_is_visible or default_is_runtime):
            differences.append(
                {
                    f"{prefix}_pin_default_target": {
                        **row,
                        "target_entry": target_node["entry"],
                    }
                }
            )
        rows.append(row)
    return rows


def audit_properties(root, source, target_node, assertions, differences):
    source_text = source_property_text(source)
    target_text = target_node["entry"]
    rows = []
    for assertion in assertions or []:
        row = {
            "source_node": source["object_path"],
            "target_node": target_node["id"],
            **assertion,
        }
        if assertion["source_contains"] not in source_text:
            differences.append({"source_property_assertion_missing": row})
        runtime_path = assertion.get("target_runtime_path")
        if runtime_path is not None:
            runtime_file = root / runtime_path
            target_text_for_assertion = (
                runtime_file.read_text(encoding="utf-8")
                if runtime_file.is_file()
                else ""
            )
        else:
            target_text_for_assertion = target_text
        if runtime_path is not None and not target_text_for_assertion:
            differences.append({"target_property_runtime_path_missing": row})
        if "target_exception" not in assertion and assertion["target_contains"] not in target_text_for_assertion:
            differences.append({"target_property_assertion_missing": row})
        rows.append(row)
    return rows


def edge_key(edge):
    return (
        edge["source"]["kind"],
        edge["source"].get("node"),
        edge["source"].get("pin"),
        edge["source"].get("raw"),
        edge["target"]["kind"],
        edge["target"].get("node"),
        edge["target"].get("pin"),
    )


def edge_description(edge):
    return {"source": edge["source"], "target": edge["target"]}


def endpoint_mapping(endpoint, source_root_path, node_map, output_pins, direction):
    node_path = endpoint["node_object_path"]
    pin_name = endpoint["pin_name"]
    if node_path == source_root_path:
        if direction != "Input":
            raise ValueError("source root direction is invalid")
        mapping = pin_map_by_source(output_pins).get(pin_name)
        if mapping is None:
            raise ValueError(f"source root pin is unmapped: {pin_name}")
        return {"kind": "output_data", "node": None, "pin": mapping["target"]}
    node = node_map.get(node_path)
    if node is None:
        raise ValueError(f"source link node is unmapped: {node_path}")
    pin_maps = node["output_pins"] if direction == "Output" else node["input_pins"]
    mapping = pin_map_by_source(pin_maps).get(pin_name)
    if mapping is None:
        raise ValueError(f"source link pin is unmapped: {node_path} {pin_name}")
    return {"kind": "node_data", "node": node["target_node"], "pin": mapping["target"]}


def expected_edges(contract, node_map, output_pins):
    root_path = source_root(contract)["object_path"]
    result = [
        {
            "source": endpoint_mapping(link["source"], root_path, node_map, output_pins, "Output"),
            "target": endpoint_mapping(link["target"], root_path, node_map, output_pins, "Input"),
        }
        for link in contract["links"]
    ]
    root = source_root(contract)
    mapped = pin_map_by_source(output_pins)
    for pin in visible_pins(root, "Input"):
        mapping = mapped.get(pin["name"])
        root_input = mapping.get("target_root_input") if mapping is not None else None
        if not pin.get("linked_to") and isinstance(root_input, str) and root_input:
            result.append(
                {
                    "source": {"kind": "other", "raw": f"InputData({root_input})"},
                    "target": {
                        "kind": "output_data",
                        "node": None,
                        "pin": mapping["target"],
                    },
                }
            )
    return result


def target_edges(text):
    values, _ = parse_edges(text)
    return [{"source": edge["source"], "target": edge["target"]} for edge in values]


def audit(root, mapping):
    issues = map_shape_issues(mapping)
    report = {"issues": issues}
    if issues:
        report["pass"] = False
        return report
    source_path = root / mapping["source_contract"]
    target_path = root / mapping["target_graph"]
    target_spec_path = root / mapping["target_spec"]
    missing_paths = [
        value
        for value, path in (
            (mapping["source_contract"], source_path),
            (mapping["target_graph"], target_path),
            (mapping["target_spec"], target_spec_path),
        )
        if not path.is_file()
    ]
    if missing_paths:
        report["issues"] = [f"required path is missing: {value}" for value in missing_paths]
        report["pass"] = False
        return report
    contract = read_json(source_path)
    if contract.get("format") != SOURCE_FORMAT:
        report["issues"] = ["source contract format is invalid"]
        report["pass"] = False
        return report
    target_text = target_path.read_text(encoding="utf-8")
    spec = read_json(target_spec_path)
    spec_issues = target_spec_issues(spec, target_text)
    if spec_issues:
        report["issues"] = spec_issues
        report["pass"] = False
        return report
    target_by_id, target_ordered = parse_nodes(target_text)
    target_position_by_id = target_positions(target_text)
    target_spec_by_id = {node["id"]: node for node in spec["nodes"]}
    source_visible = source_visible_nodes(contract)
    source_comments = [node for node in source_nodes(contract) if is_comment(node)]
    source_by_path = {node["object_path"]: node for node in source_visible}
    node_map = {value["source_node"]: value for value in mapping["node_map"]}
    target_ids = {node["id"] for node in target_ordered}
    mapped_source = set(node_map)
    mapped_target = {value["target_node"] for value in node_map.values()}
    differences = []
    missing_source = sorted(set(source_by_path) - mapped_source)
    unknown_source = sorted(mapped_source - set(source_by_path))
    missing_target = sorted(target_ids - mapped_target)
    unknown_target = sorted(mapped_target - target_ids)
    if missing_source:
        differences.append({"missing_source_nodes": missing_source})
    if unknown_source:
        differences.append({"unknown_source_nodes": unknown_source})
    if missing_target:
        differences.append({"unmapped_target_nodes": missing_target})
    if unknown_target:
        differences.append({"unknown_target_nodes": unknown_target})
    expected_node_order = [
        node_map[node["object_path"]]["target_node"]
        for node in source_visible
        if node["object_path"] in node_map
    ]
    actual_node_order = [node["id"] for node in target_ordered]
    if expected_node_order != actual_node_order:
        differences.append(
            {
                "node_order": {
                    "source_mapped_target_order": expected_node_order,
                    "target_order": actual_node_order,
                }
            }
        )
    mapped_nodes = []
    for source_path_value, node_mapping in node_map.items():
        source = source_by_path.get(source_path_value)
        target_id = node_mapping["target_node"]
        target = target_by_id.get(target_id)
        target_spec_node = target_spec_by_id.get(target_id)
        if source is None or target is None:
            continue
        if target_spec_node is None:
            differences.append({"target_node_spec_missing": target_id})
            continue
        source_position = [float(source["position_x"]), float(source["position_y"])]
        row = {
            "source_node": source_path_value,
            "source_title": source_title(source),
            "target_node": target_id,
            "target_name": target["name"],
            "source_position": source_position,
            "target_position": target_position_by_id.get(target_id),
        }
        if target["name"] != source_title(source):
            differences.append({"node_name": row})
        if target_spec_node["name"] != target["name"]:
            differences.append({"target_spec_node_name": row})
        if target_position_by_id.get(target_id) != source_position:
            differences.append({"node_position": row})
        row["input_pins"] = audit_pins(root, source, target, target_spec_node, node_mapping["input_pins"], "Input", differences)
        row["output_pins"] = audit_pins(root, source, target, target_spec_node, node_mapping["output_pins"], "Output", differences)
        row["property_assertions"] = audit_properties(
            root,
            source,
            target,
            node_mapping.get("property_assertions"),
            differences,
        )
        mapped_nodes.append(row)
    source_root_node = source_root(contract)
    source_output = [float(source_root_node["position_x"]), float(source_root_node["position_y"])]
    target_output = target_output_position(target_text)
    if source_output != target_output:
        differences.append({"output_position": {"source": source_output, "target": target_output}})
    root_rows = audit_pins(
        root,
        source_root_node,
        {"id": "Output", "entry": target_text},
        {"inputs": spec["graph_outputs"], "outputs": []},
        mapping["root_output_pins"],
        "Input",
        differences,
        allow_runtime_evidence=True,
    )
    root_property_assertions = mapping.get("root_property_assertions", [])
    root_property_rows = audit_properties(
        root,
        source_root_node,
        {"id": "Output", "entry": target_text},
        root_property_assertions,
        differences,
    )
    if mapping.get("require_root_property_assertions", False):
        for function_name in root_behavior_function_names(source_root_node):
            if not any(
                function_name in assertion.get("source_contains", "")
                for assertion in root_property_assertions
            ):
                differences.append(
                    {
                        "root_lifecycle_property_assertion_missing": {
                            "source_root": source_root_node["object_path"],
                            "function_name": function_name,
                        }
                    }
                )
    try:
        expected = expected_edges(contract, node_map, mapping["root_output_pins"])
    except ValueError as error:
        differences.append({"source_link_mapping": str(error)})
        expected = []
    actual = target_edges(target_text)
    expected_by_key = {edge_key(edge): edge for edge in expected}
    actual_by_key = {edge_key(edge): edge for edge in actual}
    missing_edges = [edge_description(expected_by_key[key]) for key in sorted(set(expected_by_key) - set(actual_by_key))]
    extra_edges = [edge_description(actual_by_key[key]) for key in sorted(set(actual_by_key) - set(expected_by_key))]
    if missing_edges:
        differences.append({"missing_source_links": missing_edges})
    if extra_edges:
        differences.append({"extra_target_links": extra_edges})
    if source_comments and mapping.get("comment_policy", "require") == "require":
        differences.append(
            {
                "missing_comment_groups": [
                    {"source_node": node["object_path"], "comment": node["comment"], "position": [node["position_x"], node["position_y"]]}
                    for node in source_comments
                ]
            }
        )
    report.update(
        {
            "source_contract": mapping["source_contract"],
            "target_graph": mapping["target_graph"],
            "target_spec": mapping["target_spec"],
            "source_visible_nodes": len(source_visible),
            "target_nodes": len(target_ordered),
            "source_comments": len(source_comments),
            "target_comments": 0,
            "comment_policy": mapping.get("comment_policy", "require"),
            "ignored_source_comments": source_comments if mapping.get("comment_policy", "require") == "ignore" else [],
            "source_links": len(contract["links"]),
            "target_edges": len(actual),
            "mapped_nodes": mapped_nodes,
            "root_output_pins": root_rows,
            "root_property_assertions": root_property_rows,
            "differences": differences,
            "pass": not differences,
        }
    )
    return report


def run_self_test():
    pose_type = [{"name": "PinSubCategoryObject", "exported_text": "/Script/Engine.PoseLink'"}]
    float_type = [
        {"name": "PinCategory", "exported_text": "real"},
        {"name": "PinSubCategory", "exported_text": "float"},
    ]
    contract = {
        "format": SOURCE_FORMAT,
        "source_graph": {
            "nodes": [
                {"object_path": "/Asset:AnimGraph.Root", "class_path": "/Script/AnimGraph.AnimGraphNode_Root", "position_x": 0, "position_y": 0, "title": "Output Pose\nAnimGraph", "properties": [{"name": "StateExitFunction", "exported_text": "(MemberName=\"source_state_exit\")"}], "binding": {"properties": [{"name": "PropertyBindings", "exported_text": "((\"Result\", (PathAsText=\"source_binding\")))"}]}, "pins": [{"name": "Result", "direction": "Input", "hidden": False, "default_value": "False", "linked_to": [], "pin_type_properties": pose_type}]},
                {"object_path": "/Asset:AnimGraph.Source", "class_path": "/Script/AnimGraph.AnimGraphNode_Test", "position_x": -128, "position_y": 0, "title": "Source", "binding": None, "properties": [{"exported_text": "source_scalar=0"}], "pins": [{"name": "Pose", "direction": "Output", "hidden": False, "pin_type_properties": pose_type}]},
                {"object_path": "/Asset:AnimGraph.Target", "class_path": "/Script/AnimGraph.AnimGraphNode_Test", "position_x": -64, "position_y": 0, "title": "Target", "binding": None, "pins": [{"name": "Source", "direction": "Input", "hidden": False, "pin_type_properties": pose_type}, {"name": "Alpha", "direction": "Input", "hidden": False, "default_value": "0.0", "linked_to": [], "pin_type_properties": float_type}, {"name": "Pose", "direction": "Output", "hidden": False, "pin_type_properties": pose_type}]},
            ]
        },
        "links": [
            {"source": {"node_object_path": "/Asset:AnimGraph.Source", "pin_name": "Pose"}, "target": {"node_object_path": "/Asset:AnimGraph.Target", "pin_name": "Source"}},
            {"source": {"node_object_path": "/Asset:AnimGraph.Target", "pin_name": "Pose"}, "target": {"node_object_path": "/Asset:AnimGraph.Root", "pin_name": "Result"}},
        ],
    }
    graph = (
        "(\n"
        'nodes: [(id: "source", name: "Source", inner: { "als_ext::LayeredGraphNode": (graph: "graphs/source.animgraph.ron") }, should_debug: false), (id: "target", name: "Target", inner: { "als_ext::LayeredGraphNode": (graph: "graphs/target.animgraph.ron") }, should_debug: false)],\n'
        'edges_inverted: { NodeData("target", "Source"): NodeData("source", "Pose"), OutputData("Result"): NodeData("target", "Pose") },\n'
        'io_spec: (input_times: [], input_data: {}, output_time: false, output_data: { "Result": Pose }, input_order: {}, output_order: { Data("Result"): 0 }),\n'
        "default_data: {},\n"
        'editor_metadata: (node_positions: { "source": (-128.0, 0.0), "target": (-64.0, 0.0) }, input_position: (-256.0, 0.0), output_position: (0.0, 0.0), input_param_order: {}, input_time_order: {}, output_data_order: {}, output_pose_order: {}),\n'
        ")\n"
    )
    spec = {
        "graph": "graph.animgraph.ron",
        "graph_source": graph,
        "nodes": [
            {"id": "source", "name": "Source", "display_name": "Source", "inputs": [], "outputs": [{"name": "Pose", "data_type": "Pose"}]},
            {"id": "target", "name": "Target", "display_name": "Target", "inputs": [{"name": "Source", "data_type": "Pose"}, {"name": "Alpha", "data_type": "F32"}], "outputs": [{"name": "Pose", "data_type": "Pose"}]},
        ],
        "graph_outputs": [{"name": "Result", "data_type": "Pose"}],
    }
    mapping = {
        "format": FORMAT,
        "source_contract": "contract.json",
        "target_graph": "graph.animgraph.ron",
        "target_spec": "spec.json",
        "node_map": [
            {"source_node": "/Asset:AnimGraph.Source", "target_node": "source", "input_pins": [], "output_pins": [{"source": "Pose", "target": "Pose"}], "property_assertions": [{"source_contains": "source_scalar=0", "target_contains": "graph: \"graphs/source.animgraph.ron\""}, {"source_contains": "source_scalar=0", "target_contains": "runtime_binding", "target_runtime_path": "runtime.rs"}]},
            {"source_node": "/Asset:AnimGraph.Target", "target_node": "target", "input_pins": [{"source": "Source", "target": "Source"}, {"source": "Alpha", "target": "Alpha", "source_default": "0.0", "target_default_contains": "graph: \"graphs/target.animgraph.ron\""}], "output_pins": [{"source": "Pose", "target": "Pose"}]},
        ],
        "root_output_pins": [{"source": "Result", "target": "Result", "target_binding": "runtime_binding", "source_default": "False", "target_default_contains": "runtime_default_false", "target_runtime_path": "runtime.rs"}],
        "root_property_assertions": [{"source_contains": "MemberName=\"source_state_exit\"", "target_contains": "source_state_exit", "target_runtime_path": "runtime.rs"}],
        "require_root_property_assertions": True,
        "comment_groups": [],
        "comment_policy": "require",
    }
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        (root / "contract.json").write_text(json.dumps(contract), encoding="utf-8")
        (root / "graph.animgraph.ron").write_text(graph, encoding="utf-8")
        (root / "spec.json").write_text(json.dumps(spec), encoding="utf-8")
        (root / "runtime.rs").write_text("runtime_binding runtime_default_false source_state_exit", encoding="utf-8")
        assert audit(root, mapping)["pass"]
        missing_root_lifecycle = json.loads(json.dumps(mapping))
        missing_root_lifecycle["root_property_assertions"] = []
        assert not audit(root, missing_root_lifecycle)["pass"]
        (root / "runtime.rs").unlink()
        assert not audit(root, mapping)["pass"]
        (root / "runtime.rs").write_text("runtime_binding runtime_default_false source_state_exit", encoding="utf-8")
        broken = dict(mapping)
        broken["node_map"] = mapping["node_map"][:-1]
        assert not audit(root, broken)["pass"]
        broken_property = json.loads(json.dumps(mapping))
        broken_property["node_map"][0]["property_assertions"][0]["target_contains"] = "missing"
        assert not audit(root, broken_property)["pass"]
        exception = json.loads(json.dumps(mapping))
        exception["node_map"][0]["property_assertions"][0] = {
            "source_contains": "source_scalar=0",
            "target_exception": "Fixture source behavior has no target ABI.",
            "target_runtime_path": "runtime.rs",
        }
        assert audit(root, exception)["pass"]
        invalid_exception = json.loads(json.dumps(exception))
        invalid_exception["node_map"][0]["property_assertions"][0].pop("target_runtime_path")
        assert not audit(root, invalid_exception)["pass"]
        reordered = json.loads(json.dumps(spec))
        reordered["nodes"][1]["inputs"].reverse()
        (root / "spec.json").write_text(json.dumps(reordered), encoding="utf-8")
        pin_order = audit(root, mapping)
        assert not pin_order["pass"]
        assert any("input_pin_order" in difference for difference in pin_order["differences"])
        missing_default = json.loads(json.dumps(mapping))
        missing_default["node_map"][1]["input_pins"].pop()
        assert not audit(root, missing_default)["pass"]
        missing_target_default = json.loads(json.dumps(mapping))
        missing_target_default["node_map"][1]["input_pins"][1].pop(
            "target_default_contains"
        )
        assert not audit(root, missing_target_default)["pass"]
        stale = dict(spec)
        stale["graph_source"] = "stale"
        (root / "spec.json").write_text(json.dumps(stale), encoding="utf-8")
        assert not audit(root, mapping)["pass"]
        crlf = dict(spec)
        crlf["graph_source"] = graph.replace("\n", "\r\n")
        (root / "spec.json").write_text(json.dumps(crlf), encoding="utf-8")
        assert audit(root, mapping)["pass"]
    assert is_root({"class_path": "/Script/AnimGraph.AnimGraphNode_TransitionResult"})
    bound_root_contract = {
        "source_graph": {
            "nodes": [
                {
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
                }
            ]
        },
        "links": [],
    }
    bound_output_pins = [
        {
            "source": "bCanEnterTransition",
            "target": "Can Enter Transition",
            "target_root_input": 'Passthrough("GetParent.Value")',
        }
    ]
    expected_bound_edges = expected_edges(bound_root_contract, {}, bound_output_pins)
    actual_bound_edges = target_edges(
        '(edges_inverted: { OutputData("Can Enter Transition"): InputData(Passthrough("GetParent.Value")) },)'
    )
    assert [edge_key(edge) for edge in expected_bound_edges] == [
        edge_key(edge) for edge in actual_bound_edges
    ]
    return {
        "exact_visible_node_pin_link_contract": "PASS",
        "source_property_assertion_rejected": "PASS",
        "unmapped_source_node_rejected": "PASS",
        "pin_order_mismatch_rejected": "PASS",
        "unconnected_pin_default_rejected": "PASS",
        "unconnected_pin_target_default_rejected": "PASS",
        "stale_target_spec_rejected": "PASS",
        "line_ending_equivalent_target_spec_accepted": "PASS",
        "transition_result_root_recognized": "PASS",
        "root_runtime_binding_and_default_evidence_required": "PASS",
        "property_runtime_path_and_exception_supported": "PASS",
        "root_lifecycle_property_assertion_required": "PASS",
        "bound_transition_result_root_edge_accepted": "PASS",
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--map", type=Path)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
        return 0
    if args.map is None or not args.check:
        raise ValueError("map and --check are required")
    root = args.root.resolve()
    mapping = read_json(root / args.map)
    report = audit(root, mapping)
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if report["pass"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
