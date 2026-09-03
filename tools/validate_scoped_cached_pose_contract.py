import argparse
import hashlib
import json
import re
import tempfile
from pathlib import Path

from animgraph_context_contract import parse_nodes


FORMAT = "als_scoped_cached_pose_contract_v3"
SOURCE_FORMAT = "als_ue_visible_graph_contract_v2"
CACHE_TITLE = re.compile(r"^(?:Save|Use) cached pose '([^']+)'$")


def read_text(path):
    return path.read_text(encoding="utf-8")


def read_json(path):
    return json.loads(read_text(path))


def canonical_hash(value):
    return hashlib.sha256(
        json.dumps(value, ensure_ascii=False, separators=(",", ":"), sort_keys=True).encode("utf-8")
    ).hexdigest()


def repo_path(root, value):
    if not isinstance(value, str) or not value:
        raise ValueError("repository path is invalid")
    path = (root / value).resolve()
    try:
        path.relative_to(root.resolve())
    except ValueError as error:
        raise ValueError(f"path escapes repository root: {value}") from error
    return path


def cache_name(title):
    if not isinstance(title, str):
        return None
    return CACHE_TITLE.fullmatch(title.split("\n", 1)[0]).group(1) if CACHE_TITLE.fullmatch(title.split("\n", 1)[0]) else None


def source_cache_names(contract):
    graph = contract.get("source_graph")
    nodes = graph.get("nodes") if isinstance(graph, dict) else None
    if not isinstance(nodes, list):
        raise ValueError("source graph nodes are invalid")
    return {
        value
        for node in nodes
        if isinstance(node, dict)
        for value in [cache_name(node.get("title"))]
        if value is not None and node.get("title", "").startswith("Save cached pose")
    }


def source_use_names(contract):
    graph = contract.get("source_graph")
    nodes = graph.get("nodes") if isinstance(graph, dict) else None
    if not isinstance(nodes, list):
        raise ValueError("source graph nodes are invalid")
    return {
        value
        for node in nodes
        if isinstance(node, dict)
        for value in [cache_name(node.get("title"))]
        if value is not None and node.get("title", "").startswith("Use cached pose")
    }


def field_string(entry, name):
    match = re.search(rf'\b{re.escape(name)}\s*:\s*"([^"\\]*(?:\\.[^"\\]*)*)"', entry)
    return json.loads(f'"{match.group(1)}"') if match is not None else None


def scoped_use_nodes(path):
    nodes, _ = parse_nodes(read_text(path))
    return [
        node
        for node in nodes.values()
        if '"als_ext::SourceScopedCachedPoseUseNode"' in node["entry"]
    ]


def shape_issues(spec):
    allowed = {
        "format",
        "source_contract",
        "source_use_contracts",
        "cache_bindings",
        "target_cache_graph",
        "target_graphs",
        "transition_defaults",
    }
    if not isinstance(spec, dict) or set(spec) != allowed:
        return ["specification shape is invalid"]
    issues = []
    if spec.get("format") != FORMAT:
        issues.append("specification format is invalid")
    source = spec.get("source_contract")
    if not isinstance(source, dict) or set(source) != {"path", "sha256"}:
        issues.append("source contract is invalid")
    elif not isinstance(source.get("path"), str) or not re.fullmatch(r"[0-9a-f]{64}", source.get("sha256", "")):
        issues.append("source contract fields are invalid")
    for key in ("source_use_contracts", "target_graphs"):
        values = spec.get(key)
        if not isinstance(values, list) or not values or any(not isinstance(value, str) or not value for value in values):
            issues.append(f"{key} is invalid")
        elif len(values) != len(set(values)):
            issues.append(f"{key} has duplicates")
    bindings = spec.get("cache_bindings")
    if not isinstance(bindings, dict) or not bindings:
        issues.append("cache bindings are invalid")
    elif any(
        not isinstance(source, str)
        or not source
        or not isinstance(target, str)
        or not target
        for source, target in bindings.items()
    ):
        issues.append("cache binding values are invalid")
    elif len(bindings.values()) != len(set(bindings.values())):
        issues.append("cache binding scope keys are duplicated")
    if not isinstance(spec.get("target_cache_graph"), str) or not spec["target_cache_graph"]:
        issues.append("target cache graph is invalid")
    transition_defaults = spec.get("transition_defaults")
    if not isinstance(transition_defaults, list) or not transition_defaults:
        issues.append("transition defaults are invalid")
    else:
        seen = set()
        for value in transition_defaults:
            if not isinstance(value, dict) or set(value) != {"graph", "node", "key", "default_value"}:
                issues.append("transition default entry is invalid")
                continue
            if any(not isinstance(value.get(key), str) or not value[key] for key in ("graph", "node", "key")):
                issues.append("transition default fields are invalid")
            if not isinstance(value.get("default_value"), (int, float)):
                issues.append("transition default value is invalid")
            identity = (value.get("graph"), value.get("node"))
            if identity in seen:
                issues.append("transition defaults are duplicated")
            seen.add(identity)
    return issues


