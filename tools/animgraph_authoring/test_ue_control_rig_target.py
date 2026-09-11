import argparse
import copy
import json
from pathlib import Path

from ue_control_rig_contract import ControlRigContractError, compile_contract
from ue_control_rig_target import compile_target, f32


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
    trace = program["instructions"][31]["Call"]
    assert read(result, trace["arguments"][2]) == {"F32": f32(0.5)}
    assert read(result, trace["arguments"][3]) == {"F32": f32(0.8)}
    assert trace["outputs"] == [8, 9]
    scale = program["instructions"][87]["Math"]
    assert scale["operation"] == "VectorScale"
    assert read(result, scale["inputs"][1]) == {"F32": f32(0.4)}
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
    assert read(modified, modified["program"]["instructions"][31]["Call"]["arguments"][2]) == {"F32": f32(0.6)}
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
    for changed, message in failures:
        try:
            compile_target(changed)
        except ControlRigContractError as error:
            assert message in str(error), str(error)
        else:
            raise AssertionError("invalid target contract was accepted: " + message)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--native-fixture", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    source = json.loads(args.native_fixture.read_text(encoding="utf-8"))
    contract = compile_contract(source)
    result = compile_target(contract)
    validate(contract, result)
    args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"target_program": "PASS", "instructions": 165}))


if __name__ == "__main__":
    main()
