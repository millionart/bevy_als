import argparse
import copy
import json
import sys
from tempfile import TemporaryDirectory
from pathlib import Path
from unittest.mock import patch

from ue_control_rig_contract import ControlRigContractError, compile_contract
from ue_control_rig_target import compile_target, main as target_main


def read(result, ref):
    value = result["program"]["initial_values"][ref["register"]]
    for index in ref["path"]:
        kind, fields = next(iter(value.items()))
        value = fields[index] if kind == "List" else {"F64": fields[index]}
    return value


def validate(contract, result):
    program = result["program"]
    assert len(program["instructions"]) == 165
    assert len(result["bones"]) == 79 and len(result["curves"]) == 43
    assert all(value is None for value in result["curves"].values())
    for native, target in zip(contract["rig_hierarchy"]["bones"], result["bones"]):
        for key in ("initial_local", "initial_global"):
            location = native[key + "_transform"]["translation"]
            assert target[key]["translation"] == [location["x"] * 0.01, location["z"] * 0.01, location["y"] * 0.01]
    assert program["entries"] == {"Forwards Solve": 0, "Construction": 150}
    for pc in (31, 40):
        trace = program["instructions"][pc]["Call"]
        for index, centimeters in ((2, 50.0), (3, 80.0), (5, 13.5), (8, 0.0)):
            assert read(result, trace["arguments"][index]) == {"Centimeters": centimeters}
        assert trace["outputs"] == [8, 9]
    foot_minimum = program["instructions"][48]["Math"]
    for reference in [*foot_minimum["inputs"], foot_minimum["output"]]:
        assert read(result, reference) == {"Centimeters": 0.0}
    spring = program["instructions"][54]["Call"]
    for index in (0, 3, 5, 8, 9, 10):
        assert read(result, spring["arguments"][index]) == {"Centimeters": 0.0}
    for pc in (83, 113):
        location = program["instructions"][pc]["Call"]
        for index, centimeters in ((3, 0.0), (4, 0.0), (5, 0.0), (6, 50.0), (15, 0.0)):
            assert read(result, location["arguments"][index]) == {"Centimeters": centimeters}
        assert read(result, location["arguments"][14]) == {"List": [{"Centimeters": 0.0}, {"Centimeters": 0.0}, {"Bool": False}]}
    scale = program["instructions"][87]["Math"]
    assert scale["operation"] == "VectorScale"
    assert read(result, scale["inputs"][1]) == {"Centimeters": 40.0}
    for pc in (98, 128):
        rotation = program["instructions"][pc]["Call"]
        assert read(result, rotation["arguments"][4]) == {"List": [{"F32": 20.0}, {"F32": -40.0}]}
        assert read(result, rotation["arguments"][11]) == {"Vector": [0.0, 1.0, 0.0]}
    assert program["instructions"][83]["Call"]["arguments"][14] != program["instructions"][113]["Call"]["arguments"][14]
    assert read(result, program["instructions"][21]["Call"]["arguments"][2]) == {"List": []}
    assert program["instructions"][19]["Copy"]["output"]["path"] == [1]
    renamed = copy.deepcopy(contract)
    renamed["source_asset"] = "/Fixture/Renamed.Renamed"
    for graph in renamed["graphs"]:
        for node in graph["nodes"]:
            node["source"] = {"path": "renamed", "title": "renamed"}
    for instruction in renamed["program"]["instructions"]:
        instruction["source"]["call_path"] = "renamed"
    assert compile_target(renamed) == result
    changed = copy.deepcopy(contract)
    native_ref = changed["program"]["instructions"][31]["operands"][2]
    changed["program"]["memories"]["Literal"][native_ref["register"]]["default_text"] = "60.000000"
    modified = compile_target(changed)
    assert read(modified, modified["program"]["instructions"][31]["Call"]["arguments"][2]) == {"Centimeters": 60.0}
    changed = copy.deepcopy(contract)
    rig_input = next(row for row in changed["program"]["memories"]["External"] if row["cpp_type"] == "FAlsControlRigInput")
    next(field for field in rig_input["fields"] if field["name"] == "SpineYawAngle")["exported_text"] = "23.0"
    modified = compile_target(changed)
    rig_value = read(modified, modified["externals"]["RigInput"])
    assert rig_value["List"][5] == {"F32": -23.0}
    failures = []
    changed = copy.deepcopy(contract)
    changed["program"]["functions"][0]["name"] = "Unknown::Execute"
    failures.append((changed, "unit is unsupported"))
    changed = copy.deepcopy(contract)
    changed["program"]["functions"][15]["arguments"][2]["cpp_type"] = "double"
    failures.append((changed, "ABI does not match"))
    changed = copy.deepcopy(contract)
    changed["program"]["memories"]["Work"][0]["fields"].pop()
    failures.append((changed, "struct fields do not match"))
    changed = copy.deepcopy(contract)
    changed["rig_hierarchy"].pop("elements")
    failures.append((changed, "hierarchy inventory"))
    changed = copy.deepcopy(contract)
    changed["program"]["instructions"][86]["operands"][5] = copy.deepcopy(changed["program"]["instructions"][86]["operands"][7])
    failures.append((changed, "units conflict"))
    for channel in ["ECC_Camera", "ECC_Pawn", "ECC_Unknown"]:
        changed = copy.deepcopy(contract)
        channel_ref = changed["program"]["instructions"][31]["operands"][1]
        assert channel_ref["memory"] == "Literal"
        changed["program"]["memories"]["Literal"][channel_ref["register"]]["default_text"] = channel
        failures.append((changed, "trace channel"))
    changed = copy.deepcopy(contract)
    channel_ref = changed["program"]["instructions"][31]["operands"][1]
    row = copy.deepcopy(changed["program"]["memories"]["Literal"][channel_ref["register"]])
    channel_ref["memory"] = "Work"
    channel_ref["register"] = len(changed["program"]["memories"]["Work"])
    changed["program"]["memories"]["Work"].append(row)
    failures.append((changed, "constant trace channel"))
    for changed, message in failures:
        try:
            compile_target(changed)
        except ControlRigContractError as error:
            assert message in str(error), str(error)
            with TemporaryDirectory() as folder:
                source_path = Path(folder) / "contract.json"
                output_path = Path(folder) / "target.json"
                source_path.write_text(json.dumps(changed), encoding="utf-8")
                with patch.object(sys, "argv", ["ue_control_rig_target", "--contract", str(source_path), "--output", str(output_path)]):
                    try:
                        target_main()
                    except ControlRigContractError as cli_error:
                        assert message in str(cli_error)
                    else:
                        raise AssertionError("invalid target CLI completed")
                assert not output_path.exists(), "Rejected target CLI wrote a loadable partial asset"
        else:
            raise AssertionError("invalid target contract was accepted: " + message)


