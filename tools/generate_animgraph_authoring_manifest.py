import argparse
import json
import math
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tools"))
sys.path.insert(0, str(ROOT / "tools" / "import"))

from importlib.util import module_from_spec, spec_from_file_location


VALIDATOR_SPEC = spec_from_file_location(
    "validate_animgraph_authoring",
    ROOT / "tools" / "validate_animgraph_authoring.py",
)
VALIDATOR = module_from_spec(VALIDATOR_SPEC)
VALIDATOR_SPEC.loader.exec_module(VALIDATOR)

TOPOLOGY_SPEC = spec_from_file_location(
    "capture_animgraph_ron_topology",
    ROOT / "tools" / "import" / "capture_animgraph_ron_topology.py",
)
TOPOLOGY = module_from_spec(TOPOLOGY_SPEC)
TOPOLOGY_SPEC.loader.exec_module(TOPOLOGY)

CONTEXT_SPEC = spec_from_file_location(
    "animgraph_context_contract",
    ROOT / "tools" / "animgraph_context_contract.py",
)
CONTEXT = module_from_spec(CONTEXT_SPEC)
CONTEXT_SPEC.loader.exec_module(CONTEXT)


RECIPE_FORMAT = "als_animgraph_authoring_recipe_v1"
RECURSIVE_AUDIT_INDEX_FORMAT = "als_ue_visible_recursive_audit_index_v1"


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def relative(path, root):
    return path.resolve().relative_to(root.resolve()).as_posix()


def included_paths(root, values):
    paths = []
    for value in values:
        path = VALIDATOR.repo_path(root, value)
        if path.is_file() and path.name.endswith((".animgraph.ron", ".fsm.ron")):
            paths.append(path)
        elif path.is_dir():
            paths.extend(path.rglob("*.animgraph.ron"))
            paths.extend(path.rglob("*.fsm.ron"))
        else:
            raise ValueError(f"authoring include is invalid: {value}")
    return sorted(set(paths))


def positions(text, field_name):
    parsed = TOPOLOGY.parse_positions(text, field_name)
    if parsed["malformed"]:
        raise ValueError(f"malformed {field_name} metadata")
    values = {}
    occupied = set()
    for entry in parsed["entries"]:
        identifier = entry["id"]
        position = entry["position"]
        if identifier in values or position is None:
            raise ValueError(f"duplicate or missing {field_name} position")
        value = [float(position[0]), float(position[1])]
        if not all(math.isfinite(item) for item in value):
            raise ValueError(f"non-finite {field_name} position")
        if tuple(value) in occupied:
            raise ValueError(f"overlapping {field_name} position")
        values[identifier] = value
        occupied.add(tuple(value))
    return values


def output_exceptions(io, evidence):
    return [
        {
            "name": name,
            "reason": "Declared graph boundary data output.",
            "evidence": evidence,
        }
        for name, data_type in io["output_data"].items()
        if CONTEXT.compact(data_type) != "Pose"
    ]


def graph_entry(root, path, recipe):
    text = path.read_text(encoding="utf-8")
    nodes, ordered = CONTEXT.parse_nodes(text)
    node_positions = positions(text, "node_positions")
    if set(nodes) != set(node_positions):
        raise ValueError(f"node position coverage differs: {relative(path, root)}")
    io = VALIDATOR.io_payload(path, text)
    edges, _ = CONTEXT.parse_edges(text)
    entry = {
        "path": relative(path, root),
        "expected_nodes": [
            {
                "id": node["id"],
                "name": node["name"],
                "position": node_positions[node["id"]],
            }
            for node in ordered
        ],
        "expected_edges": [
            {
                "target": edge["target_text"],
                "source": edge["source_text"],
            }
            for edge in edges
        ],
        "expected_io": io,
        "root_output_exceptions": output_exceptions(
            io, recipe["root_output_evidence"]
        ),
    }
    context_manifest = recipe.get("runtime_context_manifests", {}).get(entry["path"])
    if context_manifest is not None:
        entry["runtime_context_manifest"] = context_manifest
    return entry


