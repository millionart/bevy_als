import argparse
import copy
import io
import json
import sys
import tempfile
from contextlib import redirect_stdout
from pathlib import Path
from unittest.mock import patch

from ue_control_rig_contract import VM, REPORT_FORMAT, ControlRigContractError, compile_contract, main as contract_main, scalar_default


def fixture():
    asset = "/Fixture/Example.Example"
    objects = []

    def add(path, kind, **fields):
        full = asset + ":" + path
        objects.append({"object_path": full, "class_path": kind,
            "properties": [{"name": name, "exported_text": value} for name, value in fields.items()]})
        return f"{kind}'{full}'"

    def array(values):
        return "(" + ",".join(json.dumps(value) for value in values) + ")"

    def pin(path, direction, value="", kind="float", **fields):
        return add(path, VM + "RigVMPin", Direction=direction, CPPType=kind, DefaultValue=value,
            CPPTypeObjectPath="/Script/RigVM.RigVMExecuteContext" if kind == "FRigVMExecuteContext" else "", **fields)

    def node(path, kind, pins, **fields):
        return add(path, VM + kind, Pins=array(pins), **fields)

    begin = node("Main.Begin", "RigVMUnitNode", [pin("Main.Begin.Execute", "Output", kind="FRigVMExecuteContext")],
        ResolvedFunctionName="FRigUnit_BeginExecution::Execute")
    read_path = "Main.Call.Amount.Injection.Read"
    read_pins = [pin(read_path + ".Variable", "Hidden", "Height", "FName"), pin(read_path + ".Value", "Output")]
    read = node(read_path, "RigVMVariableNode", read_pins)
    injected = add("Main.Call.Amount.Injection", VM + "RigVMInjectionInfo", Node=read,
        bInjectedAsInput="True", InputPin="None", OutputPin=read_pins[1])
    call = node("Main.Call", "RigVMFunctionReferenceNode", [
        pin("Main.Call.Input", "IO", kind="FRigVMExecuteContext"),
        pin("Main.Call.Amount", "Input", "0.000000", InjectionInfos=array([injected])),
        pin("Main.Call.Result", "Output"),
    ], ReferencedFunctionHeader=f'(LibraryPointer=(LibraryNodePath="{asset}:Library.Compute"),Arguments=())')
    write = node("Main.Write", "RigVMVariableNode", [
        pin("Main.Write.Variable", "Hidden", "Height", "FName"),
        pin("Main.Write.Value", "Input"), pin("Main.Write.Execute", "IO", kind="FRigVMExecuteContext"),
    ])
    root_links = [add("Main.Link" + str(index), VM + "RigVMLink", SourcePinPath=start, TargetPinPath=end)
        for index, (start, end) in enumerate([
            ("Begin.Execute", "Call.Input"), ("Call.Input", "Write.Execute"),
            ("Call.Result", "Write.Value"), ("Read.Value", "Call.Amount"),
        ])]
    add("Main", VM + "RigVMGraph", Nodes=array([begin, call, write, read]), Links=array(root_links))
    entry = node("Library.Compute.Body.Entry", "RigVMFunctionEntryNode", [
        pin("Library.Compute.Body.Entry.Input", "IO", kind="FRigVMExecuteContext"),
        pin("Library.Compute.Body.Entry.Amount", "Output"),
    ])
    multiply = node("Library.Compute.Body.Multiply", "RigVMUnitNode", [
        pin("Library.Compute.Body.Multiply.A", "Input"),
        pin("Library.Compute.Body.Multiply.B", "Input", "2.000000"),
        pin("Library.Compute.Body.Multiply.Result", "Output"),
    ], ResolvedFunctionName="FRigVMFunction_MathFloatMul::Execute")
    result = node("Library.Compute.Body.Return", "RigVMFunctionReturnNode", [
        pin("Library.Compute.Body.Return.Input", "IO", kind="FRigVMExecuteContext"),
        pin("Library.Compute.Body.Return.Result", "Input"),
    ])
    body_links = [add("Library.Compute.Body.Link" + str(index), VM + "RigVMLink", SourcePinPath=start, TargetPinPath=end)
        for index, (start, end) in enumerate([
            ("Entry.Input", "Return.Input"), ("Entry.Amount", "Multiply.A"), ("Multiply.Result", "Return.Result"),
        ])]
    body = add("Library.Compute.Body", VM + "RigVMGraph", Nodes=array([entry, multiply, result]), Links=array(body_links))
    function = node("Library.Compute", "RigVMCollapseNode", [
        pin("Library.Compute.Input", "IO", kind="FRigVMExecuteContext"),
        pin("Library.Compute.Amount", "Input"), pin("Library.Compute.Result", "Output"),
    ], ContainedGraph=body)
    add("Library", VM + "RigVMFunctionLibrary", Nodes=array([function]), Links="")
    views = []
    for name, model, names in [("MainView", "", ["Begin", "Call", "Write"]),
            ("Functions", "FunctionLibrary::", ["Compute"]),
            ("ComputeView", "FunctionLibrary::|Compute", ["Entry", "Multiply", "Return"])]:
        add(name, "/Script/ControlRigDeveloper.ControlRigGraph", ModelNodePath=model)
        views.append({"object_path": asset + ":" + name, "name": name, "nodes": [
            {"title": value, "node_guid": str(index), "position_x": index * 80, "position_y": 0,
                "properties": [{"name": "ModelNodePath", "exported_text": value}], "pins": []}
            for index, value in enumerate(names)
        ]})
    return {"asset_path": asset, "asset_class": "/Script/ControlRigDeveloper.ControlRigBlueprint",
        "asset_objects": objects, "graphs": views, "rig_hierarchy": {},
        "asset_properties": [
            {"name": "RigVMClient", "exported_text": '(Models=("Main"),FunctionLibrary="Library")'},
            {"name": "NewVariables", "exported_text": '((VarName="Height",VarType=(PinCategory="real",PinSubCategory="float")))'},
        ], "blueprint_cdo_properties": [{"name": "Height", "cpp_type": "float", "exported_text": "4.000000"}]}


