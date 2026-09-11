import argparse
import copy
import json
import re
from pathlib import Path

from ue_exported_text import parse_bool, parse_list, parse_number, parse_quoted, parse_struct


FORMAT = "ue_control_rig_contract_ir_v1"
REPORT_FORMAT = "ue_control_rig_conversion_report_v1"
VM = "/Script/RigVMDeveloper."
NODE_KINDS = {
    "RigVMUnitNode": "unit",
    "RigVMDispatchNode": "dispatch",
    "RigVMVariableNode": "variable",
    "RigVMRerouteNode": "reroute",
    "RigVMFunctionReferenceNode": "call",
    "RigVMCollapseNode": "function",
    "RigVMFunctionEntryNode": "entry",
    "RigVMFunctionReturnNode": "return",
    "RigVMCommentNode": "comment",
}
PRIMITIVES = {"bool", "float", "double", "int32", "uint8", "uint16", "FName"}
MATH_OPERATIONS = {
    **{"FRigVMFunction_MathFloat" + name + "::Execute": ("F32" + name, [("A", "float"), ("B", "float")], "float")
        for name in ("Add", "Mul", "Min")},
    **{"FRigVMFunction_MathDouble" + name + "::Execute": ("F64" + name, [("A", "double"), ("B", "double")], "bool" if name == "GreaterEqual" else "double")
        for name in ("Add", "Sub", "Mul", "GreaterEqual")},
    "FRigVMFunction_MathDoubleAbs::Execute": ("F64Abs", [("Value", "double")], "double"),
    "FRigVMFunction_MathDoubleClamp::Execute": ("F64Clamp", [("Value", "double"), ("Minimum", "double"), ("Maximum", "double")], "double"),
    "FRigVMFunction_MathDoubleLerp::Execute": ("F64Lerp", [("A", "double"), ("B", "double"), ("T", "double")], "double"),
    "FAlsRigVMFunction_Clamp01Float::Execute": ("F32Clamp01", [("Value", "float")], "float"),
    "FRigVMFunction_MathVectorAdd::Execute": ("VectorAdd", [("A", "FVector"), ("B", "FVector")], "FVector"),
    "FRigVMFunction_MathVectorScale::Execute": ("VectorScale", [("Value", "FVector"), ("Factor", "float")], "FVector"),
    "FRigVMFunction_MathQuaternionFromRotatorV2::Execute": ("QuaternionFromEulerYzx", [("Value", "FRotator")], "FQuat"),
}


class ControlRigContractError(ValueError):
    pass


def compile_math_operation(node):
    contract = MATH_OPERATIONS.get(node.get("function"))
    if contract is None:
        return None
    operation, inputs, output_type = contract
    roots = {pin["name"]: pin for pin in node["pins"] if pin["parent"] is None}
    expected = {name: (cpp_type, "Input") for name, cpp_type in inputs}
    expected["Result"] = (output_type, "Output")
    if {name: (pin["cpp_type"], pin["direction"]) for name, pin in roots.items()} != expected:
        raise ControlRigContractError("source math pin schema does not match: " + node["function"])
    return {"operation": operation, "inputs": [roots[name]["id"] for name, _ in inputs], "output": roots["Result"]["id"]}


def properties(record):
    result = {}
    for value in record.get("properties", []):
        if value["name"] in result:
            raise ControlRigContractError("source property is duplicated: " + value["name"])
        result[value["name"]] = value["exported_text"]
    return result


def object_reference(value, asset):
    value = parse_quoted(value) if value.startswith('"') else value
    if value in {"", "None"}:
        return None
    if "'" in value:
        match = re.fullmatch(r"[^']+'([^']+)'", value)
        if match is None:
            raise ControlRigContractError("source object reference is invalid")
        return match[1]
    return value if value.startswith("/") else asset + ":" + value


def scalar_default(cpp_type, raw):
    if not raw or cpp_type not in PRIMITIVES:
        return None
    boolean = {"true": "True", "false": "False"}.get(raw, raw)
    if cpp_type == "bool":
        return parse_bool(boolean)
    if cpp_type == "uint8" and boolean in {"True", "False"}:
        return int(parse_bool(boolean))
    return raw if cpp_type == "FName" else parse_number(raw)


