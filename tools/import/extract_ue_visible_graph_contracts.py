import argparse
import hashlib
import importlib.util
import json
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
FORMAT = "als_ue_visible_graph_contract_batch_v1"


def load_module(name, filename):
    specification = importlib.util.spec_from_file_location(name, filename)
    module = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(module)
    return module


CONTRACT = load_module(
    "extract_ue_visible_graph_contract",
    ROOT / "tools" / "import" / "extract_ue_visible_graph_contract.py",
)
INVENTORY = load_module(
    "extract_ue_authored_graph_inventory",
    ROOT / "tools" / "import" / "extract_ue_authored_graph_inventory.py",
)


def canonical_hash(value):
    return CONTRACT.canonical_hash(value)


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def contract_filename(source_graph):
    digest = hashlib.sha256(source_graph.encode("utf-8")).hexdigest()[:16]
    return f"graph-{digest}.source.json"


def batch(source, root_graph_suffix, output_dir):
    inventory = INVENTORY.inventory(source, root_graph_suffix)
    entries = []
    payloads = {}
    for entry in inventory["entries"]:
        source_graph = entry["source_graph"]
        value = CONTRACT.contract(source, source_graph)
        filename = contract_filename(source_graph)
        entries.append(
            {
                "source_graph": source_graph,
                "kind": entry["kind"],
                "contract": filename,
                "source_graph_sha256": value["source_graph_sha256"],
            }
        )
        payloads[filename] = value
    return {
        "format": FORMAT,
        "source_asset": source["asset_path"],
        "root_source_graph": root_graph_suffix,
        "inventory_sha256": canonical_hash(inventory),
        "entries": entries,
    }, payloads


def generated_paths(output_dir, index, payloads):
    result = {output_dir / "index.json": index}
    result.update({output_dir / filename: value for filename, value in payloads.items()})
    return result


def write_batch(output_dir, index, payloads):
    for path, value in generated_paths(output_dir, index, payloads).items():
        write_json(path, value)


def check_batch(output_dir, index, payloads):
    issues = []
    expected_paths = generated_paths(output_dir, index, payloads)
    for path, expected in expected_paths.items():
        if not path.is_file():
            issues.append(f"missing: {path.as_posix()}")
            continue
        if canonical_hash(read_json(path)) != canonical_hash(expected):
            issues.append(f"different: {path.as_posix()}")
    actual_contracts = set(output_dir.glob("*.source.json")) if output_dir.is_dir() else set()
    expected_contracts = {
        path for path in expected_paths if path.name.endswith(".source.json")
    }
    for path in sorted(actual_contracts - expected_contracts):
        issues.append(f"stale: {path.as_posix()}")
    return issues


def fixture_source():
    return {
        "asset_path": "/Asset",
        "asset_objects": [],
        "graphs": [
            {
                "index": 0,
                "object_path": "/Asset:AnimGraph",
                "class_path": "/Script/AnimGraph.AnimationGraph",
                "name": "AnimGraph",
                "properties": [],
                "nodes": [],
            },
            {
                "index": 1,
                "object_path": "/Asset:AnimGraph.Fsm",
                "class_path": "/Script/AnimGraph.AnimationStateMachineGraph",
                "name": "Fsm",
                "properties": [],
                "nodes": [],
            },
        ],
    }


def run_self_test():
    source = fixture_source()
    index, payloads = batch(source, "AnimGraph", Path("contracts"))
    assert len(index["entries"]) == 2
    assert len(payloads) == 2
    assert len(set(payloads)) == 2
    long_name = contract_filename("AnimGraph." + "Nested.Graph." * 100)
    assert len(long_name) <= 40
    assert long_name.startswith("graph-")
    with tempfile.TemporaryDirectory() as directory:
        output_dir = Path(directory) / "contracts"
        write_batch(output_dir, index, payloads)
        assert not check_batch(output_dir, index, payloads)
        volatile_source = fixture_source()
        volatile_source["graphs"][0]["nodes"] = [
            {
                "object_path": "/Asset:AnimGraph.Source",
                "node_guid": "source",
                "pins": [
                    {
                        "pin_id": "first-pin-id",
                        "name": "Pose",
                        "direction": "Output",
                        "linked_to": [],
                    }
                ],
            }
        ]
        volatile_index, volatile_payloads = batch(
            volatile_source, "AnimGraph", Path("volatile")
        )
        volatile_dir = Path(directory) / "volatile"
        write_batch(volatile_dir, volatile_index, volatile_payloads)
        volatile_source["graphs"][0]["nodes"][0]["pins"][0]["pin_id"] = "second-pin-id"
        regenerated_index, regenerated_payloads = batch(
            volatile_source, "AnimGraph", Path("volatile")
        )
        assert not check_batch(volatile_dir, regenerated_index, regenerated_payloads)
        stale = output_dir / "stale.source.json"
        write_json(stale, {})
        assert any("stale" in issue for issue in check_batch(output_dir, index, payloads))
    return {
        "recursive_contract_extraction": "PASS",
        "deterministic_unique_contract_names": "PASS",
        "bounded_contract_filenames": "PASS",
        "volatile_pin_ids_do_not_fail_batch_freshness": "PASS",
        "write_check_and_stale_detection": "PASS",
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path)
    parser.add_argument("--root-graph-suffix")
    parser.add_argument("--output-dir", type=Path)
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
        return 0
    if (
        args.source is None
        or args.root_graph_suffix is None
        or args.output_dir is None
        or args.write == args.check
    ):
        raise ValueError(
            "source, root-graph-suffix, output-dir, and exactly one mode are required"
        )
    index, payloads = batch(
        read_json(args.source),
        args.root_graph_suffix,
        args.output_dir,
    )
    if args.write:
        write_batch(args.output_dir, index, payloads)
        print(
            json.dumps(
                {
                    "output_dir": args.output_dir.as_posix(),
                    "entries": len(index["entries"]),
                },
                ensure_ascii=False,
            )
        )
        return 0
    issues = check_batch(args.output_dir, index, payloads)
    print(json.dumps({"issues": issues}, ensure_ascii=False, indent=2))
    return 0 if not issues else 1


if __name__ == "__main__":
    raise SystemExit(main())
