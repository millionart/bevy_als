import argparse
import importlib.util
import json
import re
import tempfile
from pathlib import Path
from coordinate_basis import SOURCE_LENGTH_TO_RUNTIME_SCALE


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
    return pin["name"]


def scalar_type(pin):
    value = AUDIT.source_pin_type(pin)
    if value == "Bool":
        return "Bool", None
    if value == "F32":
        return "F32", None
    if value == "GameplayTag":
        return "Bool", "UE GameplayTag is read by als_ext::SourceGameplayTagInputNode."
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


def source_property_key(value):
    if not isinstance(value, str) or not value:
        raise ValueError("source property path is invalid")
    if value.startswith("GetParent."):
        value = value[len("GetParent.") :]
    parts = value.split(".")
    if any(re.fullmatch(r"[A-Za-z][A-Za-z0-9_]*", part) is None for part in parts):
        raise ValueError("source property path is invalid")
    return ".".join(
        re.sub(
            r"(?<=[a-z0-9])(?=[A-Z])",
            "_",
            re.sub(r"([A-Z]+)([A-Z][a-z])", r"\1_\2", part),
        ).lower()
        for part in parts
    )


def source_identifier_key(value):
    if not isinstance(value, str) or not value:
        raise ValueError("source identifier is invalid")
    value = re.sub(r"[^A-Za-z0-9.]+", "_", value).strip("._")
    if not value:
        raise ValueError("source identifier is invalid")
    return source_property_key(value)


def named_property(node, name):
    values = [
        value.get("exported_text")
        for value in node.get("properties", [])
        if value.get("name") == name and isinstance(value.get("exported_text"), str)
    ]
    if len(values) != 1:
        raise ValueError("source node property is missing or ambiguous")
    return values[0]


def source_object_reference(value):
    match = re.search(r"'(/[^']+)'", value) if isinstance(value, str) else None
    if match is None:
        raise ValueError("source object reference is invalid")
    return match.group(1)


def anim_getter_config(node):
    if node.get("class_path") != "/Script/AnimGraph.K2Node_AnimGetter":
        return None
    function = named_property(node, "FunctionReference")
    match = re.search(r'MemberName="([A-Za-z][A-Za-z0-9_]*)"', function)
    if match is None or match.group(1) not in {
        "GetInstanceStateWeight",
        "GetInstanceMachineWeight",
        "GetInstanceCurrentStateElapsedTime",
    }:
        raise ValueError("source animation getter is unsupported")
    getter = match.group(1)
    machine = source_object_reference(named_property(node, "SourceNode"))
    state_value = named_property(node, "SourceStateNode")
    state = "" if state_value == "None" else source_object_reference(state_value)
    if (getter == "GetInstanceStateWeight") != bool(state):
        raise ValueError("source animation getter state reference is invalid")
    if ":" not in machine:
        raise ValueError("source animation getter machine reference is invalid")
    asset, machine_suffix = machine.split(":", 1)
    asset_name = asset.rsplit("/", 1)[-1].split(".", 1)[0]
    key = ".".join(
        (
            "anim_getters",
            source_identifier_key(asset_name),
            source_property_key(getter),
            source_identifier_key(machine_suffix),
            *([source_identifier_key(state.split(":", 1)[1])] if state else []),
        )
    )
    outputs = AUDIT.visible_pins(node, "Output")
    if len(outputs) != 1 or outputs[0].get("name") != "ReturnValue" or scalar_type(outputs[0])[0] != "F32":
        raise ValueError("source animation getter output is unsupported")
    return {
        "source_function": getter,
        "source_machine": machine,
        "source_state": state,
        "target_key": key,
        "output": outputs[0]["name"],
    }


def node_for_path(contract, path):
    matches = [node for node in contract["source_graph"]["nodes"] if node.get("object_path") == path]
    if len(matches) != 1:
        raise ValueError("source node is missing or ambiguous")
    return matches[0]


