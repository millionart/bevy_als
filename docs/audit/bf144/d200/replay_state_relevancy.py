import gzip
import json
from pathlib import Path


directory = Path(__file__).resolve().parent
root = directory.parents[3]
instances = root / "assets/ALS/Character/AnimationInstances"
descriptors = [
    ("locomotion", "Locomotion States", "Land Movement", 2,
     instances / "AB_Als_Locomotion/AnimGraph/SM9_Locomotion_States"),
    *[("standing", "Movement Details States", name, 4,
       instances / "Stances/AB_Als_Standing/AnimGraph/SM0_Movement_Details_States")
      for name in ["Run Start", "Run Start From Walk", "First Pivot", "Second Pivot"]],
]


def state_machine_players(name):
    with gzip.open(directory / f"{name}.native.json.gz") as stream:
        return json.load(stream)["state_machine_players"]


reports = {name: state_machine_players(name) for name in ["locomotion", "standing"]}
cases = []
for asset, machine_name, state_name, count, graph_directory in descriptors:
    machine = next(item for item in reports[asset]["machines"] if item["name"] == machine_name)
    state = next(item for item in machine["states"] if item["name"] == state_name)
    players = state["players"]
    assert len(players) == count and not state["layers"]
    graph_path, = graph_directory.glob(f"*_{state_name.replace(' ', '_')}.animgraph.ron")
    graph = graph_path.read_text()
    assert "SourceScopedCachedPoseUseNode" in graph
    for player in players:
        assert f'id: "{player["guid"]}"' in graph
        assert player["asset_path"].rsplit(".", 1)[-1] + ".anim.ron" in graph
    edges = [edge for edge in state["exits"] if edge["automatic"]]
    assert edges
    checks = []
    for edge in edges:
        threshold = edge["duration"] if edge["trigger"] < 0.0 else edge["trigger"]
        duration = players[0]["asset_duration"]
        assert duration > threshold
        local = (players[0]["guid"], 0.75, duration - 0.05)
        foreign = ("cached_player", 1.0, min(0.2, threshold))
        assert local[2] > threshold >= foreign[2]
        for scope, cache in [("outside", [foreign]), ("no_outer_cache", []), ("local", [foreign])]:
            old = max([*cache, local], key=lambda candidate: candidate[1])
            restored = max([*([] if scope == "outside" else cache), local],
                           key=lambda candidate: candidate[1])
            assert (restored[2] <= threshold) == (scope == "local")
            checks.append({"scope": scope, "threshold": threshold,
                           "previous": old[0], "selected": restored[0],
                           "previous_automatic": old[2] <= threshold, "automatic": restored[2] <= threshold})
    cases.append({"state": state_name, "native_players": [player["guid"] for player in players],
                  "target_graph": graph_path.relative_to(root).as_posix(), "cases": checks})
assert sum(len(case["native_players"]) for case in cases) == 18
legacy = sum(row["previous_automatic"] != row["automatic"] for case in cases for row in case["cases"])
assert legacy == 5
print(json.dumps({"legacy_automatic_mismatches": legacy, "cases": cases}, indent=2))
