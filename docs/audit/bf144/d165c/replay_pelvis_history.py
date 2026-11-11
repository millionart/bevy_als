import gzip
import json
import struct
import subprocess
import sys
from pathlib import Path


def load(path):
    data = path.read_bytes() if path.is_file() else gzip.decompress(path.with_suffix(".json.gz").read_bytes())
    return json.loads(data)


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def fields(value):
    return {field["name"]: field for field in value["fields"]}


def vector(value):
    children = fields(value)
    return [children[axis]["numeric_value"] for axis in "XYZ"]


def point_to_meters(point):
    return [point[index] * 0.01 for index in (0, 2, 1)]


def project_doubles(value, lengths):
    if isinstance(value, list):
        return [project_doubles(child, lengths) for child in value]
    if not isinstance(value, dict):
        return value
    if value.keys() == {"Centimeters64"}:
        lengths.append(value["Centimeters64"])
        return {"F64": value["Centimeters64"] * 0.01}
    return {key: project_doubles(child, lengths) for key, child in value.items()}


directory = Path(__file__).resolve().parent
sys.path.insert(0, str(directory.parents[3] / "tools/animgraph_authoring"))
from ue_control_rig_contract import compile_contract

rig = load(directory.parent / "d165b/rig.native.json")
program = compile_contract(rig)["program"]
native = load(directory / "pelvis-history.native.json")["rig_node_history_probe"]["cases"]
archive = load(directory.parent / "465/inputs.json")["frames"]


def values(frame, pc):
    result = []
    for ref in program["instructions"][pc]["operands"]:
        memory, register = ref["memory"], ref["register"]
        if memory == "External":
            value = frame["externals_after"][program["memories"][memory][register]["name"]]
        else:
            bank = frame["work_after"] if memory == "Work" else rig["rig_vm"]["literal"]
            value = bank["properties"][register]
        if "slice" in ref:
            value = value["elements"][ref["slice"]]
        for field in ref["path"]:
            value = fields(value)[field]
        result.append(value)
    return result


def rig_input(value):
    children = fields(value)
    result = [{"Bool": children[name]["exported_text"] == "True"} for name in (
        "bUseHandIkBones", "bUseFootIkBones", "bFootTransformsValid")]
    result.extend({"F32": children[name]["numeric_value"] * factor} for name, factor in (
        ("VelocityBlendForwardAmount", 1), ("VelocityBlendBackwardAmount", 1), ("SpineYawAngle", -1), ("PelvisOffsetAmount", 1)))
    for foot in ("FootLeft", "FootRight"):
        result.append({"Vector": point_to_meters(vector(children[foot + "Location"]))})
        axes = fields(children[foot + "Rotation"])
        result.append({"Quaternion": [axes[axis]["numeric_value"] * sign for axis, sign in zip("XZYW", (-1, -1, -1, 1), strict=True)]})
    return {"List": result}


histories = []
legacy_writer_mismatches = []
missing_input_weight = 0
missing_writer_weight = 0
consumers = 0
for case in native:
    cases = []
    for query, frame in enumerate(case["frames"]):
        externals = frame["externals_after"]
        visited = next(event["instructions"] for event in frame["executions"] if event["entry"] == "Forwards Solve")
        assert all(visited.count(pc) == 1 for pc in (48, 50, 52, 54, 57, 59, 64, 66))
        inputs = values(frame, 54)
        state = inputs[11]["native_float_spring_state"]
        state = [state["previous_target"], state["velocity"], state["previous_target_valid"]]
        arguments = [value["exported_text"] == "True" if index in (4, 7) else value["numeric_value"]
                     for index, value in enumerate(inputs[:11])] + [state]
        before_weight = values(frame, 52)[1]["numeric_value"]
        after_weight = values(frame, 57)[1]["numeric_value"]
        left = externals["FootLeftOffsetLocationZ"]["numeric_value"]
        right = externals["FootRightOffsetLocationZ"]["numeric_value"]
        before_target = min(max(min(left, right), -30.0), 40.0)
        assert f32(before_target * before_weight) == arguments[0]
        result = arguments[8]
        assert arguments[9] == state[1] and arguments[10] == result
        writer = externals["PelvisOffset"]["numeric_value"]
        assert f32(result * after_weight) == writer
        if f32((result * 0.01) * after_weight * 100.0) != writer:
            legacy_writer_mismatches.append([case["name"], query])
        missing_input_weight += f32(before_target) != arguments[0]
        missing_writer_weight += result != writer
        pelvis_before = vector(fields(values(frame, 59)[3])["Translation"])
        desired = vector(values(frame, 66)[3])
        assert desired == [pelvis_before[0], pelvis_before[1], pelvis_before[2] + writer]
        feet = []
        for pc in (83, 113):
            if pc in visited:
                assert values(frame, pc)[4]["numeric_value"] == writer
                feet.append(pc)
                consumers += 1
        if case["name"] == native[0]["name"]:
            assert arguments == archive[query]["source_arguments_after"]
        cases.append({"query": query, "delta_seconds": frame["delta_seconds_after_update"],
                      "rig_input": rig_input(externals["RigInput"]), "trace_offsets": [left, right],
                      "spring_target": arguments[0], "spring_result": result, "spring_state": state,
                      "writer": writer, "pelvis_before": point_to_meters(pelvis_before),
                      "pelvis_desired": point_to_meters(desired), "feet": feet})
    histories.append({"name": case["name"], "frames": cases})

tie = histories[1]["frames"][1]
assert tie["spring_result"] == -12.000143051147461 and tie["writer"] == -9.000106811523438
assert f32((tie["spring_result"] * 0.01) * 0.75 * 100.0) == -9.000107765197754
assert all(name == histories[1]["name"] for name, _ in legacy_writer_mismatches)
assert len(legacy_writer_mismatches) == missing_input_weight == missing_writer_weight == 3
assert histories[2]["frames"][6]["writer"] == -1.6625938415527344
assert consumers == 66
path = "assets/ALS/Character/CR_Als.program.json"
prior_target = json.loads(subprocess.check_output(["git", "show", f"931c0417:{path}"]))
lengths = []
assert project_doubles(load(directory.parents[3] / path), lengths) == prior_target
assert lengths
print(json.dumps({"native_consumer_calls": consumers, "meter_writer_ablation": legacy_writer_mismatches,
                  "input_weight_ablation": missing_input_weight, "writer_weight_ablation": missing_writer_weight,
                  "source_double_length_values": len(lengths), "histories": histories}, separators=(",", ":")))