def pin_for_name(node, name):
    matches = [pin for pin in node.get("pins", []) if pin.get("name") == name]
    if len(matches) != 1:
        raise ValueError("source pin is missing or ambiguous")
    return matches[0]


def linked_source(contract, node, pin):
    matches = [
        link["source"]
        for link in contract["links"]
        if link.get("target", {}).get("node_object_path") == node.get("object_path")
        and link.get("target", {}).get("pin_name") == pin.get("name")
    ]
    if len(matches) > 1:
        raise ValueError("source pin has multiple links")
    return matches[0] if matches else None


def gameplay_tag_literal(pin):
    value = pin.get("default_value")
    if value in {"", "None", "()"}:
        return ""
    if not isinstance(value, str):
        raise ValueError("source GameplayTag default is invalid")
    match = re.fullmatch(r'\(TagName="([A-Za-z0-9_.]+)"\)', value)
    if match is None:
        raise ValueError("source GameplayTag default is unsupported")
    return match.group(1)


def gameplay_tag_predicate(node):
    title = node.get("title", "").lower()
    if "is gameplay tag valid" in title:
        return "valid"
    if "not equal (gameplaytag)" in title or "not equal (gameplay tag)" in title:
        return "not_equal"
    if "equal (gameplaytag)" in title or "equal (gameplay tag)" in title:
        return "equal"
    return ""


def gameplay_tag_input_config(contract, node):
    tag_outputs = [
        pin
        for pin in AUDIT.visible_pins(node, "Output")
        if AUDIT.source_pin_type(pin) == "GameplayTag"
    ]
    if not tag_outputs:
        return None
    if (
        not node.get("class_path", "").endswith("K2Node_PropertyAccess")
        or len(tag_outputs) != 1
    ):
        raise ValueError("source GameplayTag producer is unsupported")
    output = tag_outputs[0]
    links = [
        link
        for link in contract["links"]
        if link.get("source", {}).get("node_object_path") == node.get("object_path")
        and link.get("source", {}).get("pin_name") == output.get("name")
    ]
    if len(links) != 1:
        raise ValueError("source GameplayTag output must have one predicate consumer")
    target = node_for_path(contract, links[0]["target"].get("node_object_path"))
    predicate = gameplay_tag_predicate(target)
    target_pin = links[0]["target"].get("pin_name")
    path = property_access_key(node)
    if not path:
        raise ValueError("source GameplayTag property path is unavailable")
    if predicate == "valid" and target_pin == "GameplayTag":
        return {
            "key": source_property_key(path),
            "output": source_pin_label(output),
            "expected_tag": "",
            "operation": "is_valid",
            "source_property_path": path,
        }
    if predicate in {"equal", "not_equal"} and target_pin in {"A", "B"}:
        other_name = "B" if target_pin == "A" else "A"
        other_pin = pin_for_name(target, other_name)
        if linked_source(contract, target, other_pin) is not None:
            raise ValueError("source GameplayTag property comparison is unsupported")
        expected_tag = gameplay_tag_literal(other_pin)
        if not expected_tag:
            raise ValueError("source GameplayTag comparison literal is invalid")
        return {
            "key": source_property_key(path),
            "output": source_pin_label(output),
            "expected_tag": expected_tag,
            "operation": "matches_tag_exact",
            "source_property_path": path,
        }
    if (
        target.get("class_path", "").endswith("AlsAnimGraphNode_GameplayTagsBlend")
        and target_pin == "ActiveTag"
    ):
        return {
            "key": source_property_key(path),
            "output": source_pin_label(output),
            "expected_tag": "",
            "operation": "is_valid",
            "source_property_path": path,
        }
    raise ValueError("source GameplayTag consumer is unsupported")