def compile_vm(vm, asset, graphs, variables):
    if not vm.get("capture_complete") or vm.get("callable_count") != 0 or vm.get("predicate_count") != 0:
        raise ControlRigContractError("native RigVM instruction capture is incomplete")

    def cpp_type(value):
        value = re.sub(r"\s+", "", value)
        match = re.fullmatch(r"TEnumAsByte<(.+)>", value)
        return match[1] if match else value

    memories = {}
    for bank in ("Literal", "Work"):
        rows = sorted(vm[bank.lower()]["properties"], key=lambda row: row["index"])
        if [row["index"] for row in rows] != list(range(len(rows))):
            raise ControlRigContractError("native RigVM memory indices are incomplete")
        memories[bank] = [{"cpp_type": row["cpp_type"], "default_text": row["exported_text"],
            "fields": copy.deepcopy(row.get("fields", [])), "elements": copy.deepcopy(row.get("elements", [])),
            "source": {"name": row["name"]}} for row in rows]
    defaults = {row["name"]: row["default"] for row in variables}
    external = vm["external_variables"]
    if len({row["name"] for row in external}) != len(external) or any(row["name"] not in defaults for row in external):
        raise ControlRigContractError("native RigVM external variable binding is incomplete")
    memories["External"] = [{"name": row["name"], "cpp_type": row["cpp_type"],
        "default_text": defaults[row["name"]]["exported_text"], "fields": copy.deepcopy(defaults[row["name"]].get("fields", []))}
        for row in external]
    nodes = {asset + ":" + node["source"]["path"]: (graph["id"], node["id"], node) for graph in graphs for node in graph["nodes"]}

    def reference(value, expected=None, writable=False, private=False):
        bank, index, offset = value["memory"], value["register"], value["offset"]
        if bank not in memories or type(index) is not int or not 0 <= index < len(memories[bank]) or type(offset) is not int or offset < -1:
            raise ControlRigContractError("native RigVM operand is invalid")
        if writable and bank == "Literal":
            raise ControlRigContractError("native RigVM writes to literal memory")
        row = memories[bank][index]
        result = {"memory": bank, "register": index, "path": []}
        actual = cpp_type(row["cpp_type"])
        if offset >= 0:
            paths = vm["external_paths"] if bank == "External" else vm[bank.lower()]["paths"]
            if offset >= len(paths):
                raise ControlRigContractError("native RigVM property path is missing")
            path = paths[offset]
            if cpp_type(path["head_cpp_type"]) != actual or bank == "External" and path["register"] != index:
                raise ControlRigContractError("native RigVM property path owner does not match")
            fields = row["fields"]
            for name in path["path"].split("."):
                selected = [field for field in fields if field["name"] == name]
                if len(selected) != 1:
                    raise ControlRigContractError("native RigVM property path field is missing")
                field = selected[0]
                actual = cpp_type(field["cpp_type"])
                fields = field.get("fields", [])
                result["path"].append(name)
            if actual != cpp_type(path.get("tail_cpp_type", "")):
                raise ControlRigContractError("native RigVM property path type does not match")
        if expected is not None and actual != cpp_type(expected):
            if private and bank == "Work" and not result["path"] and actual == "TArray<" + cpp_type(expected) + ">":
                result["slice"] = 0
                actual = cpp_type(expected)
            else:
                raise ControlRigContractError("native RigVM operand type does not match: " + actual + " -> " + expected)
        return result, actual

    functions = [{"name": row["name"], "arguments": [{"name": arg["name"], "cpp_type": arg["cpp_type"]} for arg in row["arguments"]]}
        for row in vm["functions"]]
    if len({row["name"] for row in functions}) != len(functions):
        raise ControlRigContractError("native RigVM function is duplicated")
    native_ops = vm["instructions"]
    if not native_ops or [row["index"] for row in native_ops] != list(range(len(native_ops))):
        raise ControlRigContractError("native RigVM instruction indices are incomplete")

    def instruction_index(value):
        if type(value) is not int or not 0 <= value < len(native_ops):
            raise ControlRigContractError("native RigVM jump or entry is invalid")
        return value

    instructions = []
    edges = []
    for index, native in enumerate(native_ops):
        opcode = native["opcode"]
        row = {"opcode": opcode}
        successors = [index + 1]
        model = nodes.get(native["subject"])
        if native["subject"] != "None" and model is None:
            raise ControlRigContractError("native RigVM source model is missing")
        row["model"] = list(model[:2]) if model else None
        row["source"] = {"call_path": native["call_path"]}
        if opcode == "Execute":
            function_index = native["function"]
            if type(function_index) is not int or not 0 <= function_index < len(functions) or native.get("predicate_count") != 0:
                raise ControlRigContractError("native RigVM function invocation is invalid")
            function = functions[function_index]
            if model is None or model[2].get("function") != function["name"] or len(native["operands"]) != len(function["arguments"]):
                raise ControlRigContractError("native RigVM function ABI does not match its model")
            pins = {pin["name"]: pin for pin in model[2]["pins"] if pin["parent"] is None}
            operands = []
            for argument, operand in zip(function["arguments"], native["operands"]):
                pin = pins.get(argument["name"])
                if pin is None or cpp_type(pin["cpp_type"]) != cpp_type(argument["cpp_type"]):
                    raise ControlRigContractError("native RigVM function argument does not match its model")
                value, _ = reference(operand, argument["cpp_type"], pin["direction"] in {"Output", "IO", "Hidden"}, pin["direction"] == "Hidden")
                operands.append(value)
            row.update(function=function_index, operands=operands)
        elif opcode == "Copy":
            read, read_type = reference(native["source"])
            write, write_type = reference(native["target"], writable=True)
            if read_type != write_type and {read_type, write_type} != {"float", "double"}:
                raise ControlRigContractError("native RigVM copy conversion is unsupported: " + read_type + " -> " + write_type)
            row.update(input=read, output=write)
        elif opcode == "Zero":
            value, value_type = reference(native["target"], writable=True)
            if value_type not in {"FName", "int32"}:
                raise ControlRigContractError("native RigVM zero type is unsupported")
            row["output"] = value
        elif opcode in {"JumpAbsolute", "JumpForward", "JumpBackward"}:
            offset = native["jump_offset"]
            target = offset if opcode == "JumpAbsolute" else index + offset * (-1 if opcode == "JumpBackward" else 1)
            row.update(opcode="Jump", instruction=instruction_index(target))
            successors = [target]
        elif opcode == "JumpToBranch":
            row["selector"], _ = reference(native["selector"], "FName")
            start = native["first_branch"]
            branches = vm["branches"]
            if type(start) is not int or not 0 <= start < len(branches) or branches[start]["instruction"] != index:
                raise ControlRigContractError("native RigVM branch range is invalid")
            targets = {}
            for branch in branches[start:]:
                if branch["instruction"] != index:
                    break
                if branch["label"] in targets or branch["argument"] != -1 or not branch["first"] - 1 <= branch["last"] < len(native_ops):
                    raise ControlRigContractError("native RigVM branch contract is invalid")
                targets[branch["label"]] = instruction_index(branch["first"])
            if "Completed" not in targets:
                raise ControlRigContractError("native RigVM completed branch is missing")
            row["branches"] = targets
            successors = list(targets.values())
        elif opcode == "Exit":
            successors = []
        else:
            raise ControlRigContractError("native RigVM opcode is unsupported: " + opcode)
        edges.append([instruction_index(value) for value in successors])
        instructions.append(row)
    entries = {row["name"]: instruction_index(row["instruction"]) for row in vm["entries"]}
    if not entries or len(entries) != len(vm["entries"]):
        raise ControlRigContractError("native RigVM entries are missing or duplicated")
    pending = list(entries.values())
    reached = set()
    while pending:
        index = pending.pop()
        if index not in reached:
            reached.add(index)
            pending.extend(edges[index])
    if len(reached) != len(instructions):
        raise ControlRigContractError("native RigVM has unreachable instructions")
    return {"memories": memories, "functions": functions, "instructions": instructions, "entries": entries}