def fsm_entry(root, path, recipe):
    text = path.read_text(encoding="utf-8")
    payload = TOPOLOGY.capture_fsm(text)
    state_positions = positions(text, "states")
    state_ids = [state["id"] for state in payload["states"]]
    if set(state_ids) != set(state_positions):
        raise ValueError(f"state position coverage differs: {relative(path, root)}")
    io = VALIDATOR.io_payload(path, text)
    return {
        "path": relative(path, root),
        "expected_states": [
            {
                "id": state["id"],
                "label": state["label"],
                "graph": state["graph"],
                "position": state_positions[state["id"]],
            }
            for state in payload["states"]
        ],
        "expected_transitions": [
            {
                "id": transition["id"],
                "source": transition["source"],
                "target": transition["target"],
            }
            for transition in payload["transitions"]
        ],
        "expected_io": io,
        "root_output_exceptions": output_exceptions(
            io, recipe["root_output_evidence"]
        ),
    }


def source_evidence(root, values):
    result = []
    for value in values:
        if not isinstance(value, dict) or set(value) - {"path", "hash_kind"}:
            raise ValueError("source evidence recipe entry is invalid")
        path = VALIDATOR.repo_path(root, value.get("path"))
        if not path.is_file():
            raise ValueError(f"source evidence is missing: {value.get('path')}")
        kind = value.get("hash_kind", "bytes")
        result.append(
            {
                "path": value["path"],
                "sha256": VALIDATOR.source_digest(path, kind),
                **({"hash_kind": kind} if kind != "bytes" else {}),
            }
        )
    return result


def recursive_inventory_from_audit_index(root, recipe):
    index_path_value = recipe.get("recursive_audit_index")
    if index_path_value is None:
        return recipe.get("recursive_inventory")
    if recipe.get("recursive_inventory") is not None:
        raise ValueError("recursive audit index and recursive inventory cannot both be declared")
    if not isinstance(index_path_value, str) or not index_path_value:
        raise ValueError("recursive audit index is invalid")
    gate_label = recipe.get("recursive_audit_gate_label")
    if not isinstance(gate_label, str) or not gate_label:
        raise ValueError("recursive audit gate label is invalid")
    path = VALIDATOR.repo_path(root, index_path_value)
    if not path.is_file():
        raise ValueError("recursive audit index is missing")
    index = read_json(path)
    entries = index.get("entries")
    if index.get("format") != RECURSIVE_AUDIT_INDEX_FORMAT or not isinstance(entries, list):
        raise ValueError("recursive audit index format is invalid")
    result = []
    required = {"kind", "source_graph", "source_contract", "map", "target_path"}
    for entry in entries:
        if not isinstance(entry, dict) or set(entry) != required:
            raise ValueError("recursive audit index entry is invalid")
        result.append(
            {
                "kind": entry["kind"],
                "source_graph": entry["source_graph"],
                "source_contract": entry["source_contract"],
                "source_map": entry["map"],
                "target_path": entry["target_path"],
                "gate_label": gate_label,
            }
        )
    return result


def normalized_recipe(root, recipe):
    value = json.loads(json.dumps(recipe))
    value["recursive_inventory"] = recursive_inventory_from_audit_index(root, value)
    return value


