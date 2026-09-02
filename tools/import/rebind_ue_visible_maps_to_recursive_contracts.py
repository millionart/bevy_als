import argparse
import json
import tempfile
from pathlib import Path


MAP_FORMATS = {
    "als_ue_visible_animgraph_map_v2",
    "als_ue_visible_fsm_map_v1",
}


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def relative_path(root, path):
    return path.resolve().relative_to(root.resolve()).as_posix()


def source_contracts(root, inventory_path):
    index = read_json(inventory_path)
    entries = index.get("entries")
    if index.get("format") != "als_ue_visible_graph_contract_batch_v1" or not isinstance(entries, list):
        raise ValueError("recursive contract index is invalid")
    values = {}
    for entry in entries:
        if not isinstance(entry, dict):
            raise ValueError("recursive contract entry is invalid")
        source_graph = entry.get("source_graph")
        contract = entry.get("contract")
        if not isinstance(source_graph, str) or not source_graph:
            raise ValueError("recursive contract source graph is invalid")
        if not isinstance(contract, str) or not contract:
            raise ValueError("recursive contract path is invalid")
        path = inventory_path.parent / contract
        if not path.is_file():
            raise ValueError(f"recursive contract is missing: {path.as_posix()}")
        if source_graph in values:
            raise ValueError(f"duplicate recursive source graph: {source_graph}")
        values[source_graph] = relative_path(root, path)
    return values


def prior_contract_graphs(previous_index_path, current_index):
    if previous_index_path is None:
        return {}
    previous = read_json(previous_index_path)
    entries = previous.get("entries")
    if previous.get("format") != "als_ue_visible_graph_contract_batch_v1" or not isinstance(entries, list):
        raise ValueError("previous recursive contract index is invalid")
    for key in ("source_asset", "root_source_graph", "inventory_sha256"):
        if previous.get(key) != current_index.get(key):
            raise ValueError(f"previous recursive contract index differs: {key}")
    values = {}
    for entry in entries:
        if not isinstance(entry, dict):
            raise ValueError("previous recursive contract entry is invalid")
        source_graph = entry.get("source_graph")
        contract = entry.get("contract")
        if not isinstance(source_graph, str) or not source_graph:
            raise ValueError("previous recursive source graph is invalid")
        if not isinstance(contract, str) or not contract:
            raise ValueError("previous recursive contract path is invalid")
        name = Path(contract).name
        if name in values:
            raise ValueError(f"duplicate previous recursive contract: {name}")
        values[name] = source_graph
    return values


def map_source_graph(root, mapping, previous_contracts=None):
    if previous_contracts is None:
        previous_contracts = {}
    source_contract = mapping.get("source_contract")
    if not isinstance(source_contract, str) or not source_contract:
        raise ValueError("map source_contract is invalid")
    path = root / source_contract
    if not path.is_file():
        source_graph = previous_contracts.get(Path(source_contract).name)
        if source_graph is None:
            raise ValueError(f"map source contract is missing: {source_contract}")
        return source_graph
    source = read_json(path)
    source_graph = source.get("source_graph")
    if isinstance(source_graph, dict):
        source_graph = source.get("source_graph_suffix")
    if not isinstance(source_graph, str) or not source_graph:
        raise ValueError(f"map source graph is invalid: {source_contract}")
    return source_graph


def plans(root, inventory_path, map_glob, previous_index_path=None):
    contracts = source_contracts(root, inventory_path)
    previous_contracts = prior_contract_graphs(
        previous_index_path,
        read_json(inventory_path),
    )
    paths = sorted(root.glob(map_glob))
    if not paths:
        raise ValueError("map glob matched no files")
    values = []
    for path in paths:
        mapping = read_json(path)
        if mapping.get("format") not in MAP_FORMATS:
            continue
        source_graph = map_source_graph(root, mapping, previous_contracts)
        expected = contracts.get(source_graph)
        if expected is None:
            raise ValueError(f"map source graph is absent from recursive index: {source_graph}")
        values.append(
            {
                "path": path,
                "mapping": mapping,
                "source_graph": source_graph,
                "expected": expected,
                "matches": mapping["source_contract"] == expected,
            }
        )
    if not values:
        raise ValueError("map glob matched no visible graph maps")
    return values


def check(values):
    return [
        f"source contract is stale: {value['path'].as_posix()} -> {value['expected']}"
        for value in values
        if not value["matches"]
    ]


def write(values):
    updated = 0
    for value in values:
        if value["matches"]:
            continue
        value["mapping"]["source_contract"] = value["expected"]
        write_json(value["path"], value["mapping"])
        updated += 1
    return updated


def run_self_test():
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        contracts = root / "contracts"
        contracts.mkdir()
        old = root / "old.json"
        fresh = contracts / "AnimGraph.source.json"
        previous_index = root / "previous-index.json"
        mapping_path = root / "map.visible.json"
        write_json(
            old,
            {
                "source_graph": {"object_path": "/Asset:AnimGraph"},
                "source_graph_suffix": "AnimGraph",
            },
        )
        write_json(fresh, {"source_graph": "AnimGraph"})
        write_json(
            contracts / "index.json",
            {
                "format": "als_ue_visible_graph_contract_batch_v1",
                "source_asset": "/Asset",
                "root_source_graph": "AnimGraph",
                "inventory_sha256": "fixture",
                "entries": [
                    {"source_graph": "AnimGraph", "contract": "AnimGraph.source.json"}
                ],
            },
        )
        write_json(
            previous_index,
            {
                "format": "als_ue_visible_graph_contract_batch_v1",
                "source_asset": "/Asset",
                "root_source_graph": "AnimGraph",
                "inventory_sha256": "fixture",
                "entries": [{"source_graph": "AnimGraph", "contract": "old.json"}],
            },
        )
        write_json(
            mapping_path,
            {
                "format": "als_ue_visible_animgraph_map_v2",
                "source_contract": "old.json",
            },
        )
        old.unlink()
        values = plans(
            root,
            contracts / "index.json",
            "*.visible.json",
            previous_index,
        )
        assert len(check(values)) == 1
        assert write(values) == 1
        values = plans(
            root,
            contracts / "index.json",
            "*.visible.json",
            previous_index,
        )
        assert not check(values)
        mapping = read_json(mapping_path)
        assert mapping["source_contract"] == "contracts/AnimGraph.source.json"
    return {
        "source_graph_rebinding": "PASS",
        "missing_old_contract_rebound_from_prior_index": "PASS",
        "stale_contract_detection": "PASS",
        "write_then_check_round_trip": "PASS",
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--index", type=Path)
    parser.add_argument("--previous-index", type=Path)
    parser.add_argument("--map-glob")
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
        return 0
    if args.index is None or args.map_glob is None or args.write == args.check:
        raise ValueError("index, map-glob, and exactly one mode are required")
    root = args.root.resolve()
    index = args.index if args.index.is_absolute() else root / args.index
    previous_index = args.previous_index
    if previous_index is not None and not previous_index.is_absolute():
        previous_index = root / previous_index
    values = plans(root, index, args.map_glob, previous_index)
    if args.write:
        updated = write(values)
        print(json.dumps({"maps": len(values), "updated": updated}, ensure_ascii=False))
        return 0
    issues = check(values)
    print(json.dumps({"maps": len(values), "issues": issues}, ensure_ascii=False, indent=2))
    return 0 if not issues else 1


if __name__ == "__main__":
    raise SystemExit(main())
