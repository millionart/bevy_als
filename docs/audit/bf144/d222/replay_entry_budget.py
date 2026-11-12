import gzip
import json
from pathlib import Path


AUDIT = Path(__file__).resolve().parent


def main():
    path = AUDIT / "budget.native.json"
    raw = path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix(".json.gz").read_bytes())
    native = json.loads(raw)
    machine = native["state_machine_players"]["machines"][0]
    entry = machine["states"][machine["initial_state"]]
    assert machine["name"] == "Grounded States" and entry["name"] == "Entry Conduit" and entry["conduit"]
    cases = []
    extra_transitions = 0
    for sample in native["state_machine_entry_probe"]["samples"]:
        direction, mode = sample["name"].split("_")
        origin = direction.capitalize()
        target = "Crouching" if origin == "Standing" else "Standing"
        budget = sample["max_transitions"]
        expected_budget = 2 if mode == "2" else 1
        assert budget == expected_budget
        frames = sample["frames"]
        allowed = mode != "same"
        first = target if allowed and budget > 1 else origin
        states = [first, target if allowed else origin, target if allowed else origin]
        assert [frame["name"] for frame in frames] == states
        assert frames[0]["active_transitions"] == 0
        if allowed and budget == 1:
            assert frames[1]["active_transitions"] == 1 and frames[1]["active_entries"][0]["alpha"] > 0.0
            extra_transitions += 1
        else:
            assert all(frame["active_transitions"] == 0 for frame in frames)
        cases.append({"name": sample["name"], "origin": origin, "budget": budget, "allowed": allowed,
                      "states": states, "entry": True})
    assert extra_transitions == 4
    print(json.dumps({"cases": cases, "native_frames": sum(len(case["states"]) for case in cases),
                      "free_entry_ablation": extra_transitions}, indent=2))


if __name__ == "__main__":
    main()
