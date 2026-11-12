import gzip
import json
import sys
from pathlib import Path


ROOT = Path(__file__).parent


def read(name):
    path = ROOT / name
    if path.is_file():
        return json.loads(path.read_bytes())
    return json.loads(gzip.decompress(path.with_suffix(".json.gz").read_bytes()))


def write(name, value):
    (ROOT / name).write_text(json.dumps(value, indent=2) + "\n")


def source_layout():
    native = read("weights.native.json")
    machine = native["state_machine_players"]["machines"][0]
    assert machine["name"] == "Locomotion States"
    land, movement = machine["states"][3:5]
    assert [land["name"], movement["name"]] == ["Land", "Land Movement"]
    edge = next(edge for edge in movement["exits"] if edge["automatic"])
    assert edge["index"] == 11 and edge["trigger"] < 0.0
    players = []
    for state, basename, rate in [(land, "A_Als_Land_Light", 1.0),
                                  (movement, "A_Als_Land_Light_Additive", 1.75)]:
        player = next(player for player in state["players"]
                      if player["synchronization"]["asset_path"].endswith("." + basename))
        sync = player["synchronization"]
        assert sync["group"] == "Land" and sync["role"] == "CanBeLeader"
        assert not sync["marker_names"]
        players.append((player["node_index"], {"asset": sync["asset_path"], "start": 0.0, "rate": rate}))
    return native["state_machine_entry_probe"]["samples"], players


def clock_request():
    states, players = source_layout()
    samples = []
    for state, request in zip(states, read("weights.request.json")["samples"], strict=True):
        assert state["name"] == request["name"]
        assert [frame["name"] for frame in state["frames"][:4]] == ["Grounded", "Fall", "Land", "Land Movement"]
        for warm in (0.75, 0.875, 1.0):
            steps = [{"delta_seconds": warm, "weights": [1.0, None]}]
            for frame, step in zip(state["frames"][3:], request["steps"][3:], strict=True):
                weights = [next(iter(values), None) for values in frame["updates"][3:5]]
                steps.append({"delta_seconds": step["delta_seconds"], "weights": weights})
            samples.append({"name": f'{request["name"]}_{warm}', "steps": steps})
    write("clock.request.json", {"players": [player for _, player in players], "samples": samples})


def transition_request():
    _, players = source_layout()
    weights = {sample["name"]: sample for sample in read("weights.request.json")["samples"]}
    native = read("clock.native.json")["sync_group_clock_probe"]
    assert all(not player.get("markers") for player in native["players"])
    cases = []
    for request, trace in zip(read("clock.request.json")["samples"], native["samples"], strict=True):
        name = request["name"]
        assert trace["name"] == name
        state = weights[name.rsplit("_", 1)[0]]
        for mode in ("weighted", "local_only"):
            steps = [{**step, "asset_players": []} for step in state["steps"]]
            for index, (source, frame) in enumerate(zip(request["steps"], trace["frames"], strict=True), 3):
                for (node_index, _), weight, clock in zip(players, source["weights"], frame["players"], strict=True):
                    if weight is not None and index < len(steps):
                        assert clock["ticked"]
                        steps[index]["asset_players"].append({"node_index": node_index,
                                                              "weight": weight if mode == "weighted" else 1.0,
                                                              "position": clock["position"]})
            cases.append({"name": f"{name}/{mode}", "steps": steps})
    write("transition.request.json", {"machine_index": 0, "samples": cases})


def check():
    native = read("transition.native.json")["state_machine_entry_probe"]["samples"]
    requested = read("transition.request.json")["samples"]
    states = {}
    first_exit = {}
    for sample, request in zip(native, requested, strict=True):
        name = sample["name"]
        assert request["name"] == name
        frames = sample["frames"]
        assert frames[3]["name"] == "Land Movement"
        assert frames[3]["updates"][4] == [0.0 if name.startswith("zero_") else frames[3]["weights"][4]]
        assert name.startswith("zero_") == (frames[3]["weights"][4] == 0.0)
        states[name] = [frame["name"] for frame in frames]
        first_exit[name] = next((index for index in range(4, len(frames)) if frames[index]["name"] == "Grounded"), None)
    changes = {}
    for name, weighted in first_exit.items():
        if not name.endswith("/weighted"):
            continue
        prefix = name.rsplit("/", 1)[0]
        local = first_exit[f"{prefix}/local_only"]
        if weighted != local:
            changes[prefix] = [local, weighted]
        else:
            assert states[name] == states[f"{prefix}/local_only"]
    assert changes == {"zero_0.875": [4, 5], "zero_1.0": [4, 5]}, changes
    print(json.dumps({"cases": len(states) // 2, "native_replay_frames": sum(map(len, states.values())),
                      "first_exit": first_exit, "local_weight_ablation": changes}, indent=2))


if __name__ == "__main__":
    {"clock-request": clock_request, "transition-request": transition_request, "check": check}[sys.argv[1]]()