def validate_assembly(root, source, result):
    import ue_animgraph_converter as converter
    from extract_ue_visible_graph_contract import contract
    from ue_animgraph_pose_contract import control_rig_contract
    from ue_animgraph_pose_renderer import PoseRenderError, rendered_inner

    graph = contract(root, "AnimGraph")
    node = next(node for node in graph["source_graph"]["nodes"] if node["class_path"].endswith(".AnimGraphNode_ControlRig"))
    semantic, issues = control_rig_contract(node)
    assert not issues, issues

    def assemble(semantic, native):
        with patch.object(converter, "uasset_for_generated_anim_blueprint", return_value=Path("fixture.uasset")) as resolve, patch.object(
                converter, "read_uasset_source", return_value=(native, {})):
            assembled = converter.resolve_control_rig_program(semantic, None, None, None)
            assert resolve.call_args.args[0] == semantic["rig_asset"]
            return {**semantic, **assembled}

    assembled = assemble(semantic, source)
    assert assembled["definition"] == result
    assert [binding["property"] for binding in assembled["input_bindings"]] == ["control_rig_input." + field for field in (
        "b_use_hand_ik_bones", "b_use_foot_ik_bones", "b_foot_transforms_valid", "velocity_blend_forward_amount",
        "velocity_blend_backward_amount", "spine_yaw_angle", "pelvis_offset_amount", "foot_left_location",
        "foot_left_rotation", "foot_right_location", "foot_right_rotation")]
    assert [binding["path"] for binding in assembled["input_bindings"]] == [[index] for index in range(11)]
    assert all(binding["external"] == "RigInput" for binding in assembled["input_bindings"])
    kind, rendered = rendered_inner(graph, node, {"renderer": "control_rig", "semantic": assembled}, {}, {})
    assert "definition: Some(" in rendered and "input_bindings: [(external:" in rendered
    renamed_path = "/Fixture/RenamedRig.RenamedRig"
    renamed_source = json.loads(json.dumps(source).replace(source["asset_path"], renamed_path))
    renamed_node = json.loads(json.dumps(node).replace(source["asset_path"], renamed_path))
    renamed_node["node_guid"] = "f909857b-7664-4633-8497-36721dfe3831"
    renamed_node["object_path"] = "/Fixture/RenamedRoot:AnimGraph.RenamedNode"
    renamed, issues = control_rig_contract(renamed_node)
    assert not issues and renamed["rig_class"] == "RenamedRig"
    assert assemble(renamed, renamed_source)["definition"] == result
    changed = copy.deepcopy(source)
    register = compile_contract(source)["program"]["instructions"][31]["operands"][2]["register"]
    next(row for row in changed["rig_vm"]["literal"]["properties"] if row["index"] == register)["exported_text"] = "60.0"
    modified = assemble(semantic, changed)["definition"]
    assert read(modified, modified["program"]["instructions"][31]["Call"]["arguments"][2]) == {"Centimeters": 60.0}
    for key in ("definition", "input_bindings"):
        incomplete = {key_: value for key_, value in assembled.items() if key_ != key}
        try:
            rendered_inner(graph, node, {"renderer": "control_rig", "semantic": incomplete}, {}, {})
        except PoseRenderError:
            pass
        else:
            raise AssertionError("incomplete rig assembly was accepted")
    incomplete = {key: value for key, value in source.items() if key != "rig_vm"}
    try:
        assemble(semantic, incomplete)
    except ControlRigContractError as error:
        assert "complete native defaults and execution" in str(error), error
    else:
        raise AssertionError("rig source without execution was accepted")
    return "{" + json.dumps(kind) + ": " + rendered + "}\n"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--native-fixture", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--root-fixture", type=Path)
    args = parser.parse_args()
    source = json.loads(args.native_fixture.read_text(encoding="utf-8"))
    contract = compile_contract(source)
    result = compile_target(contract)
    validate(contract, result)
    if args.root_fixture:
        root = json.loads(args.root_fixture.read_text(encoding="utf-8"))
        args.output.with_suffix(".node.ron").write_text(validate_assembly(root, source, result), encoding="utf-8")
    args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"target_program": "PASS", "instructions": 165}))


if __name__ == "__main__":
    main()
