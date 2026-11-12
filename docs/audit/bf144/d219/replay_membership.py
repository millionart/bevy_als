import gzip
import json
import struct
from pathlib import Path


AUDIT = Path(__file__).resolve().parent
THRESHOLD = struct.unpack("<f", struct.pack("<f", 1e-5))[0]


def read(name):
    path = AUDIT / name
    raw = path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix(".json.gz").read_bytes())
    return json.loads(raw)


def main():
    parent = read("membership.native.json")["blend_list_probe"]["samples"]
    source = read("sync.native.json")["sync_group_clock_probe"]["samples"]
    request = read("sync.request.json")["samples"]
    summary = {}
    for index, case in enumerate(parent):
        name = case["name"]
        kept, dropped = source[index * 2:index * 2 + 2]
        kept_request, dropped_request = request[index * 2:index * 2 + 2]
        assert kept["name"] == kept_request["name"] == f"{name}/source"
        assert dropped["name"] == dropped_request["name"] == f"{name}/filtered"
        counts = {"frames": len(case["frames"]), "positive_drops": 0, "zero_drops": 0, "empty_groups": 0}
        for parent_frame, source_frame, old_frame, a, b in zip(
            case["frames"], kept["frames"], dropped["frames"], kept_request["steps"], dropped_request["steps"], strict=True
        ):
            updates = [*parent_frame["two_way"]["updates"], parent_frame["cached_updates"]]
            weights = [next(iter(values), None) for values in updates]
            expected = {player for player, value in enumerate(weights) if value is not None}
            filtered = {player for player in expected if weights[player] > THRESHOLD}
            assert a["weights"] == weights
            assert b["weights"] == [value if index in filtered else None for index, value in enumerate(weights)]
            assert set(source_frame["order"]) == expected and set(old_frame["order"]) == filtered
            assert source_frame["leader"] in expected
            assert old_frame["leader"] is None if not filtered else old_frame["leader"] in filtered
            counts["zero_drops"] += sum(weights[player] == 0.0 for player in expected - filtered)
            counts["positive_drops"] += sum(weights[player] > 0.0 for player in expected - filtered)
            counts["empty_groups"] += bool(expected and not filtered)
        summary[name] = counts
    assert summary["nested"]["positive_drops"] == 2 and summary["almost_blocked"]["positive_drops"] == 6
    assert summary["zero_parent"]["zero_drops"] == 46 and summary["zero_parent"]["empty_groups"] == 16
    assert all(summary[name]["positive_drops"] == summary[name]["zero_drops"] == 0 for name in ("normal", "blocked"))
    print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    main()