def gameplay_tag_input_bool_default(contract, node, pin):
    predicate = gameplay_tag_predicate(node)
    if predicate == "valid" and pin.get("name") == "GameplayTag":
        if linked_source(contract, node, pin) is None:
            raise ValueError("source GameplayTag validity literal is unsupported")
        return False
    if predicate in {"equal", "not_equal"} and pin.get("name") in {"A", "B"}:
        other_name = "B" if pin.get("name") == "A" else "A"
        other = pin_for_name(node, other_name)
        if linked_source(contract, node, pin) is not None:
            if linked_source(contract, node, other) is not None:
                raise ValueError("source GameplayTag property comparison is unsupported")
            return False
        if linked_source(contract, node, other) is None:
            raise ValueError("source GameplayTag literal comparison is unsupported")
        if not gameplay_tag_literal(pin):
            raise ValueError("source GameplayTag comparison literal is invalid")
        return True
    raise ValueError("source GameplayTag consumer is unsupported")


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
    if "is gameplay tag valid" in title:
        return "gameplay_tag_valid"
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


def source_property_runtime_scale(key):
    if key and source_property_key(key) in {
        "locomotion_state.speed",
        "in_air_state.vertical_velocity_world_space",
    }:
        return SOURCE_LENGTH_TO_RUNTIME_SCALE
    return 1.0


