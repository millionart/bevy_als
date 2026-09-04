import argparse
import importlib.util
import json
import re
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MAP_FORMAT = "als_ue_visible_animgraph_map_v2"


def load_module(name, filename):
    specification = importlib.util.spec_from_file_location(name, filename)
    module = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(module)
    return module


AUDIT = load_module(
    "audit_ue_visible_animgraph",
    ROOT / "tools" / "import" / "audit_ue_visible_animgraph.py",
)


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def write_text(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(value, encoding="utf-8")


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def canonical(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def quoted(value):
    return json.dumps(value, ensure_ascii=False)


def source_nodes(contract):
    return [
        node
        for node in contract["source_graph"]["nodes"]
        if not AUDIT.is_root(node) and not AUDIT.is_comment(node)
    ]


def source_pin_label(pin):
    return AUDIT.source_pin_label(pin)


def scalar_type(pin):
    value = AUDIT.source_pin_type(pin)
    if value == "Bool":
        return "Bool", None
    if value == "F32":
        return "F32", None
    return "F32", f"Source {value} pin is encoded as F32."


def scalar_defaults(pin):
    value = pin.get("default_value")
    source_default = value if isinstance(value, str) else ""
    lowered = source_default.lower()
    if lowered == "true":
        return 0.0, True, source_default
    if lowered == "false":
        return 0.0, False, source_default
    try:
        return float(source_default), False, source_default
    except ValueError:
        return 0.0, False, source_default


def operation(node):
    title = node.get("title", "").lower()
    class_path = node.get("class_path", "")
    if class_path.endswith("K2Node_PromotableOperator"):
        if "<=" in title:
            return "less_equal"
        if ">=" in title:
            return "greater_equal"
        if "<" in title:
            return "less"
        if ">" in title:
            return "greater"
        if "!=" in title:
            return "not_equal"
        if "=" in title:
            return "equal"
    if class_path.endswith("K2Node_CommutativeAssociativeBinaryOperator"):
        if "and" in title:
            return "and"
        if "or" in title:
            return "or"
    if "absolute" in title:
        return "absolute"
    if "not equal (gameplaytag)" in title or "not equal (gameplay tag)" in title:
        return "not_equal"
    if "equal (gameplaytag)" in title or "equal (gameplay tag)" in title:
        return "equal"
    if "to float (boolean)" in title:
        return "bool_to_f32"
    if "not boolean" in title or title.startswith("not"):
        return "not"
    if class_path.endswith("K2Node_Knot"):
        return "passthrough"
    return "global"


def node_id(node):
    value = node.get("node_guid")
    if not isinstance(value, str) or re.fullmatch(r"[0-9a-fA-F-]{36}", value) is None:
        raise ValueError("source node GUID is invalid")
    return value.lower()


def source_properties(node):
    values = list(node.get("properties", []))
    binding = node.get("binding")
    if isinstance(binding, dict):
        values.extend(binding.get("properties", []))
    return [
        value["exported_text"]
        for value in values
        if isinstance(value, dict)
        and isinstance(value.get("exported_text"), str)
        and value["exported_text"]
    ]


def property_access_key(node):
    if not node.get("class_path", "").endswith("K2Node_PropertyAccess"):
        return ""
    for value in source_properties(node):
        match = re.search(r"PathAsText=\"([^\"]+)\"", value)
        if match is not None:
            return match.group(1)
    for value in node.get("properties", []):
        if value.get("name") == "TextPath" and isinstance(value.get("exported_text"), str):
            return value["exported_text"]
    return ""


def pin_payload(node, pin, bindings):
    data_type, _ = scalar_type(pin)
    f32_default, bool_default, source_default = scalar_defaults(pin)
    return {
        "name": source_pin_label(pin),
        "data_type": data_type,
        "f32_default": f32_default,
        "bool_default": bool_default,
        "source_default": source_default,
        "global_key": bindings.get(
            pin["name"],
            property_access_key(node) if pin.get("direction") == "Output" else "",
        ),
    }


def pin_ron(value):
    return "(" + ", ".join(
        (
            f"name: {quoted(value['name'])}",
            f"data_type: {value['data_type']}",
            f"f32_default: {value['f32_default']!r}",
            f"bool_default: {'true' if value['bool_default'] else 'false'}",
            f"source_default: {quoted(value['source_default'])}",
            f"global_key: {quoted(value['global_key'])}",
        )
    ) + ")"


def node_inner(node):
    bindings = AUDIT.source_binding_paths(node)
    inputs = [
        pin_payload(node, pin, bindings)
        for pin in AUDIT.visible_pins(node, "Input")
    ]
    outputs = [
        pin_payload(node, pin, bindings)
        for pin in AUDIT.visible_pins(node, "Output")
    ]
    properties = source_properties(node)
    inner = ", ".join(
        (
            "input_pins: [" + ", ".join(pin_ron(pin) for pin in inputs) + "]",
            "output_pins: [" + ", ".join(pin_ron(pin) for pin in outputs) + "]",
            f"operation: {quoted(operation(node))}",
            f"source_class: {quoted(node['class_path'])}",
            "source_properties: [" + ", ".join(quoted(value) for value in properties) + "]",
        )
    )
    return "(" + inner + ")"


def node_ron(identifier, node):
    return (
        f"(id: {quoted(identifier)}, name: {quoted(AUDIT.source_title(node))}, "
        f"inner: {{ \"als_ext::SourceScalarExpressionNode\": {node_inner(node)} }}, should_debug: false)"
    )


def output_type(pin):
    value, _ = scalar_type(pin)
    return value


def data_default_ron(pin):
    data_type, _ = scalar_type(pin)
    f32_default, bool_default, _ = scalar_defaults(pin)
    if data_type == "Bool":
        return f"Bool({'true' if bool_default else 'false'})"
    return f"F32({f32_default!r})"


def root_passthrough(contract, root, pin):
    bindings = AUDIT.source_binding_paths(root)
    binding = bindings.get(pin["name"])
    if binding:
        return binding
    return "source_transition_default_" + node_id(root).replace("-", "") + "_" + pin["name"]


def graph_text(contract):
    nodes = source_nodes(contract)
    ids = {node["object_path"]: node_id(node) for node in nodes}
    root = AUDIT.source_root(contract)
    root_pins = AUDIT.visible_pins(root, "Input")
    if len(root_pins) != 1:
        raise ValueError("transition root must expose exactly one input pin")
    root_pin = root_pins[0]
    root_is_direct = not nodes
    passthrough = root_passthrough(contract, root, root_pin) if root_is_direct else None
    edges = []
    for link in contract["links"]:
        source = link["source"]
        target = link["target"]
        source_id = ids.get(source["node_object_path"])
        target_id = ids.get(target["node_object_path"])
        if target["node_object_path"] == root["object_path"]:
            if source_id is None:
                raise ValueError("transition root source is invalid")
            edges.append(
                f"OutputData({quoted(source_pin_label(root_pin))}): NodeData({quoted(source_id)}, {quoted(source_pin_label_from_name(nodes, source['node_object_path'], source['pin_name']))})"
            )
        elif source_id is not None and target_id is not None:
            edges.append(
                f"NodeData({quoted(target_id)}, {quoted(source_pin_label_from_name(nodes, target['node_object_path'], target['pin_name']))}): NodeData({quoted(source_id)}, {quoted(source_pin_label_from_name(nodes, source['node_object_path'], source['pin_name']))})"
            )
        else:
            raise ValueError("transition link references an unsupported endpoint")
    if root_is_direct:
        edges.append(
            f"OutputData({quoted(source_pin_label(root_pin))}): InputData(Passthrough({quoted(passthrough)}))"
        )
    positions = ", ".join(
        f"{quoted(ids[node['object_path']])}: ({float(node['position_x']):.3f}, {float(node['position_y']):.3f})"
        for node in nodes
    )
    root_type = output_type(root_pin)
    root_name = source_pin_label(root_pin)
    input_data = "{}"
    input_order = "{}"
    default_data = "{}"
    if root_is_direct:
        input_data = f"{{ Passthrough({quoted(passthrough)}): {root_type} }}"
        input_order = f"{{ Data(Passthrough({quoted(passthrough)})): 0 }}"
        default_data = f"{{ Passthrough({quoted(passthrough)}): {data_default_ron(root_pin)} }}"
    return "(\n" + "\n".join(
        (
            "    nodes: [",
            *[f"        {node_ron(ids[node['object_path']], node)}," for node in nodes],
            "    ],",
            "    edges_inverted: {",
            *[f"        {edge}," for edge in edges],
            "    },",
            "    io_spec: (",
            "        input_times: [],",
            f"        input_data: {input_data},",
            "        output_time: false,",
            f"        output_data: {{ {quoted(root_name)}: {root_type} }},",
            f"        input_order: {input_order},",
            f"        output_order: {{ Data({quoted(root_name)}): 0 }},",
            "    ),",
            f"    default_data: {default_data},",
            "    editor_metadata: (",
            f"        node_positions: {{ {positions} }},",
            "        input_position: (0.0, 0.0),",
            f"        output_position: ({float(root['position_x']):.1f}, {float(root['position_y']):.1f}),",
            "        input_param_order: {},",
            "        input_time_order: {},",
            "        output_data_order: {},",
            "        output_pose_order: {},",
            "    ),",
            ")",
            "",
        )
    )


def source_pin_label_from_name(nodes, path, name):
    node = next((value for value in nodes if value["object_path"] == path), None)
    if node is None:
        raise ValueError("source node is missing")
    pin = next((value for value in node.get("pins", []) if value.get("name") == name), None)
    if pin is None:
        raise ValueError("source pin is missing")
    return source_pin_label(pin)


def map_payload(contract_path, target_graph, target_spec, map_path, contract):
    nodes = source_nodes(contract)
    result = {
        "format": MAP_FORMAT,
        "source_contract": contract_path.as_posix(),
        "target_graph": target_graph.as_posix(),
        "target_spec": target_spec.as_posix(),
        "node_map": [],
        "root_output_pins": [],
        "comment_groups": [],
        "comment_policy": "ignore",
        "comment_exception": "User-approved nonfunctional editor comment omission.",
    }
    for node in nodes:
        bindings = AUDIT.source_binding_paths(node)
        node_map = {
            "source_node": node["object_path"],
            "target_node": node_id(node),
            "input_pins": [],
            "output_pins": [],
        }
        for direction, output_key in (("Input", "input_pins"), ("Output", "output_pins")):
            for pin in AUDIT.visible_pins(node, direction):
                target = source_pin_label(pin)
                entry = {"source": pin["name"], "target": target}
                _, type_exception = scalar_type(pin)
                if type_exception is not None:
                    entry["type_exception"] = type_exception
                if target != pin["name"]:
                    entry["display_name_exception"] = "Source friendly pin label is retained."
                binding = bindings.get(pin["name"])
                if binding:
                    entry["target_binding"] = binding
                default = pin.get("default_value")
                if (
                    not pin.get("linked_to")
                    and isinstance(default, str)
                    and default not in {"", "None"}
                ):
                    entry["source_default"] = default
                    entry["target_default_contains"] = f"source_default: {quoted(default)}"
                node_map[output_key].append(entry)
        assertions = [
            {
                "source_contains": value,
                "target_contains": quoted(value),
            }
            for value in source_properties(node)
        ]
        access_key = property_access_key(node)
        if access_key:
            assertions.append(
                {
                    "source_contains": access_key,
                    "target_contains": f"global_key: {quoted(access_key)}",
                }
            )
        if assertions:
            node_map["property_assertions"] = assertions
        result["node_map"].append(node_map)
    root = AUDIT.source_root(contract)
    for pin in AUDIT.visible_pins(root, "Input"):
        entry = {"source": pin["name"], "target": source_pin_label(pin)}
        _, type_exception = scalar_type(pin)
        if type_exception is not None:
            entry["type_exception"] = type_exception
        binding = AUDIT.source_binding_paths(root).get(pin["name"])
        if binding and not nodes:
            entry["target_binding"] = binding
        if not nodes:
            entry["target_root_input"] = (
                f"Passthrough({quoted(root_passthrough(contract, root, pin))})"
            )
        default = pin.get("default_value")
        if (
            not pin.get("linked_to")
            and isinstance(default, str)
            and default not in {"", "None"}
            and not nodes
        ):
            entry["source_default"] = default
            passthrough = root_passthrough(contract, root, pin)
            entry["target_default_contains"] = (
                f"Passthrough({quoted(passthrough)}): {data_default_ron(pin)}"
            )
        result["root_output_pins"].append(entry)
    return result


def generate(root, source_contract, target_graph, map_path):
    contract = read_json(root / source_contract)
    if contract.get("format") != "als_ue_visible_graph_contract_v2":
        raise ValueError("source contract format is invalid")
    if not contract["source_graph"].get("class_path", "").endswith("AnimationTransitionGraph"):
        raise ValueError("source contract is not a transition graph")
    target_spec = Path("target/bf111/source_specs") / f"{map_path.stem}.json"
    return graph_text(contract), map_payload(
        source_contract,
        target_graph,
        target_spec,
        map_path,
        contract,
    )


def fixture_contract():
    float_type = [
        {"name": "PinCategory", "exported_text": "real"},
        {"name": "PinSubCategory", "exported_text": "float"},
    ]
    bool_type = [{"name": "PinCategory", "exported_text": "bool"}]
    return {
        "format": "als_ue_visible_graph_contract_v2",
        "source_graph": {
            "class_path": "/Script/AnimGraph.AnimationTransitionGraph",
            "nodes": [
                {
                    "object_path": "/Asset:AnimGraph.Result",
                    "node_guid": "10000000-0000-4000-8000-000000000001",
                    "class_path": "/Script/AnimGraph.AnimGraphNode_TransitionResult",
                    "position_x": 0,
                    "position_y": 0,
                    "title": "Result",
                    "pins": [{"name": "Result", "direction": "Input", "hidden": False, "pin_type_properties": bool_type}],
                },
                {
                    "object_path": "/Asset:AnimGraph.Compare",
                    "node_guid": "10000000-0000-4000-8000-000000000002",
                    "class_path": "/Script/BlueprintGraph.K2Node_PromotableOperator",
                    "position_x": -64,
                    "position_y": 0,
                    "title": "float < float",
                    "properties": [{"exported_text": "Operation=Less"}],
                    "pins": [
                        {"name": "A", "direction": "Input", "hidden": False, "default_value": "0.0", "linked_to": [], "pin_type_properties": float_type},
                        {"name": "B", "direction": "Input", "hidden": False, "default_value": "1.0", "linked_to": [], "pin_type_properties": float_type},
                        {"name": "Return Value", "direction": "Output", "hidden": False, "pin_type_properties": bool_type, "linked_to": []},
                    ],
                },
            ],
        },
        "links": [
            {
                "source": {"node_object_path": "/Asset:AnimGraph.Compare", "pin_name": "Return Value"},
                "target": {"node_object_path": "/Asset:AnimGraph.Result", "pin_name": "Result"},
            }
        ],
    }


def fixture_bound_root_contract():
    bool_type = [{"name": "PinCategory", "exported_text": "bool"}]
    return {
        "format": "als_ue_visible_graph_contract_v2",
        "source_graph_suffix": "AnimGraph.BoundTransition",
        "source_graph": {
            "class_path": "/Script/AnimGraph.AnimationTransitionGraph",
            "nodes": [
                {
                    "object_path": "/Asset:AnimGraph.Result",
                    "node_guid": "10000000-0000-4000-8000-000000000003",
                    "class_path": "/Script/AnimGraph.AnimGraphNode_TransitionResult",
                    "position_x": 0,
                    "position_y": 0,
                    "title": "Result",
                    "pins": [
                        {
                            "name": "bCanEnterTransition",
                            "direction": "Input",
                            "hidden": False,
                            "friendly_name": "Can Enter Transition",
                            "default_value": "False",
                            "linked_to": [],
                            "pin_type_properties": bool_type,
                        }
                    ],
                    "binding": {
                        "properties": [
                            {
                                "name": "PropertyBindings",
                                "exported_text": "((\"bCanEnterTransition\", (PathAsText=\"GetParent.Value\")))",
                            }
                        ]
                    },
                }
            ],
        },
        "links": [],
    }


def run_self_test():
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        source_contract = Path("contract.json")
        target_graph = Path("assets/transition.animgraph.ron")
        map_path = Path("maps/transition.visible.json")
        write_json(root / source_contract, fixture_contract())
        text, mapping = generate(root, source_contract, target_graph, map_path)
        assert "operation: \"less\"" in text
        assert "operation: \"less\"" in node_inner(fixture_contract()["source_graph"]["nodes"][1])
        assert "output_data: { \"Result\": Bool }" in text
        assert mapping["node_map"][0]["input_pins"][0]["source_default"] == "0.0"
        assert mapping["root_output_pins"] == [{"source": "Result", "target": "Result"}]
        access = {
            "class_path": "/Script/PropertyAccessNode.K2Node_PropertyAccess",
            "properties": [
                {
                    "exported_text": "(PathAsText=\"GetParent.Value\")",
                }
            ],
        }
        assert property_access_key(access) == "GetParent.Value"
        assert operation(
            {
                "class_path": "/Script/BlueprintGraph.K2Node_CallFunction",
                "title": "Equal (GameplayTag)",
            }
        ) == "equal"
        assert operation(
            {
                "class_path": "/Script/BlueprintGraph.K2Node_CallFunction",
                "title": "To Float (Boolean)",
            }
        ) == "bool_to_f32"
        bound_path = Path("bound.json")
        write_json(root / bound_path, fixture_bound_root_contract())
        bound_text, bound_mapping = generate(
            root,
            bound_path,
            Path("assets/bound.animgraph.ron"),
            Path("maps/bound.visible.json"),
        )
        assert 'InputData(Passthrough("GetParent.Value"))' in bound_text
        assert 'Passthrough("GetParent.Value"): Bool(false)' in bound_text
        assert bound_mapping["root_output_pins"] == [
            {
                "source": "bCanEnterTransition",
                "target": "Can Enter Transition",
                "target_binding": "GetParent.Value",
                "target_root_input": 'Passthrough("GetParent.Value")',
                "source_default": "False",
                "target_default_contains": 'Passthrough("GetParent.Value"): Bool(false)',
            }
        ]
    return {
        "transition_scalar_node_generation": "PASS",
        "source_pin_default_and_type_mapping": "PASS",
        "source_link_and_root_output_mapping": "PASS",
        "property_access_path_becomes_target_key": "PASS",
        "bound_transition_result_root_is_preserved": "PASS",
        "gameplay_tag_and_boolean_function_operations_mapped": "PASS",
        "scalar_node_inner_is_reusable": "PASS",
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--source-contract", type=Path)
    parser.add_argument("--target-graph", type=Path)
    parser.add_argument("--map", type=Path)
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
        return 0
    if (
        args.source_contract is None
        or args.target_graph is None
        or args.map is None
        or args.write == args.check
    ):
        raise ValueError("source-contract, target-graph, map, and exactly one mode are required")
    root = args.root.resolve()
    text, mapping = generate(root, args.source_contract, args.target_graph, args.map)
    target_path = root / args.target_graph
    map_path = root / args.map
    if args.write:
        write_text(target_path, text)
        write_json(map_path, mapping)
        print(json.dumps({"target_graph": args.target_graph.as_posix(), "nodes": len(mapping["node_map"])}, ensure_ascii=False))
        return 0
    issues = []
    if not target_path.is_file() or target_path.read_text(encoding="utf-8") != text:
        issues.append("target graph differs")
    if not map_path.is_file() or canonical(read_json(map_path)) != canonical(mapping):
        issues.append("source map differs")
    print(json.dumps({"issues": issues}, ensure_ascii=False, indent=2))
    return 0 if not issues else 1


if __name__ == "__main__":
    raise SystemExit(main())
