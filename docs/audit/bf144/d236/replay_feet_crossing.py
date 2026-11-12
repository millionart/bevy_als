import gzip
import json
import struct
from pathlib import Path


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
RULE = (
    ROOT / "assets/ALS/Character/AnimationInstances/Stances/AB_Als_Standing/AnimGraph"
    / "SM2_Movement_States/T16_Transition.condition.animgraph.ron"
)


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def check_source_rule():
    path = HERE / "standing.native.json"
    native = json.loads(path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix(".json.gz").read_bytes()))
    graph, = [graph for graph in native["graphs"] if ":AnimGraph." in graph["object_path"]
              and ".Movement States." in graph["object_path"]
              and graph["object_path"].endswith("AnimStateTransitionNode_16.Transition")]
    paths = {prop["exported_text"] for node in graph["nodes"] for prop in node["properties"]}
    assert any(path.endswith("FeetState.FeetCrossingAmount") for path in paths)
    assert any(path.endswith("GroundedState.HipsDirectionLockAmount") for path in paths)
    comparisons = [node for node in graph["nodes"] if any(
        "LessEqual_DoubleDouble" in prop["exported_text"] for prop in node["properties"])]
    assert len(comparisons) == 2
    values = []
    for node in comparisons:
        pin = next(pin for pin in node["pins"] if pin["name"] == "B")
        assert not pin["linked_to"]
        values.append(float(pin["default_value"] or "0"))
    assert sorted(values) == [-0.5, 0.0]
    target = RULE.read_text()
    assert 'global_key: "feet_state.feet_crossing_amount"' in target
    assert 'global_key: "grounded_state.hips_direction_lock_amount"' in target
    assert 'f32_default: -0.5' in target and 'OutputData("bCanEnterTransition")' in target


def replay():
    check_source_rule()
    feet = (ROOT / "src/als/character/foot_ik_apply.rs").read_text()
    provider = (ROOT / "src/als/character/animation_instances/ab_als_provider.rs").read_text()
    assert feet.index("if pending {\n            lock_state.valid = false") < feet.index(
        "lock_state.feet_crossing_amount = lock_input.feet_crossing_amount.clamp(0.0, 1.0)")
    assert ".before(crate::ab_als_provider::AbAlsProviderSet)" in feet
    assert 'feet.map_or(0.0, |state| state.feet_crossing_amount)' in provider
    history = [
        (True, 0.8, -1.0), (False, 0.0, -1.0), (True, 0.0, -1.0), (False, 0.8, -1.0),
        (True, 0.8, -0.49), (False, 0.0, -0.49), (True, 0.0, -0.5), (True, 0.8, -0.5),
        (True, -0.3, -1.0), (False, -0.3, -1.0), (True, 1.2, -1.0), (False, 1.2, -1.0),
    ]
    previous = 0.0
    cases = []
    for valid, curve, hips in history:
        current = min(max(f32(curve), 0.0), 1.0)
        if valid:
            previous = current
        cases.append({"valid": valid, "feedback_curve": current, "hips_lock": f32(hips),
                      "retained": previous, "transition": hips <= -0.5 and previous <= 0.0,
                      "with_unconditional_feedback": hips <= -0.5 and current <= 0.0})
    assert [i for i, row in enumerate(cases) if row["transition"] != row["with_unconditional_feedback"]] == [1, 3]
    return {"result": "PASS_RETAINED_FEET_CROSSING", "cases": cases}


if __name__ == "__main__":
    print(json.dumps(replay(), indent=2))
