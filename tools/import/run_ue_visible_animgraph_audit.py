import argparse
import json
import os
import subprocess
import tempfile
from pathlib import Path

from audit_ue_visible_animgraph import audit, read_json


ROOT = Path(__file__).resolve().parents[2]


def target_asset_path(root, target_graph):
    path = (root / target_graph).resolve()
    assets = (root / "assets").resolve()
    try:
        return path.relative_to(assets).as_posix()
    except ValueError as error:
        raise ValueError("target graph must be inside assets") from error


def run_spec_dump(root, target_graph, output, jobs):
    environment = os.environ.copy()
    environment["ALS_GRAPH_SPEC_PATH"] = target_asset_path(root, target_graph)
    environment["ALS_GRAPH_SPEC_OUTPUT"] = str(output)
    result = subprocess.run(
        ["cargo", "run", "--example", "animgraph_spec_dump", "--jobs", str(jobs)],
        cwd=root,
        env=environment,
        text=True,
        capture_output=True,
        timeout=300,
        check=False,
    )
    if result.returncode:
        raise RuntimeError(
            "animgraph spec dump failed: " + (result.stderr or result.stdout)[-4000:]
        )


def run(root, map_path, jobs):
    mapping = read_json(root / map_path)
    target_graph = mapping.get("target_graph")
    if not isinstance(target_graph, str) or not target_graph:
        raise ValueError("visible graph map target_graph is invalid")
    target_dir = root / "target"
    target_dir.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(
        mode="w",
        encoding="utf-8",
        suffix=".json",
        prefix="ue_visible_animgraph_spec_",
        dir=target_dir,
        delete=False,
    ) as file:
        output = Path(file.name)
    try:
        run_spec_dump(root, target_graph, output, jobs)
        mapping = dict(mapping)
        mapping["target_spec"] = output.relative_to(root).as_posix()
        return audit(root, mapping)
    finally:
        output.unlink(missing_ok=True)


def run_self_test():
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        assets = root / "assets" / "graphs"
        assets.mkdir(parents=True)
        graph = assets / "graph.animgraph.ron"
        graph.write_text("()\n", encoding="utf-8")
        assert target_asset_path(root, "assets/graphs/graph.animgraph.ron") == "graphs/graph.animgraph.ron"
        try:
            target_asset_path(root, "outside.animgraph.ron")
        except ValueError:
            pass
        else:
            raise AssertionError("outside graph path was accepted")
    return {"asset_relative_path": "PASS", "outside_graph_rejected": "PASS"}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--map", type=Path)
    parser.add_argument("--jobs", type=int, default=2)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--summary", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
        return 0
    if args.map is None or not args.check or args.jobs < 1:
        raise ValueError("map, --check, and a positive jobs value are required")
    root = args.root.resolve()
    report = run(root, args.map, args.jobs)
    if args.summary:
        report = {
            "pass": report["pass"],
            "source_visible_nodes": report.get("source_visible_nodes"),
            "target_nodes": report.get("target_nodes"),
            "source_links": report.get("source_links"),
            "target_edges": report.get("target_edges"),
            "differences": len(report.get("differences", [])),
        }
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if report["pass"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