def compile_contract(source):
    if source.get("asset_class") != "/Script/ControlRigDeveloper.ControlRigBlueprint":
        raise ControlRigContractError("source asset is not a Control Rig blueprint")
    asset = source["asset_path"]
    objects = {}
    for record in source["asset_objects"]:
        path = record["object_path"]
        if path in objects:
            raise ControlRigContractError("source object is duplicated: " + path)
        objects[path] = record
    asset_fields = {value["name"]: value["exported_text"] for value in source["asset_properties"]}
    unit_defaults = {}
    for value in source.get("rig_unit_defaults", []):
        path = value["object_path"]
        if path in unit_defaults:
            raise ControlRigContractError("native unit defaults are duplicated: " + path)
        if any(not isinstance(value.get(key), list) for key in ("constructor_properties", "authored_properties")):
            raise ControlRigContractError("native unit default schema is incomplete: " + path)
        constructor_fields = {(field["name"], field.get("array_index", 0)) for field in value["constructor_properties"]}
        authored_fields = {(field["name"], field.get("array_index", 0)) for field in value["authored_properties"]}
        if constructor_fields != authored_fields:
            raise ControlRigContractError("native unit default member sets differ: " + path)
        unit_defaults[path] = value
    unit_paths = {path for path, value in objects.items() if value["class_path"] == VM + "RigVMUnitNode"}
    if "rig_unit_defaults" in source and unit_defaults.keys() != unit_paths:
        raise ControlRigContractError("native unit default closure is incomplete")
    client = parse_struct(asset_fields["RigVMClient"])
    graphs, graph_ids, node_ids, pin_ids, injection_owners, link_ids = [], {}, {}, {}, {}, set()
    root_paths = [object_reference(value, asset) for value in parse_list(client["Models"])]
    library_path = object_reference(client["FunctionLibrary"], asset)

    def record(path, classes=None):
        value = objects.get(path)
        if value is None or classes is not None and value["class_path"] not in classes:
            raise ControlRigContractError("source object is missing or has an unsupported type: " + str(path))
        return value

    def refs(fields, key):
        value = fields.get(key, "")
        return [object_reference(item, asset) for item in parse_list(value)] if value else []

    def identity(path, **values):
        return {"path": path.removeprefix(asset + ":"), **values}

    def pin(path, node, parent=None):
        fields = properties(record(path, {VM + "RigVMPin"}))
        if path in pin_ids:
            raise ControlRigContractError("source pin ownership is duplicated: " + path)
        index = len(node["pins"])
        pin_ids[path] = (node["graph"], node["id"], index)
        cpp_type = fields["CPPType"]
        type_object = fields.get("CPPTypeObjectPath", "")
        if cpp_type not in PRIMITIVES and not type_object.startswith("/Script/"):
            raise ControlRigContractError("source pin type is unsupported: " + cpp_type)
        direction = fields["Direction"]
        if direction not in {"Input", "Output", "IO", "Hidden"}:
            raise ControlRigContractError("source pin direction is unsupported: " + direction)
        raw = fields.get("DefaultValue", "")
        result = {
            "id": index, "parent": parent,
            "name": path[len(node["_path"]) + 1:], "cpp_type": cpp_type,
            "type_object": type_object, "direction": direction,
            "default_text": raw, "scalar_default": scalar_default(cpp_type, raw),
            "default_type": fields.get("DefaultValueType", "AutoDetect"),
            "default_object": fields.get("DefaultValueObject", "None"),
            "constant": parse_bool(fields.get("bIsConstant", "False")),
            "input_variable": parse_bool(fields.get("bIsInputVariable", "False")),
            "lazy": parse_bool(fields.get("bIsLazy", "False")),
            "dynamic_array": parse_bool(fields.get("bIsDynamicArray", "False")),
            "source": identity(path), "children": [], "injections": [],
        }
        node["pins"].append(result)
        result["children"] = [pin(child, node, index) for child in refs(fields, "SubPins")]
        for injected_path in refs(fields, "InjectionInfos"):
            injected = properties(record(injected_path, {VM + "RigVMInjectionInfo"}))
            if injected_path in injection_owners and injection_owners[injected_path] != path:
                raise ControlRigContractError("source injection ownership is duplicated: " + injected_path)
            injection_owners[injected_path] = path
            result["injections"].append({
                "as_input": parse_bool(injected["bInjectedAsInput"]),
                "node": object_reference(injected["Node"], asset),
                "input": object_reference(injected["InputPin"], asset),
                "output": object_reference(injected["OutputPin"], asset),
                "source": identity(injected_path),
            })
        return index

    def graph(path):
        if path in graph_ids:
            return graph_ids[path]
        model = record(path, {VM + "RigVMGraph", VM + "RigVMFunctionLibrary"})
        fields = properties(model)
        if fields.get("DetachedLinks", "") or parse_bool(fields.get("bSectionsEnabled", "False")):
            raise ControlRigContractError("source graph has detached links or execution sections: " + path)
        index = len(graphs)
        graph_ids[path] = index
        result = {"id": index, "source": identity(path), "nodes": [], "links": [],
            "locals": [parse_struct(item) for item in parse_list(fields["LocalVariables"])] if fields.get("LocalVariables") else []}
        graphs.append(result)
        for node_path in refs(fields, "Nodes"):
            native = record(node_path)
            kind = NODE_KINDS.get(native["class_path"].removeprefix(VM))
            if kind is None or not native["class_path"].startswith(VM):
                raise ControlRigContractError("source node type is unsupported: " + native["class_path"])
            if node_path in node_ids:
                raise ControlRigContractError("source node ownership is duplicated: " + node_path)
            node_fields = properties(native)
            for flag in ("bHasEarlyExitMarker", "bIsExcludedByEarlyExit"):
                if parse_bool(node_fields.get(flag, "False")):
                    raise ControlRigContractError("source early exit is unsupported: " + node_path)
            if node_fields.get("TraitRootPinNames", "") or node_fields.get("OrphanedPins", ""):
                raise ControlRigContractError("source traits or orphaned pins are unsupported: " + node_path)
            node_index = len(result["nodes"])
            node_ids[node_path] = (index, node_index)
            node = {"id": node_index, "graph": index, "kind": kind, "pins": [],
                "source": identity(node_path, title=node_fields.get("NodeTitle", ""),
                    position=node_fields.get("Position", ""), size=node_fields.get("Size", "")), "_path": node_path}
            result["nodes"].append(node)
            for pin_path in refs(node_fields, "Pins"):
                pin(pin_path, node)
            if kind in {"unit", "dispatch"}:
                function = node_fields.get("ResolvedFunctionName", "")
                if not function:
                    raise ControlRigContractError("source unit function is unresolved: " + node_path)
                node["function"] = function
                node["template"] = node_fields.get("TemplateNotation", "None")
                math = compile_math_operation(node)
                if math is not None:
                    node["math"] = math
                if kind == "unit" and node_path in unit_defaults:
                    defaults = unit_defaults[node_path]
                    signature = "F" + defaults["script_struct"].rsplit(".", 1)[-1] + "::" + defaults["method"]
                    if signature != function:
                        raise ControlRigContractError("native unit default signature does not match its node: " + node_path)
                    members = {value["name"] for value in defaults["authored_properties"]}
                    if any(value["name"] not in members for value in node["pins"] if value["parent"] is None):
                        raise ControlRigContractError("native unit default pin member is missing: " + node_path)
                    node["native_unit"] = copy.deepcopy({key: value for key, value in defaults.items() if key != "object_path"})
            elif kind == "variable":
                roots = {value["name"]: value for value in node["pins"] if value["parent"] is None}
                if not roots.get("Variable", {}).get("default_text") or "Value" not in roots:
                    raise ControlRigContractError("source variable binding is missing: " + node_path)
                direction = roots["Value"]["direction"]
                if direction not in {"Input", "Output"}:
                    raise ControlRigContractError("source variable access is unsupported: " + node_path)
                node["variable"] = {"name": roots["Variable"]["default_text"],
                    "access": "read" if direction == "Output" else "write"}
            elif kind == "function":
                node["function_graph"] = graph(object_reference(node_fields["ContainedGraph"], asset))
            elif kind == "call":
                header = parse_struct(node_fields["ReferencedFunctionHeader"])
                pointer = parse_struct(header["LibraryPointer"])
                node["function_node"] = object_reference(pointer["LibraryNodePath"], asset)
                node["arguments"] = [parse_struct(value) for value in parse_list(header.get("Arguments", "()"))]
            elif kind == "comment":
                node["source"]["text"] = node_fields.get("CommentText", "")
        lookup = {}
        for node in result["nodes"]:
            for value in node["pins"]:
                key = node["_path"].rsplit(".", 1)[-1] + "." + value["name"]
                if key in lookup:
                    raise ControlRigContractError("source pin alias is ambiguous: " + key)
                lookup[key] = (index, node["id"], value["id"])
        for link_path in refs(fields, "Links"):
            if link_path in link_ids:
                raise ControlRigContractError("source link ownership is duplicated: " + link_path)
            link_ids.add(link_path)
            link = properties(record(link_path, {VM + "RigVMLink"}))
            ends = [lookup.get(link[key]) for key in ("SourcePinPath", "TargetPinPath")]
            if None in ends:
                raise ControlRigContractError("source link endpoint is missing: " + link_path)
            start, end = [graphs[g]["nodes"][n]["pins"][p] for g, n, p in ends]
            if start["direction"] not in {"Output", "IO"} or end["direction"] not in {"Input", "IO"}:
                raise ControlRigContractError("source link direction is invalid: " + link_path)
            execution = start["cpp_type"].endswith("ExecuteContext")
            if execution != end["cpp_type"].endswith("ExecuteContext"):
                raise ControlRigContractError("source link crosses data and execution pins: " + link_path)
            result["links"].append({"from": ends[0], "to": ends[1], "kind": "execution" if execution else "data"})
        return index

    roots = [graph(path) for path in root_paths]
    library = graph(library_path)
    for model in graphs:
        for node in model["nodes"]:
            if node["kind"] == "call":
                target = node_ids.get(node["function_node"])
                if target is None:
                    raise ControlRigContractError("source function dependency is missing: " + node["function_node"])
                definition = graphs[target[0]]["nodes"][target[1]]
                if definition["kind"] != "function":
                    raise ControlRigContractError("source call target is not a function")
                node["function_node"] = target
                node["function_graph"] = definition["function_graph"]
            for value in node["pins"]:
                for injected in value["injections"]:
                    target = node_ids.get(injected["node"])
                    if target is None or target[0] != model["id"]:
                        raise ControlRigContractError("source injected node is missing from its graph")
                    injected["node"] = target
                    for key in ("input", "output"):
                        path = injected[key]
                        if path is not None and path not in pin_ids:
                            raise ControlRigContractError("source injected pin is missing")
                        injected[key] = pin_ids.get(path)

    def dependencies(index, stack):
        if index in stack:
            raise ControlRigContractError("source function recursion is unsupported")
        for node in graphs[index]["nodes"]:
            if node["kind"] == "call":
                dependencies(node["function_graph"], (*stack, index))

    for index in roots:
        dependencies(index, ())
    views = []
    for view in source["graphs"]:
        fields = properties(record(view["object_path"]))
        model_path = fields.get("ModelNodePath", "")
        if model_path.startswith("FunctionLibrary::|"):
            definitions = [node for node in graphs[library]["nodes"]
                if node["_path"].rsplit(".", 1)[-1] == model_path.rsplit("|", 1)[-1]]
            model_id = definitions[0]["function_graph"] if len(definitions) == 1 else None
        elif model_path == "FunctionLibrary::":
            model_id = library
        else:
            model_id = roots[0] if not model_path and len(roots) == 1 else None
        if model_id is None:
            raise ControlRigContractError("source visible graph model is ambiguous: " + view["object_path"])
        names = {node["_path"].rsplit(".", 1)[-1]: node["id"] for node in graphs[model_id]["nodes"]}
        nodes = []
        for native in view["nodes"]:
            model_name = properties(native).get("ModelNodePath", "")
            if model_name and model_name not in names:
                raise ControlRigContractError("source visible node model is missing: " + model_name)
            nodes.append({"model_node": names.get(model_name), "source": {
                "guid": native["node_guid"], "title": native["title"],
                "position": [native["position_x"], native["position_y"]],
                "comment": native.get("comment", ""), "pins": [
                    {**{key: value[key] for key in ("pin_id", "name", "direction", "default_value", "default_object", "linked_to", "parent_pin")},
                        "type": {field["name"]: field["exported_text"] for field in value["pin_type_properties"]}}
                    for value in native["pins"]],
            }})
        views.append({"model_graph": model_id, "source": identity(view["object_path"], title=view["name"]), "nodes": nodes})
    for model in graphs:
        for node in model["nodes"]:
            del node["_path"]
            del node["graph"]
    expected_pins = {path for path, value in objects.items() if value["class_path"] == VM + "RigVMPin"}
    expected_nodes = {path for path, value in objects.items() if value["class_path"] in {VM + key for key in NODE_KINDS}}
    if expected_pins != pin_ids.keys() or expected_nodes != node_ids.keys():
        raise ControlRigContractError("source model closure contains unowned nodes or pins")
    for classes, owned in [({VM + "RigVMInjectionInfo"}, injection_owners),
            ({VM + "RigVMLink"}, link_ids), ({VM + "RigVMGraph", VM + "RigVMFunctionLibrary"}, graph_ids)]:
        if {path for path, value in objects.items() if value["class_path"] in classes} != set(owned):
            raise ControlRigContractError("source model closure contains unowned graphs, links or injections")
    declared = [parse_struct(value) for value in parse_list(asset_fields.get("NewVariables", "()"))]
    cdo = {value["name"]: value for value in source["blueprint_cdo_properties"]}
    variables = [{"name": parse_quoted(value["VarName"]), "type": parse_struct(value["VarType"]),
        "default": cdo[parse_quoted(value["VarName"])]} for value in declared]
    functions = sorted({node["function"] for model in graphs for node in model["nodes"] if "function" in node and "math" not in node})
    result = {"format": FORMAT, "source_asset": asset, "roots": roots, "function_library": library,
        "graphs": graphs, "views": views, "variables": variables,
        "rig_hierarchy": copy.deepcopy(source.get("rig_hierarchy", {})),
        "unit_defaults_complete": unit_defaults.keys() == unit_paths,
        "runtime_ready": False, "unimplemented_functions": functions}
    if "rig_vm" in source:
        result["program"] = compile_vm(source["rig_vm"], asset, graphs, variables)
    result["execution_contract_complete"] = "program" in result
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--uasset", type=Path, required=True)
    parser.add_argument("--content-root", type=Path)
    parser.add_argument("--mount", default="/ALS")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--target-output", type=Path)
    args = parser.parse_args()
    from ue_animgraph_converter import ROOT, ConversionError, read_uasset_source, write_json
    try:
        source_record = args.output.with_suffix(".ue.json")
        if args.output.exists() or source_record.exists():
            raise ControlRigContractError("source record or output already exists")
        if args.target_output and args.target_output.resolve() in {path.resolve() for path in (args.output, source_record, args.report)}:
            raise ControlRigContractError("target output overlaps a source record or report")
        source, _ = read_uasset_source(args.uasset, args.content_root, args.mount,
            ROOT / "tools/run_ue_source_reader.ps1", 300, 150)
        write_json(source_record, source)
        result = compile_contract(source)
        if args.target_output:
            from ue_control_rig_target import compile_target
            target = compile_target(result)
            write_json(args.target_output, target)
            result["runtime_ready"] = True
            result["unimplemented_functions"] = []
        write_json(args.output, result)
        report = {"format": REPORT_FORMAT, "success": True, "runtime_ready": result["runtime_ready"],
            "unit_defaults_complete": result["unit_defaults_complete"],
            "execution_contract_complete": result["execution_contract_complete"],
            "instructions": len(result.get("program", {}).get("instructions", [])),
            "graphs": len(result["graphs"]), "nodes": sum(len(graph["nodes"]) for graph in result["graphs"]),
            "unimplemented_functions": result["unimplemented_functions"]}
    except (ConversionError, KeyError, OSError, ValueError) as error:
        report = {"format": REPORT_FORMAT, "success": False, "error": str(error), "runtime_ready": False}
    write_json(args.report, report)
    print(json.dumps(report))
    return 0 if report["success"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
