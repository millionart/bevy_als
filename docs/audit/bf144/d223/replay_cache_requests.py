import gzip
import json
import re
from pathlib import Path


AUDIT = Path(__file__).resolve().parent
ROOT = AUDIT.parents[3]


def main():
    path = AUDIT / "cache.native.json"
    raw = path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix(".json.gz").read_bytes())
    source = json.loads(raw)
    layer = next(layer for layer in source["cached_pose_schedule"]["layers"] if layer["name"] == "AnimGraph")
    ranks = {node["cache_name"]: rank for rank, node in enumerate(layer["nodes"])}
    graph = (ROOT / "assets/ALS/Character/AnimationInstances/AB_Als_Layering/AnimGraph.animgraph.ron").read_text()
    saves = {}
    for match in re.finditer(r'id: "([^"]+)", name: "Save cached pose \'([^\']+)\'", inner: \{ "als_ext::SourceCachedPoseSaveNode": \([^\n]+submission_order: Some\((\d+)\)', graph):
        identity, name, rank = match.groups()
        assert ranks[name] == int(rank)
        saves[name] = identity
    assert set(saves) == set(ranks)
    nodes = {match[1]: match[2] for match in re.finditer(r'id: "([^"]+)", name: "([^"]+)"', graph)}
    inputs = {}
    for match in re.finditer(r'NodeData\("([^"]+)", "[^"]+"\): NodeData\("([^"]+)", "[^"]+"\)', graph):
        inputs.setdefault(match[1], []).append(match[2])

    def reaches_locomotion(cache):
        frontier = [saves[cache]]
        seen = set()
        while frontier:
            node = frontier.pop()
            if node in seen:
                continue
            seen.add(node)
            name = nodes[node]
            cached = re.fullmatch(r"Use cached pose '([^']+)'", name)
            if cached and cached[1] == "Locomotion":
                return True
            if cached:
                frontier.append(saves[cached[1]])
            frontier.extend(inputs.get(node, []))
        return False

    assert all(reaches_locomotion(cache) for cache in ("Legs", "Arm Left", "Arm Right"))
    assert all(ranks[cache] < ranks["Locomotion"] for cache in ("Legs", "Arm Left", "Arm Right"))
    cases = source["cached_pose_lifecycle_probe"]["weight_cases"]
    first_request_failures = 0
    for case in cases:
        requests = case["requests"]
        expected = [max(requests)] if requests else []
        assert case["child_update_weights"] == expected
        first_request_failures += requests[:1] != expected
    assert len(cases) == 5 and first_request_failures == 2
    print(json.dumps({"cases": cases, "ranks": ranks, "first_request_ablation": first_request_failures}, indent=2))


if __name__ == "__main__":
    main()
