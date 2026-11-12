import gzip
import importlib.util
import json
from pathlib import Path


AUDIT = Path(__file__).resolve().parent


def read(name):
    path = AUDIT / name
    raw = path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix(".json.gz").read_bytes())
    return json.loads(raw)


def main():
    spec = importlib.util.spec_from_file_location("source_group_sort", AUDIT.parent / "d202/replay_sync_order.py")
    sort = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(sort)
    parent = read("order.native.json")
    layer = next(layer for layer in parent["cached_pose_schedule"]["layers"] if layer["name"] == "AnimGraph")
    ranks = {node["cache_name"]: rank for rank, node in enumerate(layer["nodes"])}
    assert ranks["Move Forward"] == 4 and ranks["Move Forward Base"] == 5
    parent_cases = parent["blend_list_probe"]["samples"]
    source = read("group.native.json")["sync_group_clock_probe"]
    request = read("group.request.json")
    labels = [2, 3, 0, 0, 2, 3]
    names = {0: "base", 2: "sprint", 3: "acceleration"}
    frames_for_fixture = []
    summary = {}
    for index, parent_case in enumerate(parent_cases):
        name = parent_case["name"]
        requested, old_requested = request["samples"][2 * index:2 * index + 2]
        fresh, old = source["samples"][2 * index:2 * index + 2]
        assert requested["name"] == fresh["name"] == f"{name}/source"
        assert old_requested["name"] == old["name"] == f"{name}/pose_order"
        count = {"frames": len(fresh["frames"]), "leaders": 0, "clock_or_marker": 0}
        for frame_index, (frame, step, old_step, a, b) in enumerate(zip(
            parent_case["frames"], requested["steps"], old_requested["steps"], fresh["frames"], old["frames"], strict=True
        )):
            weights = {0: next(iter(frame["cached_updates"]), None),
                       2: next(iter(frame["two_way"]["updates"][0]), None),
                       3: next(iter(frame["two_way"]["updates"][1]), None)}
            submission = [label for label in (2, 3, 0) if weights[label] is not None]
            assert frame["update_order"] == submission
            for step_row, observed in [(step, a), (old_step, b)]:
                registered = [candidate for candidate, value in enumerate(step_row["weights"]) if value is not None]
                assert {labels[candidate]: step_row["weights"][candidate] for candidate in registered} == {
                    label: weight for label, weight in weights.items() if weight is not None
                }
                ordered = sort.source_order(registered, step_row["weights"], 1)
                assert observed["order"] == ordered and observed["leader"] == ordered[0]
            source_leader, old_leader = labels[a["leader"]], labels[b["leader"]]
            count["leaders"] += source_leader != old_leader
            count["clock_or_marker"] += a["ratio"] != b["ratio"] or a.get("marker_end") != b.get("marker_end")
            if (frame_index == 6 and name in ("ordinary", "acceleration")) or (frame_index == 8 and name == "both"):
                assert source_leader != old_leader and weights[source_leader] == weights[old_leader]
                frames_for_fixture.append({"weights": [weights[label] or 0.0 for label in (0, 2, 3)],
                                           "submission": [names[label] for label in submission],
                                           "leader": names[source_leader], "old_leader": names[old_leader]})
        summary[name] = count
    assert sum(row["leaders"] for row in summary.values()) == 14
    assert sum(row["clock_or_marker"] for row in summary.values()) == 163
    assert summary["blocked"]["leaders"] == summary["blocked"]["clock_or_marker"] == 0
    print(json.dumps({"ranks": [ranks["Move Forward"], ranks["Move Forward Base"]],
                      "frames": frames_for_fixture, "summary": summary}, indent=2))


if __name__ == "__main__":
    main()