def semantic(value):
    if isinstance(value, dict):
        return {key: semantic(item) for key, item in value.items() if key not in {"source", "source_asset"}}
    if isinstance(value, (list, tuple)):
        return [semantic(item) for item in value]
    return value


def self_test():
    assert scalar_default("uint8", "False") == 0
    assert scalar_default("uint8", "True") == 1
    source = fixture()
    result = compile_contract(source)
    assert semantic(json.loads(json.dumps(result))) == semantic(result)
    assert len(result["graphs"]) == 3
    assert sum(len(graph["nodes"]) for graph in result["graphs"]) == 8
    assert sum(len(graph["links"]) for graph in result["graphs"]) == 7
    assert sum(link["kind"] == "execution" for graph in result["graphs"] for link in graph["links"]) == 3
    call = result["graphs"][0]["nodes"][1]
    assert call["function_graph"] == 2
    assert call["pins"][1]["injections"][0]["node"] == (0, 3)
    assert result["graphs"][0]["nodes"][2]["variable"] == {"name": "Height", "access": "write"}
    assert result["graphs"][0]["nodes"][3]["variable"] == {"name": "Height", "access": "read"}
    assert not result["runtime_ready"] and len(result["unimplemented_functions"]) == 2
    renamed = json.loads(json.dumps(source).replace("/Fixture/Example.Example", "/Another/Renamed.Renamed")
        .replace("Main", "DifferentRoot").replace("Multiply", "RenamedNode").replace("Compute", "OtherFunction"))
    for view in renamed["graphs"]:
        for node in view["nodes"]:
            node["node_guid"] += "-renamed"
    assert semantic(compile_contract(renamed)) == semantic(result)

    def mutate(suffix, field, value):
        changed = copy.deepcopy(source)
        target = next(item for item in changed["asset_objects"] if item["object_path"].endswith(suffix))
        for row in target["properties"]:
            if row["name"] == field:
                row["exported_text"] = value
                break
        else:
            raise AssertionError("fixture mutation field is missing")
        return changed

    changed = compile_contract(mutate(".Multiply.B", "DefaultValue", "3.000000"))
    assert changed["graphs"][2]["nodes"][1]["pins"][1]["scalar_default"] == 3.0
    assert semantic(changed) != semantic(result)
    repeated = mutate("Main.Call.Amount", "InjectionInfos", next(row["exported_text"]
        for item in source["asset_objects"] if item["object_path"].endswith("Main.Call.Amount")
        for row in item["properties"] if row["name"] == "InjectionInfos").replace(")", ',"/Script/RigVMDeveloper.RigVMInjectionInfo\'/Fixture/Example.Example:Main.Call.Amount.Injection\'")'))
    repeated_result = compile_contract(repeated)["graphs"][0]["nodes"][1]["pins"][1]["injections"]
    assert len(repeated_result) == 2 and repeated_result[0] == repeated_result[1]
    for text, expected in [("True", True), ("true", True), ("False", False), ("false", False), ("", None)]:
        changed = mutate(".Multiply.B", "CPPType", "bool")
        for item in changed["asset_objects"]:
            if item["object_path"].endswith(".Multiply.B"):
                next(row for row in item["properties"] if row["name"] == "DefaultValue")["exported_text"] = text
        assert compile_contract(changed)["graphs"][2]["nodes"][1]["pins"][1]["scalar_default"] is expected
    failures = [
        (mutate("Main.Link0", "TargetPinPath", "Missing.Input"), "link endpoint"),
        (mutate(".Multiply.B", "CPPType", "UnsupportedType"), "pin type"),
        (mutate("Main.Call", "ReferencedFunctionHeader", '(LibraryPointer=(LibraryNodePath="/Missing.Function"))'), "function dependency"),
        (mutate("Main.Call.Amount.Injection", "OutputPin", "Missing.Pin"), "injected pin"),
        (mutate("Main.Call.Amount", "Direction", "Output"), "link direction"),
        (mutate(":Functions", "ModelNodePath", "UnknownNamespace"), "visible graph model"),
    ]
    unknown = copy.deepcopy(source)
    next(item for item in unknown["asset_objects"] if item["object_path"].endswith("Main.Begin"))["class_path"] = VM + "UnknownNode"
    failures.append((unknown, "node type"))
    for changed, expected in failures:
        try:
            compile_contract(changed)
        except ControlRigContractError as error:
            assert expected in str(error), str(error)
        else:
            raise AssertionError("invalid source contract was accepted: " + expected)
    with tempfile.TemporaryDirectory() as directory, redirect_stdout(io.StringIO()):
        output = Path(directory) / "result.json"
        report = Path(directory) / "report.json"
        arguments = ["contract", "--uasset", str(Path(directory) / "input.uasset"),
            "--output", str(output), "--report", str(report)]
        with patch.object(sys, "argv", arguments), patch("ue_animgraph_converter.read_uasset_source", return_value=(source, {})) as reader:
            assert contract_main() == 0
            raw_record = output.with_suffix(".ue.json")
            original = raw_record.read_bytes()
            assert json.loads(report.read_text())["format"] == REPORT_FORMAT
            assert contract_main() == 1
            assert reader.call_count == 1 and raw_record.read_bytes() == original
        output = Path(directory) / "invalid.json"
        arguments[arguments.index("--output") + 1] = str(output)
        with patch.object(sys, "argv", arguments), patch("ue_animgraph_converter.read_uasset_source", return_value=(failures[0][0], {})):
            assert contract_main() == 1 and not output.exists()
            assert output.with_suffix(".ue.json").is_file()
            error = json.loads(report.read_text())
            assert not error["success"] and error["error"] and not error["runtime_ready"]
    return {"fixture": "PASS", "identity_independence": "PASS", "source_value_sensitivity": "PASS",
        "missing_dependency_and_unsupported_type": "PASS", "runtime_readiness_is_explicit": "PASS",
        "public_failure_report_and_immutable_source": "PASS"}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--native-fixture", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    report = self_test()
    if args.native_fixture:
        source = json.loads(args.native_fixture.read_text(encoding="utf-8"))
        result = compile_contract(source)
        renamed = json.loads(json.dumps(source).replace(source["asset_path"], "/Fixture/Renamed.Renamed"))
        for view in renamed["graphs"]:
            for node in view["nodes"]:
                node["node_guid"] += "-renamed"
        assert semantic(compile_contract(renamed)) == semantic(result)
        report["native"] = {"graphs": len(result["graphs"]),
            "nodes": sum(len(graph["nodes"]) for graph in result["graphs"]),
            "pins": sum(len(node["pins"]) for graph in result["graphs"] for node in graph["nodes"]),
            "links": sum(len(graph["links"]) for graph in result["graphs"]),
            "injection_references": sum(len(pin["injections"]) for graph in result["graphs"] for node in graph["nodes"] for pin in node["pins"]),
            "injection_objects": len({value["source"]["path"] for graph in result["graphs"] for node in graph["nodes"] for pin in node["pins"] for value in pin["injections"]}),
            "visible_pins": sum(len(node["source"]["pins"]) for view in result["views"] for node in view["nodes"]),
            "native_identity_independence": "PASS"}
        if args.output:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report))


if __name__ == "__main__":
    main()
