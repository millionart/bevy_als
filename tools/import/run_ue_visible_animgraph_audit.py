import argparse
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

from audit_ue_visible_animgraph import audit, read_json
from extract_ue_visible_graph_contract import canonical_hash, contract


ROOT = Path(__file__).resolve().parents[2]
SPEC_DUMP_TIMEOUT_SECONDS = 90


def target_asset_path(root, target_graph):
    path = (root / target_graph).resolve()
    for asset_root_name in ("assets", "tools"):
        asset_root = (root / asset_root_name).resolve()
        try:
            return asset_root_name, path.relative_to(asset_root).as_posix()
        except ValueError:
            continue
    raise ValueError("target graph must be inside assets or tools")


def terminate_process_tree(process):
    if process.poll() is not None:
        return False
    if os.name == "nt":
        subprocess.run(
            ["taskkill", "/PID", str(process.pid), "/T", "/F"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )
    else:
        process.kill()
    return True


def run_with_timeout(command, root, environment, timeout_seconds):
    process = subprocess.Popen(
        command,
        cwd=root,
        env=environment,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    try:
        stdout, stderr = process.communicate(timeout=timeout_seconds)
    except subprocess.TimeoutExpired:
        process_tree_terminated = terminate_process_tree(process)
        stdout, stderr = process.communicate()
        raise RuntimeError(
            "command timed out after "
            + str(timeout_seconds)
            + " seconds; process_tree_terminated="
            + str(process_tree_terminated)
            + "; output="
            + (stderr or stdout)[-4000:]
        )
    return subprocess.CompletedProcess(command, process.returncode, stdout, stderr)


def run_spec_dump(root, target_graph, output, jobs):
    environment = os.environ.copy()
    asset_root, graph_path = target_asset_path(root, target_graph)
    environment["ALS_GRAPH_SPEC_ASSET_ROOT"] = asset_root
    environment["ALS_GRAPH_SPEC_PATH"] = graph_path
    environment["ALS_GRAPH_SPEC_OUTPUT"] = str(output)
    result = run_with_timeout(
        ["cargo", "run", "--example", "animgraph_spec_dump", "--jobs", str(jobs)],
        root,
        environment,
        SPEC_DUMP_TIMEOUT_SECONDS,
    )
    if result.returncode:
        raise RuntimeError(
            "animgraph spec dump failed: " + (result.stderr or result.stdout)[-4000:]
        )


def verify_source_contract(root, mapping, source_path):
    source_contract = mapping.get("source_contract")
    if not isinstance(source_contract, str) or not source_contract:
        raise ValueError("visible graph map source_contract is invalid")
    contract_path = (root / source_contract).resolve()
    if not contract_path.is_file():
        raise ValueError("visible graph source contract is missing")
    existing = read_json(contract_path)
    suffix = existing.get("source_graph_suffix")
    if not isinstance(suffix, str) or not suffix:
        raise ValueError("visible graph source contract suffix is invalid")
    current = contract(read_json(source_path), suffix)
    if canonical_hash(current) != canonical_hash(existing):
        raise ValueError("visible graph source contract differs from native extraction")


def run(root, map_path, jobs, source_path=None):
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
        report = audit(root, mapping)
        if source_path is not None:
            verify_source_contract(root, mapping, source_path)
            report["source_contract_fresh"] = True
        return report
    finally:
        output.unlink(missing_ok=True)


def run_self_test():
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        assets = root / "assets" / "graphs"
        tools = root / "tools" / "graphs"
        assets.mkdir(parents=True)
        tools.mkdir(parents=True)
        graph = assets / "graph.animgraph.ron"
        tool_graph = tools / "tool_graph.animgraph.ron"
        graph.write_text("()\n", encoding="utf-8")
        tool_graph.write_text("()\n", encoding="utf-8")
        assert target_asset_path(root, "assets/graphs/graph.animgraph.ron") == (
            "assets",
            "graphs/graph.animgraph.ron",
        )
        assert target_asset_path(root, "tools/graphs/tool_graph.animgraph.ron") == (
            "tools",
            "graphs/tool_graph.animgraph.ron",
        )
        try:
            target_asset_path(root, "outside.animgraph.ron")
        except ValueError:
            pass
        else:
            raise AssertionError("outside graph path was accepted")
        source = {
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
                }
            ],
        }
        source_path = root / "source.json"
        source_path.write_text(json.dumps(source), encoding="utf-8")
        contract_path = root / "contract.json"
        contract_path.write_text(
            json.dumps(contract(source, "AnimGraph")),
            encoding="utf-8",
        )
        mapping = {"source_contract": "contract.json"}
        verify_source_contract(root, mapping, source_path)
        source["graphs"][0]["name"] = "Changed"
        source_path.write_text(json.dumps(source), encoding="utf-8")
        try:
            verify_source_contract(root, mapping, source_path)
        except ValueError:
            pass
        else:
            raise AssertionError("stale source contract was accepted")
        child = root / "child.py"
        child.write_text("import time\ntime.sleep(60)\n", encoding="utf-8")
        parent = root / "parent.py"
        parent.write_text(
            "import subprocess, sys, time\nsubprocess.Popen([sys.executable, sys.argv[1]])\ntime.sleep(60)\n",
            encoding="utf-8",
        )
        try:
            run_with_timeout(
                [sys.executable, str(parent), str(child)],
                root,
                os.environ.copy(),
                0.01,
            )
        except RuntimeError as error:
            assert "process_tree_terminated=True" in str(error)
        else:
            raise AssertionError("timed-out command was accepted")
    return {
        "asset_relative_path": "PASS",
        "tool_asset_root_supported": "PASS",
        "outside_graph_rejected": "PASS",
        "native_source_contract_rechecked": "PASS",
        "timed_out_spec_dump_process_tree_terminated": "PASS",
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--map", type=Path)
    parser.add_argument("--source", type=Path)
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
    source_path = None
    if args.source is not None:
        source_path = args.source if args.source.is_absolute() else root / args.source
        if not source_path.is_file():
            raise ValueError("source extraction is missing")
    report = run(root, args.map, args.jobs, source_path)
    if args.summary:
        report = {
            "pass": report["pass"],
            "source_visible_nodes": report.get("source_visible_nodes"),
            "target_nodes": report.get("target_nodes"),
            "source_links": report.get("source_links"),
            "target_edges": report.get("target_edges"),
            "differences": len(report.get("differences", [])),
            "source_contract_fresh": report.get("source_contract_fresh", False),
        }
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if report["pass"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