def audit(root, spec):
    differences = shape_issues(spec)
    report = {"format": FORMAT, "differences": differences}
    if differences:
        report["pass"] = False
        return report
    source_spec = spec["source_contract"]
    source_path = repo_path(root, source_spec["path"])
    source_contract = read_json(source_path)
    if source_contract.get("format") != SOURCE_FORMAT:
        differences.append("source contract format")
    if canonical_hash(source_contract) != source_spec["sha256"]:
        differences.append("source contract hash")
    bindings = spec["cache_bindings"]
    declared_caches = set(bindings)
    try:
        actual_caches = source_cache_names(source_contract)
    except ValueError as error:
        differences.append(str(error))
        actual_caches = set()
    if actual_caches != declared_caches:
        differences.append("source cache ownership")
    source_uses = set()
    for value in spec["source_use_contracts"]:
        contract = read_json(repo_path(root, value))
        if contract.get("format") != SOURCE_FORMAT:
            differences.append(f"source use contract format: {value}")
            continue
        try:
            source_uses.update(source_use_names(contract))
        except ValueError as error:
            differences.append(f"source use contract: {value}: {error}")
    if source_uses != declared_caches:
        differences.append("source cache consumer coverage")
    target_cache_graph_path = repo_path(root, spec["target_cache_graph"])
    try:
        target_cache_nodes, _ = parse_nodes(read_text(target_cache_graph_path))
        target_save_bindings = {
            cache_name(node["name"]): field_string(node["entry"], "cache_key")
            for node in target_cache_nodes.values()
            if '"als_ext::SourceCachedPoseSaveNode"' in node["entry"]
            and cache_name(node["name"]) in declared_caches
        }
    except ValueError as error:
        differences.append(f"target cache graph: {error}")
        target_save_bindings = {}
    if target_save_bindings != bindings:
        differences.append("target cache scope bindings")
    target_uses = []
    for value in spec["target_graphs"]:
        path = repo_path(root, value)
        try:
            nodes = scoped_use_nodes(path)
        except ValueError as error:
            differences.append(f"target graph: {value}: {error}")
            continue
        if not nodes:
            differences.append(f"target graph has no scoped cache use: {value}")
        for node in nodes:
            cache_value = field_string(node["entry"], "cache_node")
            cache = cache_name(cache_value) or cache_value
            scoped_key = field_string(node["entry"], "cache_key")
            target_uses.append(
                {"graph": value, "node": node["name"], "cache": cache, "cache_key": scoped_key}
            )
            if cache not in declared_caches:
                differences.append(f"target cache is not source owned: {value}:{node['name']}")
            elif scoped_key != bindings[cache]:
                differences.append(f"target cache scope key is wrong: {value}:{node['name']}")
    target_cache_names = {value["cache"] for value in target_uses if isinstance(value["cache"], str)}
    if target_cache_names != declared_caches:
        differences.append("target cache consumer coverage")
    transition_defaults = []
    for expected in spec["transition_defaults"]:
        graph_path = repo_path(root, expected["graph"])
        try:
            nodes, _ = parse_nodes(read_text(graph_path))
        except ValueError as error:
            differences.append(f"transition default graph: {expected['graph']}: {error}")
            continue
        node = next((node for node in nodes.values() if node["name"] == expected["node"]), None)
        if node is None or '"als_ext::DirectTransitionEventNode"' not in node["entry"]:
            differences.append(f"transition default node is missing: {expected['graph']}:{expected['node']}")
            continue
        actual_key = field_string(node["entry"], "key")
        actual_default = field_string(node["entry"], "default_value")
        if actual_default is None:
            match = re.search(r"\bdefault_value\s*:\s*(-?(?:\d+(?:\.\d*)?|\.\d+))", node["entry"])
            actual_default = match.group(1) if match is not None else None
        actual_default = float(actual_default) if actual_default is not None else None
        transition_defaults.append(
            {
                "graph": expected["graph"],
                "node": expected["node"],
                "key": actual_key,
                "default_value": actual_default,
            }
        )
        if actual_key != expected["key"] or actual_default != float(expected["default_value"]):
            differences.append(f"transition no-event default is wrong: {expected['graph']}:{expected['node']}")
    report.update(
        {
            "source_contract": source_spec["path"],
            "source_cache_nodes": sorted(actual_caches),
            "source_use_nodes": sorted(source_uses),
            "target_cache_graph": spec["target_cache_graph"],
            "target_cache_bindings": target_save_bindings,
            "target_uses": target_uses,
            "transition_defaults": transition_defaults,
            "pass": not differences,
        }
    )
    return report


