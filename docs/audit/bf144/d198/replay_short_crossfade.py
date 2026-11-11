import gzip
import json
import struct
from pathlib import Path


directory = Path(__file__).resolve().parent
root = directory.parents[3]
request = json.loads((directory / "clock.request.json").read_bytes())
with gzip.open(directory / "clock.native.json.gz") as stream:
    native = json.load(stream)["blend_profile_probe"]
with gzip.open(directory / "locomotion.native.json.gz") as stream:
    locomotion = json.load(stream)
guid = "910985a7-40ad-7d3b-7a20-c3bb6d5a8254"
node = next(node for graph in locomotion["graphs"] for node in graph["nodes"] if node["node_guid"] == guid)
properties = {prop["name"]: prop for prop in node["properties"]}
assert properties["bAutomaticRuleBasedOnSequencePlayerInState"]["exported_text"] == "True"
assert properties["AutomaticRuleTriggerTime"]["numeric_value"] == -1
assert properties["BlendMode"]["exported_text"] == "HermiteCubic"
nominal = properties["CrossfadeDuration"]["numeric_value"]
base = root / "assets/ALS/Character/AnimationInstances/AB_Als_Locomotion/AnimGraph"
machine = (base / "SM9_Locomotion_States.fsm.ron").read_text()
transition = next(line for line in machine.splitlines() if guid in line)
assert "timed: Some(0.3)" in transition and "14e291d3-4d0c-1c14-bde0-b1a262bbe55a" in transition
graph = base / "SM9_Locomotion_States/T2/Transition.transition.animgraph.ron"
assert "alpha_blend_mode: HermiteCubic" in graph.read_text()


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


cases = []
failures = 0
for sample, raw in zip(request["samples"], native["samples"], strict=True):
    assert sample["name"] == raw["name"]
    duration = f32(sample["duration"])
    if duration < 0.001:
        assert nominal == f32(0.3)
        adjustment = f32(nominal - duration)
        assert f32(nominal - adjustment) == duration
    frames = []
    for frame in raw["frames"]:
        factor = min(f32(frame["elapsed"] / duration), 1.0)
        assert factor == frame["factor"]
        alpha = f32(f32(factor * factor) * f32(3.0 - f32(2.0 * factor)))
        assert alpha == frame["alpha"]
        legacy = 1.0 if duration <= 2 ** -23 else alpha
        failures += legacy != alpha
        assert frame["complete"] == (frame["elapsed"] >= duration)
        frames.append({"elapsed": frame["elapsed"], "weights": [f32(1.0 - alpha), alpha],
                       "complete": frame["complete"], "legacy_target_weight": legacy})
    cases.append({"name": sample["name"], "duration": duration, "frames": frames})
assert failures == 6
print(json.dumps({"source_transition": guid, "legacy_mismatches": failures, "cases": cases}, indent=2))
