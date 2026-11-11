import json
from pathlib import Path


directory = Path(__file__).resolve().parent
root = directory.parents[3]
head = json.loads((directory / "head.native.json").read_text())
nodes = [node for graph in head["graphs"] for node in graph["nodes"]]
init, = [node for node in nodes if node["title"] == "InitializeHead\nOn Become Relevant"]
refresh, = [node for node in nodes if node["title"] == "RefreshHead\nOn Update"]
assert init["enabled"] and refresh["enabled"]
installed = (root / "assets/ALS/Character/AnimationInstances/AB_Als_Head/AnimGraph.animgraph.ron").read_text()
assert f'id: "{init["node_guid"]}"' in installed
assert f'id: "{refresh["node_guid"]}"' in installed
assert 'property: "head_state.b_initialization_required", value: Bool(true)' in installed
assert 'on_become_relevant: true' in installed
assert f'NodeData("{init["node_guid"]}", "Source"): NodeData("{refresh["node_guid"]}", "Pose")' in installed
assert 'key: "head_state.pitch_angle"' in installed

clock = json.loads((directory / "montage.native.json").read_text())["montage_clock_probe"]
samples = {sample["name"]: sample["frames"] for sample in clock["samples"]}
running = samples["running"]
terminal = samples["terminal"]
held = samples["held"]
assert running[-1]["valid"] and running[-1]["position"] > running[0]["position"]
assert terminal[1]["valid"] and terminal[1]["stopped"] and terminal[1]["weight"] > 0.0
assert not terminal[2]["valid"] and terminal[2]["weight"] == 0.0
held_frame = next(i for i, frame in enumerate(held[:-1]) if frame["valid"] and not frame["playing"])
assert held[held_frame]["weight"] > 0.0

cases = [
    ("idle", False, False, False, False),
    ("running", running[0]["valid"], running[0]["playing"], running[1]["valid"], True),
    ("held", held[held_frame]["valid"], held[held_frame]["playing"], held[held_frame + 1]["valid"], True),
    ("terminal", terminal[1]["valid"], terminal[1]["playing"], terminal[2]["valid"], True),
    ("expired", terminal[2]["valid"], terminal[2]["playing"], terminal[3]["valid"], False),
]
results = []
without_reset = []
for name, before, playing, after, expected in cases:
    last_call_frame = 8
    resumed_graph_frame = 9
    last_relevant_frame = None if before else last_call_frame
    became_relevant = last_relevant_frame is None or resumed_graph_frame - last_relevant_frame > 1
    assert became_relevant == expected
    if (resumed_graph_frame - last_call_frame > 1) != expected:
        without_reset.append(name)
    results.append({
        "case": name,
        "valid_before_advance": before,
        "playing_before_advance": playing,
        "valid_after_advance": after,
        "frozen_graph_frame": last_call_frame,
        "resumed_graph_frame": resumed_graph_frame,
        "initialize_head": became_relevant,
    })
assert without_reset == ["running", "held", "terminal"]
playing_only = [name for name, _, playing, _, expected in cases if playing != expected]
after_advance_only = [name for name, _, _, after, expected in cases if after != expected]
assert playing_only == ["held"]
assert after_advance_only == ["terminal"]
print(json.dumps({
    "initialize_head": init["node_guid"],
    "refresh_head": refresh["node_guid"],
    "cases": results,
    "ablation_failures": {
        "without_reset": without_reset,
        "playing_only": playing_only,
        "after_advance_only": after_advance_only,
    },
}, indent=2))
