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
    return {"format": FORMAT, "source_asset": asset, "roots": roots, "function_library": library,
        "graphs": graphs, "views": views, "variables": variables,
        "rig_hierarchy": copy.deepcopy(source.get("rig_hierarchy", {})),
        "unit_defaults_complete": unit_defaults.keys() == unit_paths,
        "runtime_ready": False, "unimplemented_functions": functions}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--uasset", type=Path, required=True)
    parser.add_argument("--content-root", type=Path)
    parser.add_argument("--mount", default="/ALS")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    from ue_animgraph_converter import ROOT, ConversionError, read_uasset_source, write_json
    try:
        source_record = args.output.with_suffix(".ue.json")
        if args.output.exists() or source_record.exists():
            raise ControlRigContractError("source record or output already exists")
        source, _ = read_uasset_source(args.uasset, args.content_root, args.mount,
            ROOT / "tools/run_ue_source_reader.ps1", 300, 150)
        write_json(source_record, source)
        result = compile_contract(source)
        write_json(args.output, result)
        report = {"format": REPORT_FORMAT, "success": True, "runtime_ready": False,
            "unit_defaults_complete": result["unit_defaults_complete"],
            "graphs": len(result["graphs"]), "nodes": sum(len(graph["nodes"]) for graph in result["graphs"]),
            "unimplemented_functions": result["unimplemented_functions"]}
    except (ConversionError, KeyError, OSError, ValueError) as error:
        report = {"format": REPORT_FORMAT, "success": False, "error": str(error), "runtime_ready": False}
    write_json(args.report, report)
    print(json.dumps(report))
    return 0 if report["success"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