def fixture_contract(nodes):
    return {"format": SOURCE_FORMAT, "source_graph": {"nodes": nodes}}


def fixture_spec(source_hash):
    return {
        "format": FORMAT,
        "source_contract": {"path": "source.json", "sha256": source_hash},
        "source_use_contracts": ["use.json"],
        "cache_bindings": {"Movement": "scope:movement"},
        "target_cache_graph": "assets/graphs/owner.animgraph.ron",
        "target_graphs": ["child.animgraph.ron"],
        "transition_defaults": [
            {
                "graph": "transition.animgraph.ron",
                "node": "transition driver",
                "key": "transition_index",
                "default_value": -1.0,
            }
        ],
    }


def run_self_test():
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        source = fixture_contract([{"title": "Save cached pose 'Movement'"}])
        use = fixture_contract([{"title": "Use cached pose 'Movement'"}])
        (root / "source.json").write_text(json.dumps(source), encoding="utf-8")
        (root / "use.json").write_text(json.dumps(use), encoding="utf-8")
        owner = root / "assets" / "graphs"
        owner.mkdir(parents=True)
        (owner / "owner.animgraph.ron").write_text(
            '(nodes: [(id: "owner", name: "Save cached pose \'Movement\'", inner: { "als_ext::SourceCachedPoseSaveNode": (cache_key: "scope:movement") })])',
            encoding="utf-8",
        )
        child = root / "child.animgraph.ron"
        child.write_text(
            '(nodes: [(id: "child", name: "Use cached pose \'Movement\'", inner: { "als_ext::SourceScopedCachedPoseUseNode": (cache_node: "Movement", cache_key: "scope:movement") })])',
            encoding="utf-8",
        )
        transition = root / "transition.animgraph.ron"
        transition.write_text(
            '(nodes: [(id: "transition", name: "transition driver", inner: { "als_ext::DirectTransitionEventNode": (key: "transition_index", default_value: -1.0) })])',
            encoding="utf-8",
        )
        spec = fixture_spec(canonical_hash(source))
        assert audit(root, spec)["pass"]
        child.write_text(
            '(nodes: [(id: "child", name: "Use cached pose \'Movement\'", inner: { "als_ext::SourceScopedCachedPoseUseNode": (cache_node: "Movement") })])',
            encoding="utf-8",
        )
        assert not audit(root, spec)["pass"]
        child.write_text(
            '(nodes: [(id: "child", name: "Use cached pose \'Movement\'", inner: { "als_ext::SourceScopedCachedPoseUseNode": (cache_node: "Movement", cache_key: "scope:movement") })])',
            encoding="utf-8",
        )
        transition.write_text(
            '(nodes: [(id: "transition", name: "transition driver", inner: { "als_ext::DirectTransitionEventNode": (key: "transition_index", default_value: 0.0) })])',
            encoding="utf-8",
        )
        assert not audit(root, spec)["pass"]
        transition.write_text(
            '(nodes: [(id: "transition", name: "transition driver", inner: { "als_ext::DirectTransitionEventNode": (key: "transition_index", default_value: -1.0) })])',
            encoding="utf-8",
        )
        spec["source_contract"]["sha256"] = "0" * 64
        assert not audit(root, spec)["pass"]
    return {
        "scoped_cache_key_required": "PASS",
        "missing_scope_key_rejected": "PASS",
        "transition_default_rejected": "PASS",
        "stale_source_contract_rejected": "PASS",
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--spec", type=Path)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
        return 0
    if args.spec is None or not args.check:
        raise ValueError("--spec and --check are required")
    root = args.root.resolve()
    spec_path = repo_path(root, args.spec.as_posix())
    report = audit(root, read_json(spec_path))
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if report["pass"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
