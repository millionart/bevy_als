import argparse
import hashlib
import importlib.util
import json
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
FORMAT = "als_ue_transition_expression_graph_batch_v1"


def load_module(name, filename):
    specification = importlib.util.spec_from_file_location(name, filename)
    module = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(module)
    return module


GENERATOR = load_module(
    "generate_ue_transition_expression_graph",
    ROOT / "tools" / "import" / "generate_ue_transition_expression_graph.py",
)


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def canonical(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def filename(index, source_graph):
    digest = hashlib.sha256(source_graph.encode("utf-8")).hexdigest()[:12]
    return f"ue_source_transition_{index:02d}_{digest}"


def batch(root, inventory_path, contract_dir, target_dir, map_dir, map_prefix):
    inventory = read_json(root / inventory_path)
    entries = [entry for entry in inventory.get("entries", []) if entry.get("kind") == "transition"]
    if not entries:
        raise ValueError("transition inventory is empty")
    result = []
    generated = {}
    for index, entry in enumerate(entries):
        source_graph = entry.get("source_graph")
        contract_name = entry.get("contract")
        if not isinstance(source_graph, str) or not isinstance(contract_name, str):
            raise ValueError("transition inventory entry is invalid")
        stem = filename(index, source_graph)
        source_contract = contract_dir / contract_name
        target_graph = target_dir / f"{stem}.animgraph.ron"
        map_path = map_dir / f"{map_prefix}.{stem}.visible.json"
        text, mapping = GENERATOR.generate(root, source_contract, target_graph, map_path)
        generated[target_graph] = text
        generated[map_path] = mapping
        result.append(
            {
                "source_graph": source_graph,
                "source_contract": source_contract.as_posix(),
                "target_graph": target_graph.as_posix(),
                "map": map_path.as_posix(),
            }
        )
    return {
        "format": FORMAT,
        "inventory": inventory_path.as_posix(),
        "entries": result,
    }, generated


def write_batch(root, index_path, index, generated):
    write_json(root / index_path, index)
    for path, value in generated.items():
        output = root / path
        if isinstance(value, str):
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_text(value, encoding="utf-8")
        else:
            write_json(output, value)


def check_batch(root, index_path, index, generated, target_dir, map_dir, map_prefix):
    issues = []
    index_file = root / index_path
    if not index_file.is_file() or canonical(read_json(index_file)) != canonical(index):
        issues.append("batch index differs")
    for path, expected in generated.items():
        output = root / path
        if not output.is_file():
            issues.append(f"missing: {path.as_posix()}")
            continue
        current = output.read_text(encoding="utf-8") if isinstance(expected, str) else read_json(output)
        if (current != expected) if isinstance(expected, str) else (canonical(current) != canonical(expected)):
            issues.append(f"different: {path.as_posix()}")
    expected_graphs = {root / path for path, value in generated.items() if isinstance(value, str)}
    expected_maps = {root / path for path, value in generated.items() if not isinstance(value, str)}
    for path in (root / target_dir).glob("ue_source_transition_*.animgraph.ron"):
        if path not in expected_graphs:
            issues.append(f"stale: {path.relative_to(root).as_posix()}")
    for path in (root / map_dir).glob(f"{map_prefix}.ue_source_transition_*.visible.json"):
        if path not in expected_maps:
            issues.append(f"stale: {path.relative_to(root).as_posix()}")
    return issues


def run_self_test():
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        contract_dir = Path("contracts")
        contract_name = "transition.source.json"
        write_json(root / contract_dir / contract_name, GENERATOR.fixture_contract())
        inventory_path = Path("inventory.json")
        write_json(
            root / inventory_path,
            {
                "entries": [
                    {
                        "kind": "transition",
                        "source_graph": "AnimGraph.Transition",
                        "contract": contract_name,
                    }
                ]
            },
        )
        index, generated = batch(
            root,
            inventory_path,
            contract_dir,
            Path("assets/generated"),
            Path("maps"),
            "fixture",
        )
        index_path = Path("batch.json")
        write_batch(root, index_path, index, generated)
        assert not check_batch(
            root,
            index_path,
            index,
            generated,
            Path("assets/generated"),
            Path("maps"),
            "fixture",
        )
        stale = root / "assets/generated/ue_source_transition_99_stale.animgraph.ron"
        stale.write_text("()\n", encoding="utf-8")
        assert any(
            "stale" in issue
            for issue in check_batch(
                root,
                index_path,
                index,
                generated,
                Path("assets/generated"),
                Path("maps"),
                "fixture",
            )
        )
    return {
        "recursive_transition_generation": "PASS",
        "generated_source_map_coverage": "PASS",
        "write_check_and_stale_detection": "PASS",
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--inventory", type=Path)
    parser.add_argument("--contract-dir", type=Path)
    parser.add_argument("--target-dir", type=Path)
    parser.add_argument("--map-dir", type=Path)
    parser.add_argument("--map-prefix")
    parser.add_argument("--output-index", type=Path)
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
        return 0
    if (
        args.inventory is None
        or args.contract_dir is None
        or args.target_dir is None
        or args.map_dir is None
        or not args.map_prefix
        or args.output_index is None
        or args.write == args.check
    ):
        raise ValueError("all paths, map-prefix, and exactly one mode are required")
    root = args.root.resolve()
    index, generated = batch(
        root,
        args.inventory,
        args.contract_dir,
        args.target_dir,
        args.map_dir,
        args.map_prefix,
    )
    if args.write:
        write_batch(root, args.output_index, index, generated)
        print(json.dumps({"entries": len(index["entries"])}, ensure_ascii=False))
        return 0
    issues = check_batch(
        root,
        args.output_index,
        index,
        generated,
        args.target_dir,
        args.map_dir,
        args.map_prefix,
    )
    print(json.dumps({"issues": issues}, ensure_ascii=False, indent=2))
    return 0 if not issues else 1


if __name__ == "__main__":
    raise SystemExit(main())
