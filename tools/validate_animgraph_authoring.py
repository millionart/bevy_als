import argparse
import hashlib
import json
import math
import re
import subprocess
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tools"))
sys.path.insert(0, str(ROOT / "tools" / "import"))

from animgraph_context_contract import parse_edges, parse_nodes
from apply_runtime_animation_context import audit as runtime_context_audit
from capture_animgraph_ron_topology import capture_fsm, parse_positions
from normalize_animgraph_io_order import compact, field_span, split_top_level, top_level_colon


FORMAT = "als_animgraph_authoring_gate_v1"
READ_ONLY_SCOPES = {"source", "layout", "editor"}
FORBIDDEN_COMMAND_ARGUMENTS = {"--fix", "--write", "--write-baseline"}
SOURCE_HASH_KINDS = {"bytes", "canonical_json"}
RECURSIVE_INVENTORY_FORMAT = "als_ue_authored_graph_inventory_v1"
FSM_MAP_FORMAT = "als_ue_visible_fsm_map_v1"


def read_text(path):
    return path.read_text(encoding="utf-8")


def read_json(path):
    return json.loads(read_text(path))


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def sha256_file(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def source_digest(path, kind):
    if kind == "bytes":
        return sha256_file(path)
    if kind == "canonical_json":
        value = read_json(path)
        encoded = json.dumps(
            value,
            ensure_ascii=False,
            separators=(",", ":"),
            sort_keys=True,
        ).encode("utf-8")
        return hashlib.sha256(encoded).hexdigest()
    raise ValueError(f"source hash kind is invalid: {kind}")


def repo_path(root, value):
    if not isinstance(value, str) or not value:
        raise ValueError("repository path is invalid")
    path = (root / value).resolve()
    try:
        path.relative_to(root.resolve())
    except ValueError as error:
        raise ValueError(f"path escapes repository root: {value}") from error
    return path


def input_times(text):
    _, start, end = field_span(text, "input_times")
    return [compact(value) for value in split_top_level(text[start + 1 : end], ",")]


def output_time(text):
    match = re.search(r"(?<![A-Za-z0-9_])output_time\s*:\s*(true|false)", text)
    if match is None:
        raise ValueError("output_time is missing")
    return match.group(1) == "true"


def data_key(value):
    text = value.strip()
    quoted = re.fullmatch(r'"([^"\\]*(?:\\.[^"\\]*)*)"', text)
    if quoted is not None:
        return json.loads(quoted.group(0))
    passthrough = re.fullmatch(r'Passthrough\("([^"\\]*(?:\\.[^"\\]*)*)"\)', text)
    if passthrough is not None:
        return json.loads(f'"{passthrough.group(1)}"')
    return compact(text)


def data_map(text, field_name):
    _, start, end = field_span(text, field_name)
    values = {}
    for entry in split_top_level(text[start + 1 : end], ","):
        key, value = top_level_colon(entry)
        name = data_key(key)
        if name in values:
            raise ValueError(f"duplicate data key {name}")
        values[name] = value.strip()
    return values


def io_payload(path, text):
    return {
        "input_data": data_map(text, "input_data"),
        "input_times": input_times(text),
        "output_time": output_time(text),
        "output_data": data_map(text, "output_data"),
    }


def normalized_edge(value):
    return {
        "target": compact(value["target"]),
        "source": compact(value["source"]),
    }


def expected_edge(value):
    if not isinstance(value, dict) or set(value) != {"target", "source"}:
        raise ValueError("expected edge must contain target and source")
    if not isinstance(value["target"], str) or not isinstance(value["source"], str):
        raise ValueError("expected edge values must be strings")
    return normalized_edge(value)


def expected_position(value):
    if not isinstance(value, list) or len(value) != 2:
        raise ValueError("expected position must contain two values")
    position = [float(value[0]), float(value[1])]
    if not all(math.isfinite(item) for item in position):
        raise ValueError("expected position must be finite")
    return position


def command_shape_issues(command):
    issues = []
    if not isinstance(command, dict):
        return ["verification command must be an object"]
    label = command.get("label")
    scope = command.get("scope")
    argv = command.get("argv")
    if not isinstance(label, str) or not label:
        issues.append("verification command label is invalid")
    if scope not in READ_ONLY_SCOPES:
        issues.append("verification command scope is invalid")
    if not isinstance(argv, list) or len(argv) < 2 or any(
        not isinstance(value, str) or not value for value in argv
    ):
        issues.append("verification command argv is invalid")
        return issues
    if argv[0] != "python":
        issues.append("verification command must use python")
    if not argv[1].startswith("tools/"):
        issues.append("verification command script must be under tools")
    if any(value in FORBIDDEN_COMMAND_ARGUMENTS for value in argv):
        issues.append("verification command is not read-only")
    timeout = command.get("timeout_seconds", 120)
    if not isinstance(timeout, int) or isinstance(timeout, bool) or not 1 <= timeout <= 600:
        issues.append("verification command timeout_seconds is invalid")
    return issues


def recursive_source_inventory_issues(value, require_digest=True):
    if value is None:
        return []
    allowed = {"path", "hash_kind"}
    if require_digest:
        allowed.add("sha256")
    if not isinstance(value, dict) or set(value) != allowed:
        return ["recursive_source_inventory is invalid"]
    path = value.get("path")
    if not isinstance(path, str) or not path:
        return ["recursive_source_inventory path is invalid"]
    if value.get("hash_kind", "bytes") not in SOURCE_HASH_KINDS:
        return ["recursive_source_inventory hash_kind is invalid"]
    if require_digest and (
        not isinstance(value.get("sha256"), str)
        or re.fullmatch(r"[0-9a-f]{64}", value["sha256"]) is None
    ):
        return ["recursive_source_inventory sha256 is invalid"]
    return []


def recursive_inventory_issues(value, source_inventory):
    if value is None:
        return ["recursive_inventory is required with recursive_source_inventory"] if source_inventory is not None else []
    if not isinstance(value, list) or not value:
        return ["recursive_inventory must be a non-empty array"]
    issues = []
    source_graphs = set()
    targets = set()
    allowed = {"kind", "source_graph", "source_contract", "source_map", "target_path", "gate_label"}
    for index, entry in enumerate(value):
        label = f"recursive_inventory[{index}]"
        if not isinstance(entry, dict) or set(entry) != allowed:
            issues.append(f"{label} is invalid")
            continue
        if entry.get("kind") not in {"graph", "fsm", "transition"}:
            issues.append(f"{label} kind is invalid")
        for key in ("source_graph", "source_contract", "source_map", "target_path", "gate_label"):
            if not isinstance(entry.get(key), str) or not entry[key]:
                issues.append(f"{label} {key} is invalid")
        source_graph = entry.get("source_graph")
        target = entry.get("target_path")
        if isinstance(source_graph, str) and source_graph:
            if source_graph in source_graphs:
                issues.append(f"duplicate recursive source graph {source_graph}")
            source_graphs.add(source_graph)
        if isinstance(target, str) and target:
            if target in targets and entry.get("kind") != "transition":
                issues.append(f"duplicate recursive target path {target}")
            if entry.get("kind") != "transition":
                targets.add(target)
    return issues


def manifest_shape_issues(manifest):
    issues = []
    if not isinstance(manifest, dict):
        return ["manifest must be an object"]
    if manifest.get("format") != FORMAT:
        issues.append("manifest format is invalid")
    if not isinstance(manifest.get("id"), str) or not manifest["id"]:
        issues.append("manifest id is invalid")
    evidence = manifest.get("source_evidence")
    if not isinstance(evidence, list) or not evidence:
        issues.append("source_evidence must be a non-empty array")
    else:
        names = set()
        for index, item in enumerate(evidence):
            label = f"source_evidence[{index}]"
            if not isinstance(item, dict):
                issues.append(f"{label} must be an object")
                continue
            path = item.get("path")
            digest = item.get("sha256")
            if not isinstance(path, str) or not path:
                issues.append(f"{label} path is invalid")
            elif path in names:
                issues.append(f"duplicate source evidence path {path}")
            else:
                names.add(path)
            if not isinstance(digest, str) or re.fullmatch(r"[0-9a-f]{64}", digest) is None:
                issues.append(f"{label} sha256 is invalid")
            kind = item.get("hash_kind", "bytes")
            if kind not in SOURCE_HASH_KINDS:
                issues.append(f"{label} hash_kind is invalid")
    graphs = manifest.get("graphs", [])
    if not isinstance(graphs, list):
        issues.append("graphs must be an array")
    else:
        paths = set()
        for index, graph in enumerate(graphs):
            label = f"graphs[{index}]"
            if not isinstance(graph, dict):
                issues.append(f"{label} must be an object")
                continue
            path = graph.get("path")
            if not isinstance(path, str) or not path.endswith(".animgraph.ron"):
                issues.append(f"{label} path is invalid")
            elif path in paths:
                issues.append(f"duplicate graph path {path}")
            else:
                paths.add(path)
            nodes = graph.get("expected_nodes")
            if not isinstance(nodes, list) or not nodes:
                issues.append(f"{label} expected_nodes must be a non-empty array")
            else:
                ids = set()
                for node_index, node in enumerate(nodes):
                    node_label = f"{label}.expected_nodes[{node_index}]"
                    if not isinstance(node, dict):
                        issues.append(f"{node_label} must be an object")
                        continue
                    identifier = node.get("id")
                    if not isinstance(identifier, str) or not identifier:
                        issues.append(f"{node_label} id is invalid")
                    elif identifier in ids:
                        issues.append(f"duplicate expected node id {identifier}")
                    else:
                        ids.add(identifier)
                    if not isinstance(node.get("name"), str) or not node["name"]:
                        issues.append(f"{node_label} name is invalid")
                    try:
                        expected_position(node.get("position"))
                    except (TypeError, ValueError):
                        issues.append(f"{node_label} position is invalid")
            edges = graph.get("expected_edges")
            if not isinstance(edges, list):
                issues.append(f"{label} expected_edges must be an array")
            else:
                try:
                    [expected_edge(value) for value in edges]
                except ValueError as error:
                    issues.append(f"{label} {error}")
            expected_io = graph.get("expected_io")
            if not isinstance(expected_io, dict) or set(expected_io) != {
                "input_data",
                "input_times",
                "output_time",
                "output_data",
            }:
                issues.append(f"{label} expected_io is invalid")
            elif (
                not isinstance(expected_io["input_data"], dict)
                or not isinstance(expected_io["input_times"], list)
                or not isinstance(expected_io["output_time"], bool)
                or not isinstance(expected_io["output_data"], dict)
            ):
                issues.append(f"{label} expected_io values are invalid")
            exceptions = graph.get("root_output_exceptions", [])
            if not isinstance(exceptions, list):
                issues.append(f"{label} root_output_exceptions must be an array")
            else:
                names = set()
                for exception_index, exception in enumerate(exceptions):
                    exception_label = f"{label}.root_output_exceptions[{exception_index}]"
                    if not isinstance(exception, dict):
                        issues.append(f"{exception_label} must be an object")
                        continue
                    name = exception.get("name")
                    if not isinstance(name, str) or not name:
                        issues.append(f"{exception_label} name is invalid")
                    elif name in names:
                        issues.append(f"duplicate root output exception {name}")
                    else:
                        names.add(name)
                    for key in ("reason", "evidence"):
                        if not isinstance(exception.get(key), str) or not exception[key]:
                            issues.append(f"{exception_label} {key} is invalid")
            runtime_manifest = graph.get("runtime_context_manifest")
            if runtime_manifest is not None and (
                not isinstance(runtime_manifest, str) or not runtime_manifest
            ):
                issues.append(f"{label} runtime_context_manifest is invalid")
    fsms = manifest.get("fsms", [])
    if not isinstance(fsms, list):
        issues.append("fsms must be an array")
    else:
        paths = set()
        for index, fsm in enumerate(fsms):
            label = f"fsms[{index}]"
            if not isinstance(fsm, dict):
                issues.append(f"{label} must be an object")
                continue
            path = fsm.get("path")
            if not isinstance(path, str) or not path.endswith(".fsm.ron"):
                issues.append(f"{label} path is invalid")
            elif path in paths:
                issues.append(f"duplicate FSM path {path}")
            else:
                paths.add(path)
            states = fsm.get("expected_states")
            if not isinstance(states, list) or not states:
                issues.append(f"{label} expected_states must be a non-empty array")
            else:
                ids = set()
                for state_index, state in enumerate(states):
                    state_label = f"{label}.expected_states[{state_index}]"
                    if not isinstance(state, dict):
                        issues.append(f"{state_label} must be an object")
                        continue
                    identifier = state.get("id")
                    if not isinstance(identifier, str) or not identifier:
                        issues.append(f"{state_label} id is invalid")
                    elif identifier in ids:
                        issues.append(f"duplicate expected state id {identifier}")
                    else:
                        ids.add(identifier)
                    for key in ("label", "graph"):
                        if state.get(key) is not None and not isinstance(state.get(key), str):
                            issues.append(f"{state_label} {key} is invalid")
                    try:
                        expected_position(state.get("position"))
                    except (TypeError, ValueError):
                        issues.append(f"{state_label} position is invalid")
            transitions = fsm.get("expected_transitions")
            if not isinstance(transitions, list):
                issues.append(f"{label} expected_transitions must be an array")
            else:
                ids = set()
                for transition_index, transition in enumerate(transitions):
                    transition_label = f"{label}.expected_transitions[{transition_index}]"
                    if not isinstance(transition, dict) or set(transition) != {
                        "id",
                        "source",
                        "target",
                    }:
                        issues.append(f"{transition_label} is invalid")
                        continue
                    identifier = transition["id"]
                    if not isinstance(identifier, str) or not identifier:
                        issues.append(f"{transition_label} id is invalid")
                    elif identifier in ids:
                        issues.append(f"duplicate expected transition id {identifier}")
                    else:
                        ids.add(identifier)
                    for key in ("source", "target"):
                        if not isinstance(transition[key], str) or not transition[key]:
                            issues.append(f"{transition_label} {key} is invalid")
            expected_io = fsm.get("expected_io")
            if not isinstance(expected_io, dict) or set(expected_io) != {
                "input_data",
                "input_times",
                "output_time",
                "output_data",
            }:
                issues.append(f"{label} expected_io is invalid")
            elif (
                not isinstance(expected_io["input_data"], dict)
                or not isinstance(expected_io["input_times"], list)
                or not isinstance(expected_io["output_time"], bool)
                or not isinstance(expected_io["output_data"], dict)
            ):
                issues.append(f"{label} expected_io values are invalid")
            exceptions = fsm.get("root_output_exceptions", [])
            if not isinstance(exceptions, list):
                issues.append(f"{label} root_output_exceptions must be an array")
            else:
                names = set()
                for exception_index, exception in enumerate(exceptions):
                    exception_label = f"{label}.root_output_exceptions[{exception_index}]"
                    if not isinstance(exception, dict):
                        issues.append(f"{exception_label} must be an object")
                        continue
                    name = exception.get("name")
                    if not isinstance(name, str) or not name:
                        issues.append(f"{exception_label} name is invalid")
                    elif name in names:
                        issues.append(f"duplicate root output exception {name}")
                    else:
                        names.add(name)
                    for key in ("reason", "evidence"):
                        if not isinstance(exception.get(key), str) or not exception[key]:
                            issues.append(f"{exception_label} {key} is invalid")
    if isinstance(graphs, list) and isinstance(fsms, list) and not graphs and not fsms:
        issues.append("graphs or fsms must be declared")
    recursive_source_inventory = manifest.get("recursive_source_inventory")
    issues.extend(recursive_source_inventory_issues(recursive_source_inventory))
    if recursive_source_inventory is None and manifest.get("recursive_inventory") is not None:
        issues.append("recursive_source_inventory is required with recursive_inventory")
    issues.extend(
        recursive_inventory_issues(
            manifest.get("recursive_inventory"),
            recursive_source_inventory,
        )
    )
    commands = manifest.get("verification_commands")
    if not isinstance(commands, list):
        issues.append("verification_commands must be an array")
    else:
        labels = set()
        scopes = set()
        for index, command in enumerate(commands):
            for issue in command_shape_issues(command):
                issues.append(f"verification_commands[{index}] {issue}")
            if isinstance(command, dict):
                label = command.get("label")
                if isinstance(label, str) and label:
                    if label in labels:
                        issues.append(f"duplicate verification command label {label}")
                    labels.add(label)
                scope = command.get("scope")
                if scope in READ_ONLY_SCOPES:
                    scopes.add(scope)
        for scope in sorted(READ_ONLY_SCOPES - scopes):
            issues.append(f"verification command scope is missing: {scope}")
    runtime_gate = manifest.get("runtime_gate")
    if not isinstance(runtime_gate, dict) or set(runtime_gate) != {
        "contract_gate",
        "manual_case",
    }:
        issues.append("runtime_gate is invalid")
    elif any(not isinstance(runtime_gate[key], str) or not runtime_gate[key] for key in runtime_gate):
        issues.append("runtime_gate values are invalid")
    return issues


def inventory_entries(value):
    if not isinstance(value, dict) or value.get("format") != RECURSIVE_INVENTORY_FORMAT:
        raise ValueError("recursive source inventory format is invalid")
    root_source_graph = value.get("root_source_graph")
    entries = value.get("entries")
    if not isinstance(root_source_graph, str) or not root_source_graph:
        raise ValueError("recursive source inventory root graph is invalid")
    if not isinstance(entries, list) or not entries:
        raise ValueError("recursive source inventory entries are invalid")
    result = {}
    allowed = {
        "source_graph",
        "kind",
        "class_path",
        "name",
        "index",
        "parent_source_graph",
    }
    for index, entry in enumerate(entries):
        if not isinstance(entry, dict) or set(entry) != allowed:
            raise ValueError(f"recursive source inventory entry {index} is invalid")
        source_graph = entry.get("source_graph")
        if not isinstance(source_graph, str) or not source_graph:
            raise ValueError(f"recursive source inventory entry {index} graph is invalid")
        if source_graph in result:
            raise ValueError(f"recursive source inventory graph is duplicated: {source_graph}")
        if entry.get("kind") not in {"graph", "fsm", "transition"}:
            raise ValueError(f"recursive source inventory entry {index} kind is invalid")
        if not isinstance(entry.get("class_path"), str) or not entry["class_path"]:
            raise ValueError(f"recursive source inventory entry {index} class is invalid")
        if not isinstance(entry.get("name"), str) or not entry["name"]:
            raise ValueError(f"recursive source inventory entry {index} name is invalid")
        if not isinstance(entry.get("index"), int) or entry["index"] < 0:
            raise ValueError(f"recursive source inventory entry {index} index is invalid")
        parent = entry.get("parent_source_graph")
        if parent is not None and (not isinstance(parent, str) or not parent):
            raise ValueError(f"recursive source inventory entry {index} parent is invalid")
        result[source_graph] = entry
    if root_source_graph not in result:
        raise ValueError("recursive source inventory root is missing")
    return result


def validate_recursive_source_inventory(root, value):
    if value is None:
        return {}, None, {}, []
    report = {
        "path": value["path"],
        "expected_sha256": value["sha256"],
        "hash_kind": value.get("hash_kind", "bytes"),
    }
    path = repo_path(root, value["path"])
    report["exists"] = path.is_file()
    if not path.is_file():
        return {}, None, report, [f"recursive source inventory is missing: {value['path']}"]
    digest = source_digest(path, report["hash_kind"])
    report["sha256"] = digest
    report["matches"] = digest == value["sha256"]
    failures = []
    if not report["matches"]:
        failures.append(f"recursive source inventory hash differs: {value['path']}")
    try:
        inventory = read_json(path)
        entries = inventory_entries(inventory)
        source_asset = inventory.get("source_asset")
        if not isinstance(source_asset, str) or not source_asset:
            raise ValueError("recursive source inventory asset is invalid")
    except (ValueError, json.JSONDecodeError) as error:
        failures.append(str(error))
        entries = {}
        source_asset = None
    report["entries"] = len(entries)
    report["source_asset"] = source_asset
    return entries, source_asset, report, failures


def map_values(path):
    try:
        return read_json(path), None
    except (OSError, json.JSONDecodeError) as error:
        return {}, str(error)


def command_references_map(root, command, map_path):
    if map_path in command["argv"]:
        return True
    argv = command["argv"]
    for index, value in enumerate(argv[:-1]):
        if value != "--index":
            continue
        index_path = repo_path(root, argv[index + 1])
        if not index_path.is_file():
            continue
        index_value, error = map_values(index_path)
        if error is not None or not isinstance(index_value.get("entries"), list):
            continue
        if any(
            isinstance(entry, dict) and entry.get("map") == map_path
            for entry in index_value["entries"]
        ):
            return True
    return False


def validate_recursive_inventory(
    root,
    entries,
    graphs,
    fsms,
    commands,
    expected_source_entries,
    expected_source_asset,
):
    if entries is None:
        return [], []
    graph_paths = {entry["path"] for entry in graphs}
    fsm_paths = {entry["path"] for entry in fsms}
    command_by_label = {entry["label"]: entry for entry in commands}
    failures = []
    report = []
    declared_sources = set()
    for entry in entries:
        row = dict(entry)
        declared_sources.add(entry["source_graph"])
        source_contract = repo_path(root, entry["source_contract"])
        source_map = repo_path(root, entry["source_map"])
        row["source_contract_exists"] = source_contract.is_file()
        row["source_map_exists"] = source_map.is_file()
        expected = expected_source_entries.get(entry["source_graph"])
        row["source_discovered"] = expected is not None
        row["source_kind_matches"] = expected is not None and expected["kind"] == entry["kind"]
        if entry["kind"] == "graph":
            row["target_declared"] = entry["target_path"] in graph_paths
        elif entry["kind"] == "fsm":
            row["target_declared"] = entry["target_path"] in fsm_paths
        else:
            target_path = repo_path(root, entry["target_path"])
            row["target_declared"] = (
                target_path.is_file()
                and entry["target_path"].replace("\\", "/").startswith("tools/")
            )
        command = command_by_label.get(entry["gate_label"])
        row["gate_declared"] = command is not None
        row["gate_references_map"] = (
            command is not None and command_references_map(root, command, entry["source_map"])
        )
        row["gate_references_native_source"] = (
            command is not None and "--source" in command["argv"]
        )
        row["contract_source_graph_matches"] = False
        row["contract_source_asset_matches"] = False
        if source_contract.is_file():
            contract, contract_error = map_values(source_contract)
            if contract_error is not None:
                row["contract_error"] = contract_error
            else:
                row["contract_source_graph_matches"] = (
                    contract.get("source_graph_suffix") == entry["source_graph"]
                )
                row["contract_source_asset_matches"] = (
                    expected_source_asset is not None
                    and contract.get("source_asset") == expected_source_asset
                )
        row["map_source_contract_matches"] = False
        row["map_target_matches"] = False
        row["map_format_matches"] = False
        if source_map.is_file():
            mapping, mapping_error = map_values(source_map)
            if mapping_error is not None:
                row["map_error"] = mapping_error
            else:
                row["map_source_contract_matches"] = (
                    mapping.get("source_contract") == entry["source_contract"]
                )
                target_key = "target_fsm" if entry["kind"] == "fsm" else "target_graph"
                map_target = mapping.get(target_key)
                row["map_target_matches"] = (
                    isinstance(map_target, str)
                    and map_target.replace("\\", "/")
                    == entry["target_path"].replace("\\", "/")
                )
                row["map_format_matches"] = (
                    mapping.get("format") == "als_ue_visible_animgraph_map_v2"
                    if entry["kind"] in {"graph", "transition"}
                    else mapping.get("format") == FSM_MAP_FORMAT
                )
        for key in (
            "source_contract_exists",
            "source_map_exists",
            "source_discovered",
            "source_kind_matches",
            "target_declared",
            "gate_declared",
            "gate_references_map",
            "gate_references_native_source",
            "contract_source_graph_matches",
            "contract_source_asset_matches",
            "map_source_contract_matches",
            "map_target_matches",
            "map_format_matches",
        ):
            if not row[key]:
                failures.append(f"recursive inventory {entry['target_path']} {key} failed")
        report.append(row)
    if expected_source_entries:
        missing = sorted(set(expected_source_entries) - declared_sources)
        unexpected = sorted(declared_sources - set(expected_source_entries))
        if missing:
            failures.append("recursive inventory missing source graphs: " + ", ".join(missing))
        if unexpected:
            failures.append("recursive inventory has unknown source graphs: " + ", ".join(unexpected))
    return report, failures


def validate_source_evidence(root, values):
    failures = []
    report = []
    for item in values:
        path = repo_path(root, item["path"])
        result = {
            "path": item["path"],
            "exists": path.is_file(),
            "expected_sha256": item["sha256"],
            "hash_kind": item.get("hash_kind", "bytes"),
        }
        if not path.is_file():
            failures.append(f"source evidence is missing: {item['path']}")
            report.append(result)
            continue
        digest = source_digest(path, result["hash_kind"])
        result["sha256"] = digest
        result["matches"] = digest == item["sha256"]
        if not result["matches"]:
            failures.append(f"source evidence hash differs: {item['path']}")
        report.append(result)
    return report, failures


def position_report(text, field_name, expected_items, item_name):
    parsed = parse_positions(text, field_name)
    failures = []
    by_id = {}
    occupied = {}
    for entry in parsed["entries"]:
        identifier = entry["id"]
        if identifier in by_id:
            failures.append(f"duplicate {item_name} position {identifier}")
            continue
        position = entry["position"]
        if position is None or not all(math.isfinite(value) for value in position):
            failures.append(f"invalid {item_name} position {identifier}")
            continue
        by_id[identifier] = [float(position[0]), float(position[1])]
        occupied.setdefault(tuple(by_id[identifier]), []).append(identifier)
    if parsed["malformed"]:
        failures.append(f"malformed {item_name} position metadata")
    for identifiers in occupied.values():
        if len(identifiers) > 1:
            failures.append(f"overlapping {item_name} positions {','.join(sorted(identifiers))}")
    expected_ids = [item["id"] for item in expected_items]
    actual_ids = set(by_id)
    missing = [identifier for identifier in expected_ids if identifier not in actual_ids]
    extra = sorted(actual_ids - set(expected_ids))
    if missing:
        failures.append(f"missing {item_name} positions {','.join(missing)}")
    if extra:
        failures.append(f"unexpected {item_name} positions {','.join(extra)}")
    for item in expected_items:
        identifier = item["id"]
        if identifier not in by_id:
            continue
        expected = expected_position(item["position"])
        actual = by_id[identifier]
        if not all(math.isclose(left, right, abs_tol=0.001) for left, right in zip(actual, expected)):
            failures.append(f"{item_name} position differs {identifier}")
    return {"positions": by_id}, failures


def validate_root_output_exceptions(actual_io, values):
    failures = []
    exceptions = {item["name"]: item for item in values}
    data_outputs = actual_io["output_data"]
    non_pose = {
        name
        for name, data_type in data_outputs.items()
        if compact(data_type) != "Pose"
    }
    missing = sorted(non_pose - set(exceptions))
    extra = sorted(set(exceptions) - non_pose)
    if missing:
        failures.append(f"unregistered root output exceptions {','.join(missing)}")
    if extra:
        failures.append(f"stale root output exceptions {','.join(extra)}")
    return {
        "non_pose_outputs": sorted(non_pose),
        "exceptions": sorted(exceptions),
    }, failures


def expected_io_payload(value):
    return {
        "input_data": value["input_data"],
        "input_times": [compact(item) for item in value["input_times"]],
        "output_time": value["output_time"],
        "output_data": value["output_data"],
    }


def validate_runtime_context(root, graph_path, text, value):
    manifest_path = repo_path(root, value)
    if not manifest_path.is_file():
        return {}, [f"runtime context manifest is missing: {value}"]
    manifest = read_json(manifest_path)
    try:
        asset_root = repo_path(root, manifest["asset_root"])
        context_graph = repo_path(asset_root, manifest["graph"])
    except (KeyError, ValueError) as error:
        return {}, [f"runtime context manifest is invalid: {error}"]
    if context_graph != graph_path:
        return {}, [f"runtime context graph differs: {value}"]
    report = runtime_context_audit(text, manifest)
    return report, list(report.get("failures", []))


def validate_graph(root, value):
    path = repo_path(root, value["path"])
    report = {"path": value["path"], "exists": path.is_file()}
    failures = []
    if not path.is_file():
        return report, [f"graph is missing: {value['path']}"]
    text = read_text(path)
    actual_nodes, ordered_nodes = parse_nodes(text)
    expected_nodes = value["expected_nodes"]
    expected_ids = [node["id"] for node in expected_nodes]
    actual_ids = [node["id"] for node in ordered_nodes]
    report["nodes"] = len(ordered_nodes)
    if actual_ids != expected_ids:
        failures.append("node order or membership differs")
    for node in expected_nodes:
        actual = actual_nodes.get(node["id"])
        if actual is None:
            continue
        if actual["name"] != node["name"]:
            failures.append(f"node name differs {node['id']}")
    positions, position_failures = position_report(
        text, "node_positions", expected_nodes, "node"
    )
    report["positions"] = positions
    failures.extend(position_failures)
    edges, _ = parse_edges(text)
    actual_edges = [
        {"target": compact(edge["target_text"]), "source": compact(edge["source_text"])}
        for edge in edges
    ]
    expected_edges = [expected_edge(edge) for edge in value["expected_edges"]]
    report["edges"] = len(actual_edges)
    if actual_edges != expected_edges:
        failures.append("edge order or membership differs")
    actual_io = io_payload(path, text)
    report["io"] = actual_io
    expected_io = expected_io_payload(value["expected_io"])
    if actual_io != expected_io:
        failures.append("root IO differs")
    exceptions, exception_failures = validate_root_output_exceptions(
        actual_io, value.get("root_output_exceptions", [])
    )
    report["root_output_exceptions"] = exceptions
    failures.extend(exception_failures)
    runtime_manifest = value.get("runtime_context_manifest")
    if runtime_manifest is not None:
        context_report, context_failures = validate_runtime_context(
            root, path, text, runtime_manifest
        )
        report["runtime_context"] = context_report
        failures.extend(f"runtime context {failure}" for failure in context_failures)
    return report, failures


def validate_fsm(root, value):
    path = repo_path(root, value["path"])
    report = {"path": value["path"], "exists": path.is_file()}
    if not path.is_file():
        return report, [f"FSM is missing: {value['path']}"]
    text = read_text(path)
    parsed = capture_fsm(text)
    failures = []
    expected_states = value["expected_states"]
    actual_states = [
        {
            "id": state["id"],
            "label": state["label"],
            "graph": state["graph"],
        }
        for state in parsed["states"]
    ]
    required_states = [
        {
            "id": state["id"],
            "label": state.get("label"),
            "graph": state.get("graph"),
        }
        for state in expected_states
    ]
    report["states"] = len(actual_states)
    if actual_states != required_states:
        failures.append("state order or membership differs")
    positions, position_failures = position_report(
        text, "states", expected_states, "state"
    )
    report["positions"] = positions
    failures.extend(position_failures)
    actual_transitions = [
        {
            "id": transition["id"],
            "source": transition["source"],
            "target": transition["target"],
        }
        for transition in parsed["transitions"]
    ]
    report["transitions"] = len(actual_transitions)
    if actual_transitions != value["expected_transitions"]:
        failures.append("transition order or membership differs")
    actual_io = io_payload(path, text)
    report["io"] = actual_io
    if actual_io != expected_io_payload(value["expected_io"]):
        failures.append("FSM IO differs")
    exceptions, exception_failures = validate_root_output_exceptions(
        actual_io, value.get("root_output_exceptions", [])
    )
    report["root_output_exceptions"] = exceptions
    failures.extend(exception_failures)
    return report, failures


def run_command(root, value):
    argv = value["argv"]
    script_path = repo_path(root, argv[1])
    result = {
        "label": value["label"],
        "scope": value["scope"],
        "argv": argv,
        "script_exists": script_path.is_file(),
    }
    if not script_path.is_file():
        return result, [f"verification script is missing: {argv[1]}"]
    timeout = value.get("timeout_seconds", 120)
    result["timeout_seconds"] = timeout
    process = subprocess.Popen(
        [sys.executable, *argv[1:]],
        cwd=root,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    try:
        stdout, stderr = process.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        if sys.platform == "win32":
            subprocess.run(
                ["taskkill", "/PID", str(process.pid), "/T", "/F"],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                check=False,
            )
        else:
            process.kill()
        stdout, stderr = process.communicate()
        if isinstance(stdout, bytes):
            stdout = stdout.decode("utf-8", errors="replace")
        if isinstance(stderr, bytes):
            stderr = stderr.decode("utf-8", errors="replace")
        result["exit_code"] = None
        result["timed_out"] = True
        result["process_tree_terminated"] = True
        result["stdout_tail"] = stdout[-2000:]
        result["stderr_tail"] = stderr[-2000:]
        return result, [f"verification command timed out: {value['label']}"]
    result["exit_code"] = process.returncode
    result["stdout_tail"] = stdout[-2000:]
    result["stderr_tail"] = stderr[-2000:]
    if process.returncode:
        return result, [f"verification command failed: {value['label']}"]
    return result, []


def run(root, manifest, execute_commands=True):
    failures = manifest_shape_issues(manifest)
    report = {
        "id": manifest.get("id"),
        "failures": failures,
        "graphs": [],
        "fsms": [],
        "commands": [],
        "recursive_source_inventory": {},
        "recursive_inventory": [],
    }
    if failures:
        return report
    evidence, evidence_failures = validate_source_evidence(root, manifest["source_evidence"])
    report["source_evidence"] = evidence
    failures.extend(evidence_failures)
    for graph in manifest.get("graphs", []):
        graph_report, graph_failures = validate_graph(root, graph)
        report["graphs"].append(graph_report)
        failures.extend(f"{graph['path']}: {failure}" for failure in graph_failures)
    for fsm in manifest.get("fsms", []):
        fsm_report, fsm_failures = validate_fsm(root, fsm)
        report["fsms"].append(fsm_report)
        failures.extend(f"{fsm['path']}: {failure}" for failure in fsm_failures)
    source_entries, source_asset, source_inventory, source_inventory_failures = (
        validate_recursive_source_inventory(
            root,
            manifest.get("recursive_source_inventory"),
        )
    )
    report["recursive_source_inventory"] = source_inventory
    failures.extend(source_inventory_failures)
    inventory, inventory_failures = validate_recursive_inventory(
        root,
        manifest.get("recursive_inventory"),
        manifest.get("graphs", []),
        manifest.get("fsms", []),
        manifest.get("verification_commands", []),
        source_entries,
        source_asset,
    )
    report["recursive_inventory"] = inventory
    failures.extend(inventory_failures)
    if execute_commands:
        for command in manifest["verification_commands"]:
            command_report, command_failures = run_command(root, command)
            report["commands"].append(command_report)
            failures.extend(command_failures)
    report["pass"] = not failures
    return report


def fixture_graph(output_data='"pose": Pose', position="(0.0, 0.0)"):
    return (
        "(\n"
        'nodes: [(id: "node", name: "Source", inner: { "als_ext::LayeredGraphNode": (graph: "graphs/source.animgraph.ron") }, should_debug: false)],\n'
        'edges_inverted: { OutputData("pose"): NodeData("node", "pose") },\n'
        "io_spec: (input_times: [], input_data: {}, output_time: false, "
        f"output_data: {{ {output_data} }}, input_order: {{}}, output_order: {{ Data(\"pose\"): 0 }}),\n"
        "default_data: {},\n"
        f'editor_metadata: (node_positions: {{ "node": {position} }}, input_position: (-64.0, 0.0), output_position: (64.0, 0.0), input_param_order: {{}}, input_time_order: {{}}, output_data_order: {{}}, output_pose_order: {{}}),\n'
        ")\n"
    )


def fixture_manifest(digest):
    return {
        "format": FORMAT,
        "id": "fixture",
        "source_evidence": [{"path": "source.json", "sha256": digest}],
        "graphs": [
            {
                "path": "graph.animgraph.ron",
                "expected_nodes": [
                    {"id": "node", "name": "Source", "position": [0.0, 0.0]}
                ],
                "expected_edges": [
                    {
                        "target": 'OutputData("pose")',
                        "source": 'NodeData("node", "pose")',
                    }
                ],
                "expected_io": {
                    "input_data": {},
                    "input_times": [],
                    "output_time": False,
                    "output_data": {"pose": "Pose"},
                },
                "root_output_exceptions": [],
            }
        ],
        "verification_commands": [
            {
                "label": "source",
                "scope": "source",
                "argv": ["python", "tools/source.py", "--check"],
            },
            {
                "label": "layout",
                "scope": "layout",
                "argv": ["python", "tools/layout.py", "--check"],
            },
            {
                "label": "editor",
                "scope": "editor",
                "argv": ["python", "tools/editor.py", "--check"],
            },
        ],
        "runtime_gate": {
            "contract_gate": "fixture_contract_gate",
            "manual_case": "fixture",
        },
    }


def fixture_fsm():
    return (
        "(\n"
        'states: [(id: "entry", label: "Entry", graph: "graphs/entry.animgraph.ron"), (id: "run", label: "Run", graph: "graphs/run.animgraph.ron")],\n'
        'transitions: [(id: "entry_to_run", source: "entry", target: "run")],\n'
        "node_spec: (input_times: [], input_data: { \"movement_pose\": Pose }, output_time: false, output_data: { \"pose\": Pose }, input_order: { Data(\"movement_pose\"): 0 }, output_order: { Data(\"pose\"): 0 }),\n"
        "default_data: {},\n"
        'editor_metadata: (states: { "entry": (0.0, 0.0), "run": (128.0, 0.0) }, input_position: (-64.0, 0.0), output_position: (256.0, 0.0), input_param_order: {}, input_time_order: {}, output_data_order: {}, output_pose_order: {}),\n'
        ")\n"
    )


def run_self_test():
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        (root / "source.json").write_text("{}\n", encoding="utf-8")
        graph_path = root / "graph.animgraph.ron"
        graph_path.write_text(fixture_graph(), encoding="utf-8")
        manifest = fixture_manifest(sha256_file(root / "source.json"))
        valid = run(root, manifest, execute_commands=False)
        assert valid["pass"]
        report_path = root / "report.json"
        write_json(report_path, valid)
        assert read_json(report_path) == valid
        (root / "tools").mkdir(exist_ok=True)
        (root / "tools" / "source.py").write_text(
            "import time\ntime.sleep(60)\n", encoding="utf-8"
        )
        timeout_command = {
            "label": "source",
            "scope": "source",
            "argv": ["python", "tools/source.py", "--check"],
            "timeout_seconds": 1,
        }
        timeout_report, timeout_failures = run_command(root, timeout_command)
        assert timeout_report["timed_out"]
        assert timeout_report["process_tree_terminated"]
        assert timeout_failures == ["verification command timed out: source"]
        graph_path.write_text(
            fixture_graph(output_data='"pose": Pose, "curve": F32'),
            encoding="utf-8",
        )
        output_failure = run(root, manifest, execute_commands=False)
        assert not output_failure["pass"]
        assert any("root IO differs" in failure for failure in output_failure["failures"])
        graph_path.write_text(fixture_graph(position="(1.0, 0.0)"), encoding="utf-8")
        position_failure = run(root, manifest, execute_commands=False)
        assert not position_failure["pass"]
        assert any("node position differs" in failure for failure in position_failure["failures"])
        graph_path.write_text(fixture_graph(), encoding="utf-8")
        fsm_path = root / "state.fsm.ron"
        fsm_path.write_text(fixture_fsm(), encoding="utf-8")
        fsm_manifest = fixture_manifest(sha256_file(root / "source.json"))
        fsm_manifest["fsms"] = [
            {
                "path": "state.fsm.ron",
                "expected_states": [
                    {
                        "id": "entry",
                        "label": "Entry",
                        "graph": "graphs/entry.animgraph.ron",
                        "position": [0.0, 0.0],
                    },
                    {
                        "id": "run",
                        "label": "Run",
                        "graph": "graphs/run.animgraph.ron",
                        "position": [128.0, 0.0],
                    },
                ],
                "expected_transitions": [
                    {
                        "id": "entry_to_run",
                        "source": "entry",
                        "target": "run",
                    }
                ],
                "expected_io": {
                    "input_data": {"movement_pose": "Pose"},
                    "input_times": [],
                    "output_time": False,
                    "output_data": {"pose": "Pose"},
                },
                "root_output_exceptions": [],
            }
        ]
        fsm_valid = run(root, fsm_manifest, execute_commands=False)
        assert fsm_valid["pass"]
        invalid_command = fixture_manifest(sha256_file(root / "source.json"))
        invalid_command["verification_commands"][0]["argv"] = [
            "python",
            "tools/source.py",
            "--fix",
        ]
        assert manifest_shape_issues(invalid_command)
        inventory_path = root / "inventory.json"
        inventory_path.write_text(
            json.dumps(
                {
                    "format": RECURSIVE_INVENTORY_FORMAT,
                    "source_asset": "/Asset",
                    "root_source_graph": "AnimGraph",
                    "entries": [
                        {
                            "source_graph": "AnimGraph",
                            "kind": "graph",
                            "class_path": "/Script/AnimGraph.AnimationGraph",
                            "name": "AnimGraph",
                            "index": 0,
                            "parent_source_graph": None,
                        }
                    ],
                }
            )
            + "\n",
            encoding="utf-8",
        )
        contract_path = root / "contract.json"
        contract_path.write_text(
            json.dumps(
                {
                    "source_asset": "/Asset",
                    "source_graph_suffix": "AnimGraph",
                }
            )
            + "\n",
            encoding="utf-8",
        )
        map_path = root / "map.json"
        map_path.write_text(
            json.dumps(
                {
                    "format": "als_ue_visible_animgraph_map_v2",
                    "source_contract": "contract.json",
                    "target_graph": "graph.animgraph.ron",
                }
            )
            + "\n",
            encoding="utf-8",
        )
        recursive_manifest = fixture_manifest(sha256_file(root / "source.json"))
        recursive_manifest["recursive_source_inventory"] = {
            "path": "inventory.json",
            "sha256": source_digest(inventory_path, "canonical_json"),
            "hash_kind": "canonical_json",
        }
        recursive_manifest["recursive_inventory"] = [
            {
                "kind": "graph",
                "source_graph": "AnimGraph",
                "source_contract": "contract.json",
                "source_map": "map.json",
                "target_path": "graph.animgraph.ron",
                "gate_label": "source",
            }
        ]
        audit_index_path = root / "audit-index.json"
        audit_index_path.write_text(
            json.dumps({"entries": [{"map": "map.json"}]}) + "\n",
            encoding="utf-8",
        )
        recursive_manifest["verification_commands"][0]["argv"].extend(
            ["--index", "audit-index.json", "--source", "raw.json"]
        )
        recursive_valid = run(root, recursive_manifest, execute_commands=False)
        assert recursive_valid["pass"]
        transition_contract_path = root / "transition-contract.json"
        transition_contract_path.write_text(
            json.dumps(
                {
                    "source_asset": "/Asset",
                    "source_graph_suffix": "AnimGraph.Transition",
                }
            )
            + "\n",
            encoding="utf-8",
        )
        transition_map_path = root / "transition-map.json"
        transition_map_path.write_text(
            json.dumps(
                {
                    "format": "als_ue_visible_animgraph_map_v2",
                    "source_contract": "transition-contract.json",
                    "target_graph": "tools/transition.animgraph.ron",
                }
            )
            + "\n",
            encoding="utf-8",
        )
        (root / "tools").mkdir(exist_ok=True)
        (root / "tools" / "transition.animgraph.ron").write_text("()\n", encoding="utf-8")
        recursive_source = read_json(inventory_path)
        recursive_source["entries"].append(
            {
                "source_graph": "AnimGraph.Transition",
                "kind": "transition",
                "class_path": "/Script/AnimGraph.AnimationTransitionGraph",
                "name": "Transition",
                "index": 1,
                "parent_source_graph": "AnimGraph",
            }
        )
        inventory_path.write_text(json.dumps(recursive_source) + "\n", encoding="utf-8")
        recursive_manifest["recursive_source_inventory"]["sha256"] = source_digest(
            inventory_path,
            "canonical_json",
        )
        recursive_manifest["recursive_inventory"].append(
            {
                "kind": "transition",
                "source_graph": "AnimGraph.Transition",
                "source_contract": "transition-contract.json",
                "source_map": "transition-map.json",
                "target_path": "tools/transition.animgraph.ron",
                "gate_label": "source",
            }
        )
        audit_index_path.write_text(
            json.dumps(
                {"entries": [{"map": "map.json"}, {"map": "transition-map.json"}]}
            )
            + "\n",
            encoding="utf-8",
        )
        recursive_transition_valid = run(root, recursive_manifest, execute_commands=False)
        assert recursive_transition_valid["pass"]
        recursive_manifest["verification_commands"][0]["argv"] = [
            "python",
            "tools/source.py",
            "--check",
            "--index",
            "audit-index.json",
        ]
        recursive_missing_source = run(root, recursive_manifest, execute_commands=False)
        assert not recursive_missing_source["pass"]
        assert any(
            "gate_references_native_source failed" in failure
            for failure in recursive_missing_source["failures"]
        )
        recursive_source = read_json(inventory_path)
        recursive_source["entries"].append(
            {
                "source_graph": "AnimGraph.State",
                "kind": "graph",
                "class_path": "/Script/AnimGraph.AnimationStateGraph",
                "name": "State",
                "index": 1,
                "parent_source_graph": "AnimGraph",
            }
        )
        inventory_path.write_text(
            json.dumps(recursive_source) + "\n",
            encoding="utf-8",
        )
        recursive_manifest["recursive_source_inventory"]["sha256"] = source_digest(
            inventory_path,
            "canonical_json",
        )
        recursive_missing = run(root, recursive_manifest, execute_commands=False)
        assert not recursive_missing["pass"]
        assert any(
            "recursive inventory missing source graphs" in failure
            for failure in recursive_missing["failures"]
        )
    return {
        "exact_graph_contract": "PASS",
        "unregistered_output_rejected": "PASS",
        "position_mismatch_rejected": "PASS",
        "exact_fsm_contract": "PASS",
        "mutating_command_rejected": "PASS",
        "recursive_inventory_closure_rejected": "PASS",
        "recursive_inventory_live_source_required": "PASS",
        "indexed_recursive_gate_and_tool_transition_supported": "PASS",
        "machine_readable_report_round_trip": "PASS",
        "command_timeout_reported_without_crash": "PASS",
    }


def summary(report):
    return {
        "id": report.get("id"),
        "pass": report.get("pass", False),
        "failures": report.get("failures", []),
        "graphs": len(report.get("graphs", [])),
        "graph_nodes": sum(item.get("nodes", 0) for item in report.get("graphs", [])),
        "graph_edges": sum(item.get("edges", 0) for item in report.get("graphs", [])),
        "fsms": len(report.get("fsms", [])),
        "fsm_states": sum(item.get("states", 0) for item in report.get("fsms", [])),
        "fsm_transitions": sum(
            item.get("transitions", 0) for item in report.get("fsms", [])
        ),
        "commands": [
            {
                "label": item.get("label"),
                "scope": item.get("scope"),
                "exit_code": item.get("exit_code"),
            }
            for item in report.get("commands", [])
        ],
        "source_evidence": [
            {
                "path": item.get("path"),
                "hash_kind": item.get("hash_kind"),
                "matches": item.get("matches"),
            }
            for item in report.get("source_evidence", [])
        ],
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--summary", action="store_true")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--no-execute", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
        return 0
    if args.manifest is None or not args.check:
        raise ValueError("manifest and --check are required")
    root = args.root.resolve()
    manifest_path = repo_path(root, args.manifest.as_posix())
    report = run(root, read_json(manifest_path), execute_commands=not args.no_execute)
    if args.output is not None:
        write_json(repo_path(root, args.output.as_posix()), report)
    print(json.dumps(summary(report) if args.summary else report, ensure_ascii=False, indent=2))
    return 0 if report["pass"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