def pin_payload(contract, node, pin, bindings):
    data_type, _ = scalar_type(pin)
    f32_default, bool_default, source_default = scalar_defaults(pin)
    access_key = property_access_key(node) if pin.get("direction") == "Output" else ""
    global_key = bindings.get(
        pin["name"],
        source_property_key(access_key) if access_key else "",
    )
    if data_type == "F32":
        scale = source_property_runtime_scale(access_key)
        if contract and pin.get("direction") == "Input" and node.get("class_path", "").endswith("K2Node_PromotableOperator"):
            for other in AUDIT.visible_pins(node, "Input"):
                source = linked_source(contract, node, other)
                if source:
                    scale = source_property_runtime_scale(property_access_key(node_for_path(contract, source["node_object_path"])))
                    if scale != 1.0:
                        break
        f32_default *= scale
    if AUDIT.source_pin_type(pin) == "GameplayTag":
        if pin.get("direction") != "Input":
            raise ValueError("source GameplayTag output must use the dedicated adapter node")
        if contract is None:
            raise ValueError("source GameplayTag input needs a graph contract")
        bool_default = gameplay_tag_input_bool_default(contract, node, pin)
        global_key = ""
    return {
        "name": source_pin_label(pin),
        "data_type": data_type,
        "f32_default": f32_default,
        "bool_default": bool_default,
        "source_default": source_default,
        "global_key": global_key,
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


def node_inner(contract, node=None):
    if node is None:
        node = contract
        contract = None
    bindings = AUDIT.source_binding_paths(node)
    inputs = [
        pin_payload(contract, node, pin, bindings)
        for pin in AUDIT.visible_pins(node, "Input")
    ]
    outputs = [
        pin_payload(contract, node, pin, bindings)
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


def node_ron(contract, identifier, node, getter_bindings=None, validate_only=False):
    getter = anim_getter_config(node)
    if getter is not None:
        binding = (getter_bindings or {}).get(node["object_path"])
        return anim_getter_node(identifier, node, getter, binding, validate_only)
    tag_input = gameplay_tag_input_config(contract, node)
    if tag_input is not None:
        inner = "(" + ", ".join(
            (
                f"key: {quoted(tag_input['key'])}",
                f"output: {quoted(tag_input['output'])}",
                f"expected_tag: {quoted(tag_input['expected_tag'])}",
                f"source_property_path: {quoted(tag_input['source_property_path'])}",
                "source_properties: ["
                + ", ".join(quoted(value) for value in source_properties(node))
                + "]",
            )
        ) + ")"
        return (
            f"(id: {quoted(identifier)}, name: {quoted(AUDIT.source_title(node))}, "
            f"inner: {{ \"als_ext::SourceGameplayTagInputNode\": {inner} }}, should_debug: false)"
        )
    return (
        f"(id: {quoted(identifier)}, name: {quoted(AUDIT.source_title(node))}, "
        f"inner: {{ \"als_ext::SourceScalarExpressionNode\": {node_inner(contract, node)} }}, should_debug: false)"
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


def root_output_name(contract):
    root = AUDIT.source_root(contract)
    pins = AUDIT.visible_pins(root, "Input")
    if len(pins) != 1:
        raise ValueError("transition root must expose exactly one input pin")
    return source_pin_label(pins[0])


def direct_bound_root_node(root, pin, binding):
    output = pin_payload(None, root, pin, {})
    output["global_key"] = source_property_key(binding)
    inner = ", ".join(
        (
            "input_pins: []",
            "output_pins: [" + pin_ron(output) + "]",
            'operation: "global"',
            "source_class: " + quoted(root["class_path"]),
            "source_properties: ["
            + ", ".join(quoted(value) for value in source_properties(root))
            + "]",
        )
    )
    return (
        f"(id: {quoted(node_id(root))}, name: {quoted(AUDIT.source_title(root))}, "
        f"inner: {{ \"als_ext::SourceScalarExpressionNode\": ({inner}) }}, should_debug: false)"
    )


def anim_getter_node(identifier, node, config, binding, validate_only):
    if binding is None:
        if not validate_only:
            raise ValueError("source animation getter target is unavailable")
        fsm_node = ""
        state_id = ""
    else:
        fsm_node = binding.get("target_fsm_node")
        state_id = binding.get("target_state_id", "")
        if not isinstance(fsm_node, str) or not fsm_node:
            raise ValueError("source animation getter machine target is unavailable")
        if not isinstance(state_id, str):
            raise ValueError("source animation getter state target is invalid")
    if config["source_function"] == "GetInstanceStateWeight" and not state_id and not validate_only:
        raise ValueError("source animation getter state target is unavailable")
    if config["source_function"] != "GetInstanceStateWeight" and state_id:
        raise ValueError("source machine getter unexpectedly references a state")
    inner = ", ".join(
        (
            "source_function: " + quoted(config["source_function"]),
            "fsm_node: " + quoted(fsm_node),
            "state_id: " + quoted(state_id),
            "output: " + quoted(config["output"]),
        )
    )
    return (
        f"(id: {quoted(identifier)}, name: {quoted(AUDIT.source_title(node))}, "
        f"inner: {{ \"als_ext::SourceAnimationStateGetterNode\": ({inner}) }}, should_debug: false)"
    )


def graph_text(contract, getter_bindings=None, validate_only=False):
    nodes = source_nodes(contract)
    ids = {node["object_path"]: node_id(node) for node in nodes}
    root = AUDIT.source_root(contract)
    root_pins = AUDIT.visible_pins(root, "Input")
    if len(root_pins) != 1:
        raise ValueError("transition root must expose exactly one input pin")
    root_pin = root_pins[0]
    root_is_direct = not nodes
    binding = AUDIT.source_binding_paths(root).get(root_pin["name"])
    root_is_bound = root_is_direct and isinstance(binding, str) and bool(binding)
    passthrough = root_passthrough(contract, root, root_pin) if root_is_direct and not root_is_bound else None
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
    if root_is_bound:
        edges.append(
            f"OutputData({quoted(source_pin_label(root_pin))}): NodeData({quoted(node_id(root))}, {quoted(source_pin_label(root_pin))})"
        )
    elif root_is_direct:
        edges.append(
            f"OutputData({quoted(source_pin_label(root_pin))}): InputData(Passthrough({quoted(passthrough)}))"
        )
    position_rows = [
        f"{quoted(ids[node['object_path']])}: ({float(node['position_x']):.3f}, {float(node['position_y']):.3f})"
        for node in nodes
    ]
    if root_is_bound:
        position_rows.append(
            f"{quoted(node_id(root))}: ({float(root['position_x']):.3f}, {float(root['position_y']):.3f})"
        )
    positions = ", ".join(position_rows)
    root_type = output_type(root_pin)
    root_name = source_pin_label(root_pin)
    input_data = "{}"
    input_order = "{}"
    default_data = "{}"
    if root_is_direct and not root_is_bound:
        input_data = f"{{ Passthrough({quoted(passthrough)}): {root_type} }}"
        input_order = f"{{ Data(Passthrough({quoted(passthrough)})): 0 }}"
        default_data = f"{{ Passthrough({quoted(passthrough)}): {data_default_ron(root_pin)} }}"
    return "(\n" + "\n".join(
        (
            "    nodes: [",
            *[
                f"        {node_ron(contract, ids[node['object_path']], node, getter_bindings, validate_only)},"
                for node in nodes
            ],
            *([f"        {direct_bound_root_node(root, root_pin, binding)},"] if root_is_bound else []),
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


def gameplay_tag_providers(contract):
    rows = []
    for node in source_nodes(contract):
        config = gameplay_tag_input_config(contract, node)
        if config is not None:
            rows.append(
                {
                    "source_node": node["object_path"],
                    "source_property_path": config["source_property_path"],
                    "target_key": config["key"],
                    "operation": config["operation"],
                    "expected_tag": config["expected_tag"],
                    "provider_api": "als_ext::set_gameplay_tag_input",
                    "player_entity": "AnimationGraphPlayer entity",
                    "status": "requires_target_provider",
                }
            )
    return sorted(rows, key=lambda value: value["source_node"])


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
            tag_input = gameplay_tag_input_config(contract, node)
            assertions.append(
                {
                    "source_contains": access_key,
                    "target_contains": (
                        f"key: {quoted(tag_input['key'])}"
                        if tag_input is not None
                        else f"global_key: {quoted(source_property_key(access_key))}"
                    ),
                }
            )
            if tag_input is not None and tag_input["expected_tag"]:
                assertions.append(
                    {
                        "source_contains": f'TagName="{tag_input["expected_tag"]}"',
                        "target_contains": f"expected_tag: {quoted(tag_input['expected_tag'])}",
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
                f"NodeData({quoted(node_id(root))}, {quoted(source_pin_label(pin))})"
                if binding
                else f"Passthrough({quoted(root_passthrough(contract, root, pin))})"
            )
        default = pin.get("default_value")
        if (
            not pin.get("linked_to")
            and isinstance(default, str)
            and default not in {"", "None"}
            and not nodes
        ):
            entry["source_default"] = default
            entry["target_default_contains"] = (
                f"source_default: {quoted(default)}"
                if binding
                else f"Passthrough({quoted(root_passthrough(contract, root, pin))}): {data_default_ron(pin)}"
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


def fixture_anim_getter_contract():
    bool_type = [{"name": "PinCategory", "exported_text": "bool"}]
    float_type = [
        {"name": "PinCategory", "exported_text": "real"},
        {"name": "PinSubCategory", "exported_text": "float"},
    ]
    return {
        "format": "als_ue_visible_animgraph_contract_v2",
        "source_graph": {
            "class_path": "/Script/AnimGraph.AnimationTransitionGraph",
            "nodes": [
                {
                    "object_path": "/Asset:Transition.Result",
                    "node_guid": "20000000-0000-4000-8000-000000000001",
                    "class_path": "/Script/AnimGraph.AnimGraphNode_TransitionResult",
                    "position_x": 0,
                    "position_y": 0,
                    "title": "Result",
                    "pins": [{"name": "Result", "direction": "Input", "hidden": False, "pin_type_properties": bool_type}],
                },
                {
                    "object_path": "/Asset:Transition.Getter",
                    "node_guid": "20000000-0000-4000-8000-000000000002",
                    "class_path": "/Script/AnimGraph.K2Node_AnimGetter",
                    "position_x": -64,
                    "position_y": 0,
                    "title": "State Weight (Move)",
                    "properties": [
                        {"name": "SourceNode", "exported_text": "/Script/AnimGraph.AnimGraphNode_StateMachine'/ALS/ALS/Character/AnimationInstances/AB_Als_Test.AB_Als_Test:AnimGraph.AnimGraphNode_StateMachine_1'"},
                        {"name": "SourceStateNode", "exported_text": "/Script/AnimGraph.AnimStateNode'/ALS/ALS/Character/AnimationInstances/AB_Als_Test.AB_Als_Test:AnimGraph.AnimGraphNode_StateMachine_1.Movement States.AnimStateNode_2'"},
                        {"name": "FunctionReference", "exported_text": "(MemberName=\"GetInstanceStateWeight\")"},
                    ],
                    "pins": [{"name": "ReturnValue", "direction": "Output", "hidden": False, "default_value": "0.0", "pin_type_properties": float_type}],
                },
            ],
        },
        "links": [
            {"source": {"node_object_path": "/Asset:Transition.Getter", "pin_name": "ReturnValue"}, "target": {"node_object_path": "/Asset:Transition.Result", "pin_name": "Result"}},
        ],
    }


def fixture_gameplay_tag_contract():
    bool_type = [{"name": "PinCategory", "exported_text": "bool"}]
    tag_type = [
        {"name": "PinCategory", "exported_text": "struct"},
        {
            "name": "PinSubCategoryObject",
            "exported_text": "/Script/CoreUObject.ScriptStruct'/Script/GameplayTags.GameplayTag'",
        },
    ]
    property_node = lambda path, guid: {
        "object_path": f"/Asset:AnimGraph.{guid}",
        "node_guid": guid,
        "class_path": "/Script/PropertyAccessNode.K2Node_PropertyAccess",
        "position_x": -256,
        "position_y": 0,
        "title": "Property Access",
        "properties": [{"name": "TextPath", "exported_text": path}],
        "pins": [
            {
                "name": "Value",
                "direction": "Output",
                "hidden": False,
                "pin_type_properties": tag_type,
            }
        ],
    }
    return {
        "format": "als_ue_visible_graph_contract_v2",
        "source_graph": {
            "class_path": "/Script/AnimGraph.AnimationTransitionGraph",
            "nodes": [
                {
                    "object_path": "/Asset:AnimGraph.Result",
                    "node_guid": "30000000-0000-4000-8000-000000000001",
                    "class_path": "/Script/AnimGraph.AnimGraphNode_TransitionResult",
                    "position_x": 0,
                    "position_y": 0,
                    "title": "Result",
                    "pins": [
                        {
                            "name": "Result",
                            "direction": "Input",
                            "hidden": False,
                            "pin_type_properties": bool_type,
                        }
                    ],
                },
                property_node("GetParent.Gait", "30000000-0000-4000-8000-000000000002"),
                property_node("GetParent.Gait", "30000000-0000-4000-8000-000000000003"),
                {
                    "object_path": "/Asset:AnimGraph.Valid",
                    "node_guid": "30000000-0000-4000-8000-000000000004",
                    "class_path": "/Script/BlueprintGraph.K2Node_CallFunction",
                    "position_x": -128,
                    "position_y": -32,
                    "title": "Is Gameplay Tag Valid",
                    "pins": [
                        {
                            "name": "GameplayTag",
                            "direction": "Input",
                            "hidden": False,
                            "pin_type_properties": tag_type,
                        },
                        {
                            "name": "ReturnValue",
                            "direction": "Output",
                            "hidden": False,
                            "pin_type_properties": bool_type,
                        },
                    ],
                },
                {
                    "object_path": "/Asset:AnimGraph.Equal",
                    "node_guid": "30000000-0000-4000-8000-000000000005",
                    "class_path": "/Script/BlueprintGraph.K2Node_CallFunction",
                    "position_x": -128,
                    "position_y": 32,
                    "title": "Equal (GameplayTag)",
                    "pins": [
                        {
                            "name": "A",
                            "direction": "Input",
                            "hidden": False,
                            "pin_type_properties": tag_type,
                        },
                        {
                            "name": "B",
                            "direction": "Input",
                            "hidden": False,
                            "default_value": '(TagName="Als.Gait.Walking")',
                            "linked_to": [],
                            "pin_type_properties": tag_type,
                        },
                        {
                            "name": "ReturnValue",
                            "direction": "Output",
                            "hidden": False,
                            "pin_type_properties": bool_type,
                        },
                    ],
                },
                {
                    "object_path": "/Asset:AnimGraph.And",
                    "node_guid": "30000000-0000-4000-8000-000000000006",
                    "class_path": "/Script/BlueprintGraph.K2Node_CommutativeAssociativeBinaryOperator",
                    "position_x": -64,
                    "position_y": 0,
                    "title": "AND Boolean",
                    "pins": [
                        {"name": "A", "direction": "Input", "hidden": False, "pin_type_properties": bool_type},
                        {"name": "B", "direction": "Input", "hidden": False, "pin_type_properties": bool_type},
                        {"name": "ReturnValue", "direction": "Output", "hidden": False, "pin_type_properties": bool_type},
                    ],
                },
            ],
        },
        "links": [
            {"source": {"node_object_path": "/Asset:AnimGraph.30000000-0000-4000-8000-000000000002", "pin_name": "Value"}, "target": {"node_object_path": "/Asset:AnimGraph.Valid", "pin_name": "GameplayTag"}},
            {"source": {"node_object_path": "/Asset:AnimGraph.30000000-0000-4000-8000-000000000003", "pin_name": "Value"}, "target": {"node_object_path": "/Asset:AnimGraph.Equal", "pin_name": "A"}},
            {"source": {"node_object_path": "/Asset:AnimGraph.Valid", "pin_name": "ReturnValue"}, "target": {"node_object_path": "/Asset:AnimGraph.And", "pin_name": "A"}},
            {"source": {"node_object_path": "/Asset:AnimGraph.Equal", "pin_name": "ReturnValue"}, "target": {"node_object_path": "/Asset:AnimGraph.And", "pin_name": "B"}},
            {"source": {"node_object_path": "/Asset:AnimGraph.And", "pin_name": "ReturnValue"}, "target": {"node_object_path": "/Asset:AnimGraph.Result", "pin_name": "Result"}},
        ],
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
        assert "operation: \"less\"" in node_inner(fixture_contract(), fixture_contract()["source_graph"]["nodes"][1])
        assert "output_data: { \"Result\": Bool }" in text
        assert mapping["node_map"][0]["input_pins"][0]["source_default"] == "0.0"
        assert mapping["root_output_pins"] == [{"source": "Result", "target": "Result"}]
        access = {
            "class_path": "/Script/PropertyAccessNode.K2Node_PropertyAccess",
            "properties": [
                {
                    "exported_text": "(PathAsText=\"GetParent.PoseState.GroundedAmount\")",
                }
            ],
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
        assert property_access_key(access) == "GetParent.PoseState.GroundedAmount"
        assert 'global_key: "pose_state.grounded_amount"' in node_inner(None, access)
        speed_contract = fixture_contract()
        speed_access = dict(access, object_path="/Asset:AnimGraph.Speed")
        speed_access["properties"] = [{"name": "TextPath", "exported_text": "GetParent.LocomotionState.Speed"}]
        speed_contract["source_graph"]["nodes"].append(speed_access)
        compare = speed_contract["source_graph"]["nodes"][1]
        compare["pins"][1]["default_value"] = "650.0"
        speed_contract["links"].append({
            "source": {"node_object_path": speed_access["object_path"], "pin_name": "Value"},
            "target": {"node_object_path": compare["object_path"], "pin_name": "A"},
        })
        threshold = pin_payload(speed_contract, compare, compare["pins"][1], {})
        assert threshold["f32_default"] == 6.5 and threshold["source_default"] == "650.0", threshold
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
        assert operation(
            {
                "class_path": "/Script/BlueprintGraph.K2Node_CallFunction",
                "title": "Is Gameplay Tag Valid",
            }
        ) == "gameplay_tag_valid"
        tag_contract = fixture_gameplay_tag_contract()
        tag_text = graph_text(tag_contract)
        assert '"als_ext::SourceGameplayTagInputNode"' in tag_text
        assert 'key: "gait"' in tag_text
        assert 'expected_tag: "Als.Gait.Walking"' in tag_text
        assert 'operation: "gameplay_tag_valid"' in tag_text
        assert gameplay_tag_providers(tag_contract) == [
            {
                "source_node": "/Asset:AnimGraph.30000000-0000-4000-8000-000000000002",
                "source_property_path": "GetParent.Gait",
                "target_key": "gait",
                "operation": "is_valid",
                "expected_tag": "",
                "provider_api": "als_ext::set_gameplay_tag_input",
                "player_entity": "AnimationGraphPlayer entity",
                "status": "requires_target_provider",
            },
            {
                "source_node": "/Asset:AnimGraph.30000000-0000-4000-8000-000000000003",
                "source_property_path": "GetParent.Gait",
                "target_key": "gait",
                "operation": "matches_tag_exact",
                "expected_tag": "Als.Gait.Walking",
                "provider_api": "als_ext::set_gameplay_tag_input",
                "player_entity": "AnimationGraphPlayer entity",
                "status": "requires_target_provider",
            },
        ]
        bound_path = Path("bound.json")
        write_json(root / bound_path, fixture_bound_root_contract())
        bound_text, bound_mapping = generate(
            root,
            bound_path,
            Path("assets/bound.animgraph.ron"),
            Path("maps/bound.visible.json"),
        )
        assert 'global_key: "value"' in bound_text
        assert 'OutputData("bCanEnterTransition"): NodeData("10000000-0000-4000-8000-000000000003", "bCanEnterTransition")' in bound_text
        assert bound_mapping["root_output_pins"] == [
            {
                "source": "bCanEnterTransition",
                "target": "bCanEnterTransition",
                "target_binding": "GetParent.Value",
                "target_root_input": 'NodeData("10000000-0000-4000-8000-000000000003", "bCanEnterTransition")',
                "source_default": "False",
                "target_default_contains": 'source_default: "False"',
            }
        ]
        getter_contract = fixture_anim_getter_contract()
        getter = anim_getter_config(getter_contract["source_graph"]["nodes"][1])
        getter_text = graph_text(
            getter_contract,
            {
                "/Asset:Transition.Getter": {
                    "target_fsm_node": "20000000-0000-4000-8000-000000000003",
                    "target_state_id": "20000000-0000-4000-8000-000000000004",
                }
            },
        )
        assert getter["target_key"] == "anim_getters.ab_als_test.get_instance_state_weight.anim_graph.anim_graph_node_state_machine_1.anim_graph.anim_graph_node_state_machine_1.movement_states.anim_state_node_2"
        assert '"als_ext::SourceAnimationStateGetterNode"' in getter_text
        assert 'fsm_node: "20000000-0000-4000-8000-000000000003"' in getter_text
        assert 'OutputData("Result"): NodeData("20000000-0000-4000-8000-000000000002", "ReturnValue")' in getter_text
        getter_node = getter_contract["source_graph"]["nodes"][1]
        getter_node["properties"][1]["exported_text"] = "None"
        getter_node["properties"][2]["exported_text"] = '(MemberName="GetInstanceCurrentStateElapsedTime")'
        elapsed = anim_getter_config(getter_node)
        assert elapsed["source_state"] == "" and elapsed["source_function"] == "GetInstanceCurrentStateElapsedTime"
        elapsed_text = graph_text(getter_contract, {getter_node["object_path"]:{"target_fsm_node":"20000000-0000-4000-8000-000000000003", "target_state_id":""}})
        assert 'source_function: "GetInstanceCurrentStateElapsedTime"' in elapsed_text and 'state_id: ""' in elapsed_text
    return {
        "transition_scalar_node_generation": "PASS",
        "source_pin_default_and_type_mapping": "PASS",
        "source_link_and_root_output_mapping": "PASS",
        "property_access_path_becomes_target_key": "PASS",
        "bound_transition_result_root_is_preserved": "PASS",
        "animation_state_getter_target_is_materialized": "PASS",
        "gameplay_tag_and_boolean_function_operations_mapped": "PASS",
        "gameplay_tag_adapter_reads_real_tag_provider": "PASS",
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