def validate_recipe(recipe):
    if not isinstance(recipe, dict) or recipe.get("format") != RECIPE_FORMAT:
        raise ValueError("recipe format is invalid")
    if not isinstance(recipe.get("id"), str) or not recipe["id"]:
        raise ValueError("recipe id is invalid")
    if not isinstance(recipe.get("includes"), list) or not recipe["includes"]:
        raise ValueError("recipe includes are invalid")
    if not isinstance(recipe.get("source_evidence"), list) or not recipe["source_evidence"]:
        raise ValueError("recipe source_evidence is invalid")
    if not isinstance(recipe.get("root_output_evidence"), str) or not recipe["root_output_evidence"]:
        raise ValueError("recipe root_output_evidence is invalid")
    if not isinstance(recipe.get("verification_commands"), list):
        raise ValueError("recipe verification_commands are invalid")
    if not isinstance(recipe.get("runtime_gate"), dict):
        raise ValueError("recipe runtime_gate is invalid")
    recursive_source_inventory = recipe.get("recursive_source_inventory")
    inventory_issues = VALIDATOR.recursive_source_inventory_issues(
        recursive_source_inventory,
        require_digest=False,
    )
    if inventory_issues:
        raise ValueError("; ".join(inventory_issues))
    if recursive_source_inventory is None and recipe.get("recursive_inventory") is not None:
        raise ValueError("recursive_source_inventory is required with recursive_inventory")
    recursive_issues = VALIDATOR.recursive_inventory_issues(
        recipe.get("recursive_inventory"),
        recursive_source_inventory,
    )
    if recursive_issues:
        raise ValueError("; ".join(recursive_issues))
    contexts = recipe.get("runtime_context_manifests", {})
    if not isinstance(contexts, dict) or any(
        not isinstance(key, str) or not isinstance(value, str)
        for key, value in contexts.items()
    ):
        raise ValueError("recipe runtime_context_manifests are invalid")


def generate(root, recipe):
    recipe = normalized_recipe(root, recipe)
    validate_recipe(recipe)
    graphs = []
    fsms = []
    for path in included_paths(root, recipe["includes"]):
        if path.name.endswith(".animgraph.ron"):
            graphs.append(graph_entry(root, path, recipe))
        else:
            fsms.append(fsm_entry(root, path, recipe))
    manifest = {
        "format": VALIDATOR.FORMAT,
        "id": recipe["id"],
        "source_evidence": source_evidence(root, recipe["source_evidence"]),
        "graphs": graphs,
        "fsms": fsms,
        "verification_commands": recipe["verification_commands"],
        "runtime_gate": recipe["runtime_gate"],
    }
    if "recursive_source_inventory" in recipe:
        manifest["recursive_source_inventory"] = source_evidence(
            root,
            [recipe["recursive_source_inventory"]],
        )[0]
    if "recursive_inventory" in recipe:
        manifest["recursive_inventory"] = recipe["recursive_inventory"]
    issues = VALIDATOR.manifest_shape_issues(manifest)
    if issues:
        raise ValueError("; ".join(issues))
    return manifest


