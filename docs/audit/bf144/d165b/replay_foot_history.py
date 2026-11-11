import gzip
import json
import math
import struct
import sys
from pathlib import Path


def load(path):
    data = path.read_bytes() if path.is_file() else gzip.decompress(path.with_suffix(".json.gz").read_bytes())
    return json.loads(data)


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def point_to_meters(vector):
    return [vector[index] * 0.01 for index in (0, 2, 1)]


directory = Path(__file__).resolve().parent
sys.path.insert(0, str(directory.parents[3] / "tools/animgraph_authoring"))
from ue_control_rig_contract import compile_contract

rig = load(directory / "rig.native.json")
program = compile_contract(rig)["program"]
history = load(directory / "foot-history.native.json")["rig_node_history_probe"]["cases"][0]["frames"]
recording = load(directory.parent / "468/inputs.json")["rows"]
archive = {(row["query"], row["pc"]): row for row in recording}


def values(frame, pc):
    result = []
    for ref in program["instructions"][pc]["operands"]:
        memory, register = ref["memory"], ref["register"]
        if memory == "External":
            key = program["memories"][memory][register]["name"]
            value = frame["externals_after"][key]
        else:
            bank = frame["work_after"] if memory == "Work" else rig["rig_vm"]["literal"]
            value = bank["properties"][register]
        if "slice" in ref:
            value = value["elements"][ref["slice"]]
        for field in ref["path"]:
            value = next(item for item in value["fields"] if item["name"] == field)
        result.append(value)
    return result


def fields(value):
    return {field["name"]: field for field in value["fields"]}


def vector(value):
    return [fields(value)[axis]["numeric_value"] for axis in "XYZ"]


def spring_state(value):
    state = fields(value)
    return [state["Velocity"]["numeric_value"], state["PreviousTarget"]["numeric_value"],
            state["bStateValid"]["exported_text"] == "True"]


native_calls = 0
for query, frame in enumerate(history):
    visited = next(event["instructions"] for event in frame["executions"] if event["entry"] == "Forwards Solve")
    if query == 0:
        assert 83 not in visited and 113 not in visited
        continue
    for pc in (83, 113):
        assert visited.count(pc) == 1
        arguments = values(frame, pc)
        row = archive[query, pc]
        expected = row["source_arguments_after"]
        assert vector(arguments[2]) == expected[2]
        assert [argument["numeric_value"] for argument in arguments[3:11]] == expected[3:11]
        assert (arguments[11]["exported_text"] == "True") == expected[11]
        assert spring_state(arguments[14]) == expected[14]
        assert arguments[15]["numeric_value"] == expected[15]
        assert vector(arguments[16]) == expected[16]
        bones = {bone["name"]: bone for bone in frame["hierarchy_after"]["bones"]}
        for bone, key in (("pelvis", "source_pelvis_translation_cm"), (expected[1][1], "source_thigh_translation_cm")):
            position = bones[bone]["current_global_transform"]["translation"]
            assert [position[axis] for axis in "xyz"] == row[key]
        native_calls += 1
    for trace_pc, location_pc in ((31, 83), (40, 113)):
        assert visited.count(trace_pc) == 1
        assert values(frame, trace_pc)[8]["numeric_value"] == archive[query, location_pc]["source_arguments_after"][3]

replay = []
target_failures = 0
old_state_failures = 0
clamped = 0
for row in recording:
    source = row["source_arguments_after"]
    pelvis = point_to_meters(row["source_pelvis_translation_cm"])
    thigh = point_to_meters(row["source_thigh_translation_cm"])
    target = point_to_meters(source[2])
    maximum = f32(f32(f32(pelvis[1] * 100.0) - source[6]) - f32(target[1] * 100.0))
    target_offset = max(min(source[3], maximum), source[4])
    target_failures += target_offset != row["source_target_cm"]
    legacy = row["expected_common_meter_scalar"]
    old_state_failures += f32(legacy["offset"] * 100.0) != source[15] or any(
        f32(value * 100.0) != expected for value, expected in zip(legacy["state"][:2], source[14][:2], strict=True)
    )
    target_cm = [value * 100.0 for value in target]
    thigh_cm = [value * 100.0 for value in thigh]
    target_cm[1] += source[15]
    leg = [goal - origin for goal, origin in zip(target_cm, thigh_cm, strict=True)]
    squared = (leg[0] * leg[0] + leg[2] * leg[2]) + leg[1] * leg[1]
    max_length = f32(source[5] * source[7])
    clamped += squared > max_length * max_length
    if squared > max_length * max_length:
        scale = max_length * (1.0 / math.sqrt(squared))
        leg = [value * scale for value in leg]
    output = [(origin + delta) * 0.01 for origin, delta in zip(thigh_cm, leg, strict=True)]
    expected = point_to_meters(source[16])
    assert math.dist(output, expected) < 1e-12, (row["query"], row["pc"], output, expected)
    assert list(map(f32, output)) == list(map(f32, expected))
    replay.append({"query": row["query"], "foot": int(row["pc"] == 113), "delta_seconds": row["delta_seconds"],
                   "pelvis": pelvis, "thigh": thigh, "target": target, "inputs": source[3:11],
                   "state": source[14], "offset": source[15], "location": expected})

assert native_calls == 22 and len(replay) == 636
assert target_failures == 0 and clamped == 121 and old_state_failures > 0
frame = history[1]
query = frame["reference_query_after_forward"]
assert query["blocking_hit"]
native_trace = values(frame, 31)
trace = []
for point, normal in (([query["vm_impact_point_cm"][axis] for axis in "xyz"],
                       [query["vm_impact_normal"][axis] for axis in "xyz"]),
                      ([0.0, 0.0, -31.204151999], [0.0, 0.0, 1.0])):
    slope = f32(normal[2])
    height = native_trace[5]["numeric_value"]
    correction = f32(f32(height / slope) - height)
    offset = f32(point[2] + correction)
    trace.append({"point": point_to_meters(point), "normal": [normal[index] for index in (0, 2, 1)], "offset": offset})
assert trace[0]["offset"] == native_trace[8]["numeric_value"]
assert trace[1]["offset"] == -31.204151153564453
assert trace[1]["offset"] * 0.01 != f32(trace[1]["point"][1])
result = {"native_resampled_calls": native_calls, "clamped_legs": clamped,
          "meter_state_ablation_failures": old_state_failures, "trace": trace, "cases": replay}
print(json.dumps(result, separators=(",", ":")))