def run_self_test():
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        tools = root / "tools"
        tools.mkdir()
        for name in ("source.py", "layout.py", "editor.py"):
            (tools / name).write_text("", encoding="utf-8")
        (root / "source.json").write_text('{"source": true}\n', encoding="utf-8")
        (root / "inventory.json").write_text(
            json.dumps(
                {
                    "format": "als_ue_authored_graph_inventory_v1",
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
        graph = (
            "(\n"
            'nodes: [(id: "node", name: "Source", inner: { "als_ext::LayeredGraphNode": (graph: "graphs/source.animgraph.ron") }, should_debug: false)],\n'
            'edges_inverted: { OutputData("pose"): NodeData("node", "pose") },\n'
            'io_spec: (input_times: [], input_data: {}, output_time: false, output_data: { "pose": Pose }, input_order: {}, output_order: { Data("pose"): 0 }),\n'
            "default_data: {},\n"
            'editor_metadata: (node_positions: { "node": (0.0, 0.0) }, input_position: (-64.0, 0.0), output_position: (64.0, 0.0), input_param_order: {}, input_time_order: {}, output_data_order: {}, output_pose_order: {}),\n'
            ")\n"
        )
        fsm = (
            "(\n"
            'states: [(id: "entry", label: "Entry", graph: "graphs/entry.animgraph.ron")],\n'
            "transitions: [],\n"
            'node_spec: (input_times: [], input_data: {}, output_time: false, output_data: { "pose": Pose }, input_order: {}, output_order: { Data("pose"): 0 }),\n'
            "default_data: {},\n"
            'editor_metadata: (states: { "entry": (0.0, 0.0) }, input_position: (-64.0, 0.0), output_position: (64.0, 0.0), input_param_order: {}, input_time_order: {}, output_data_order: {}, output_pose_order: {}),\n'
            ")\n"
        )
        (root / "graph.animgraph.ron").write_text(graph, encoding="utf-8")
        (root / "states.fsm.ron").write_text(fsm, encoding="utf-8")
        recipe = {
            "format": RECIPE_FORMAT,
            "id": "fixture",
            "source_evidence": [
                {"path": "source.json", "hash_kind": "canonical_json"}
            ],
            "includes": ["graph.animgraph.ron", "states.fsm.ron"],
            "root_output_evidence": "source.json",
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
        manifest = generate(root, recipe)
        assert len(manifest["graphs"]) == 1
        assert len(manifest["fsms"]) == 1
        assert manifest["source_evidence"][0]["hash_kind"] == "canonical_json"
        recipe["recursive_source_inventory"] = {
            "path": "inventory.json",
            "hash_kind": "canonical_json",
        }
        recipe["recursive_inventory"] = [
            {
                "kind": "graph",
                "source_graph": "AnimGraph",
                "source_contract": "source.json",
                "source_map": "map.json",
                "target_path": "graph.animgraph.ron",
                "gate_label": "source",
            }
        ]
        recursive_manifest = generate(root, recipe)
        assert recursive_manifest["recursive_source_inventory"]["path"] == "inventory.json"
        assert len(recursive_manifest["recursive_inventory"]) == 1
        audit_index_path = root / "audit-index.json"
        write_json(
            audit_index_path,
            {
                "format": RECURSIVE_AUDIT_INDEX_FORMAT,
                "source_contract_index": "contracts/index.json",
                "entries": [
                    {
                        "kind": "graph",
                        "source_graph": "AnimGraph",
                        "source_contract": "source.json",
                        "map": "map.json",
                        "target_path": "graph.animgraph.ron",
                    }
                ],
            },
        )
        indexed_recipe = json.loads(json.dumps(recipe))
        indexed_recipe.pop("recursive_inventory")
        indexed_recipe["recursive_audit_index"] = "audit-index.json"
        indexed_recipe["recursive_audit_gate_label"] = "source"
        indexed_manifest = generate(root, indexed_recipe)
        assert len(indexed_manifest["recursive_inventory"]) == 1
    return {
        "animgraph_contract_generation": "PASS",
        "fsm_contract_generation": "PASS",
        "canonical_source_evidence": "PASS",
        "recursive_inventory_generation": "PASS",
        "recursive_audit_index_generation": "PASS",
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--recipe", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
        return 0
    if args.recipe is None or args.output is None or args.write == args.check:
        raise ValueError("recipe, output, and exactly one of --write or --check are required")
    root = args.root.resolve()
    recipe = read_json(VALIDATOR.repo_path(root, args.recipe.as_posix()))
    output = VALIDATOR.repo_path(root, args.output.as_posix())
    generated = generate(root, recipe)
    if args.write:
        write_json(output, generated)
        print(json.dumps({"output": relative(output, root), "graphs": len(generated["graphs"]), "fsms": len(generated["fsms"])}))
        return 0
    if not output.is_file():
        raise ValueError("authoring manifest is missing")
    current = read_json(output)
    differences = [
        key
        for key in sorted(set(generated) | set(current))
        if generated.get(key) != current.get(key)
    ]
    print(json.dumps({"output": relative(output, root), "differences": differences}))
    return 0 if not differences else 1


if __name__ == "__main__":
    raise SystemExit(main())
