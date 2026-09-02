import argparse
import json
import re
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]

import sys

sys.path.insert(0, str(ROOT / "tools" / "import"))

from capture_animgraph_ron_topology import capture_fsm
from extract_ue_visible_graph_contract import canonical_hash, contract
from normalize_fsm_editor_layout import field_span, split_top_level


FORMAT = "als_ue_visible_fsm_map_v1"
SOURCE_FORMAT = "als_ue_visible_graph_contract_v2"
COMMENT_POLICIES = {"require", "ignore"}
TRANSITION_STATIC_PROPERTIES = {
    "PriorityOrder",
    "CrossfadeDuration",
    "BlendMode",
    "bAutomaticRuleBasedOnSequencePlayerInState",
    "AutomaticRuleTriggerTime",
    "SyncGroupNameToRequireValidMarkersRule",
    "bAllowInertializationForSelfTransitions",
    "bSharedRules",
    "bSharedCrossfade",
}


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def verify_source_contract(root, mapping, source_path):
    contract_path = root / mapping["source_contract"]
    if not contract_path.is_file():
        raise ValueError("source contract is missing")
    existing = read_json(contract_path)
    suffix = existing.get("source_graph_suffix")
    if not isinstance(suffix, str) or not suffix:
        raise ValueError("source graph suffix is invalid")
    current = contract(read_json(source_path), suffix)
    if canonical_hash(current) != canonical_hash(existing):
        raise ValueError("source contract differs from native extraction")


def source_nodes(contract):
    return contract["source_graph"]["nodes"]


def source_states(contract):
    return [
        node
        for node in source_nodes(contract)
        if node["class_path"].endswith("AnimStateNode")
    ]


def source_transitions(contract):
    return [
        node
        for node in source_nodes(contract)
        if node["class_path"].endswith("AnimStateTransitionNode")
    ]


def source_entries(contract):
    return [
        node
        for node in source_nodes(contract)
        if node["class_path"].endswith("AnimStateEntryNode")
    ]


def source_comments(contract):
    return [
        node
        for node in source_nodes(contract)
        if node["class_path"].endswith("EdGraphNode_Comment")
    ]


def source_transition_endpoints(contract, transition):
    path = transition["object_path"]
    incoming = []
    outgoing = []
    state_paths = {node["object_path"] for node in source_states(contract)}
    for link in contract["links"]:
        source = link["source"]["node_object_path"]
        target = link["target"]["node_object_path"]
        if target == path and source in state_paths:
            incoming.append(source)
        if source == path and target in state_paths:
            outgoing.append(target)
    return sorted(set(incoming)), sorted(set(outgoing))


def source_entry_target(contract, entry):
    path = entry["object_path"]
    state_paths = {node["object_path"] for node in source_states(contract)}
    targets = [
        link["target"]["node_object_path"]
        for link in contract["links"]
        if link["source"]["node_object_path"] == path
        and link["target"]["node_object_path"] in state_paths
    ]
    return sorted(set(targets))


def source_transition_static_properties(node):
    return [
        f"{value['name']}={value['exported_text']}"
        for value in node.get("properties", [])
        if value.get("name") in TRANSITION_STATIC_PROPERTIES
        and isinstance(value.get("exported_text"), str)
        and value["exported_text"]
    ]


def source_transition_static_contract(node):
    source_node = node["object_path"].rsplit(".", 1)[-1]
    return source_node + "|" + "|".join(source_transition_static_properties(node))


def raw_fsm_entries(text, field_name):
    _, start, end = field_span(text, field_name)
    values = split_top_level(text[start + 1 : end], ",")
    result = {}
    for value in values:
        match = re.search(r'\bid\s*:\s*"([^"]+)"', value)
        if match is None:
            raise ValueError(f"target {field_name} entry id is missing")
        identifier = match.group(1)
        if identifier in result:
            raise ValueError(f"target {field_name} entry id is duplicated")
        result[identifier] = value
    return result


def property_assertion_issues(value, label):
    if not isinstance(value, list):
        return [f"{label} must be an array"]
    issues = []
    for index, assertion in enumerate(value):
        item_label = f"{label}[{index}]"
        allowed = {
            "source_contains",
            "target_contains",
            "target_exception",
            "target_runtime_path",
        }
        if not isinstance(assertion, dict) or "source_contains" not in assertion or not set(assertion).issubset(allowed):
            issues.append(f"{item_label} is invalid")
            continue
        if not isinstance(assertion.get("source_contains"), str) or not assertion["source_contains"]:
            issues.append(f"{item_label} source_contains is invalid")
        target_contains = assertion.get("target_contains")
        target_exception = assertion.get("target_exception")
        if (isinstance(target_contains, str) and bool(target_contains)) == (
            isinstance(target_exception, str) and bool(target_exception)
        ):
            issues.append(f"{item_label} requires exactly one target assertion")
        if target_exception:
            if not assertion.get("source_contains", "").endswith("=None"):
                issues.append(f"{item_label} target_exception requires a source None value")
            if "target_runtime_path" not in assertion:
                issues.append(f"{item_label} target_exception requires target_runtime_path")
        if "target_runtime_path" in assertion and (
            not isinstance(assertion["target_runtime_path"], str)
            or not assertion["target_runtime_path"]
        ):
            issues.append(f"{item_label} target_runtime_path is invalid")
    return issues


def map_shape_issues(mapping):
    issues = []
    if not isinstance(mapping, dict) or mapping.get("format") != FORMAT:
        return ["FSM map format is invalid"]
    for key in ("source_contract", "target_fsm"):
        if not isinstance(mapping.get(key), str) or not mapping[key]:
            issues.append(f"{key} is invalid")
    required_state = {"source_node", "target_state", "target_graph"}
    if "transition_default_assertions" in mapping:
        issues.extend(
            property_assertion_issues(
                mapping["transition_default_assertions"],
                "transition_default_assertions",
            )
        )
    if "transition_static_contract_runtime_path" in mapping and (
        not isinstance(mapping["transition_static_contract_runtime_path"], str)
        or not mapping["transition_static_contract_runtime_path"]
    ):
        issues.append("transition_static_contract_runtime_path is invalid")
    for key in ("state_map", "transition_map"):
        if not isinstance(mapping.get(key), list):
            issues.append(f"{key} must be an array")
    if isinstance(mapping.get("state_map"), list):
        seen_source = set()
        seen_target = set()
        for index, value in enumerate(mapping["state_map"]):
            label = f"state_map[{index}]"
            if not isinstance(value, dict) or set(value) != required_state:
                issues.append(f"{label} is invalid")
                continue
            for field in required_state:
                if not isinstance(value[field], str) or not value[field]:
                    issues.append(f"{label} {field} is invalid")
            if value["source_node"] in seen_source:
                issues.append(f"duplicate source state {value['source_node']}")
            if value["target_state"] in seen_target:
                issues.append(f"duplicate target state {value['target_state']}")
            seen_source.add(value["source_node"])
            seen_target.add(value["target_state"])
    required_transition = {"source_node", "target_transition"}
    if isinstance(mapping.get("transition_map"), list):
        seen_source = set()
        seen_target = set()
        for index, value in enumerate(mapping["transition_map"]):
            label = f"transition_map[{index}]"
            if (
                not isinstance(value, dict)
                or not required_transition.issubset(value)
                or not set(value).issubset(required_transition | {"property_assertions"})
            ):
                issues.append(f"{label} is invalid")
                continue
            for field in ("source_node", "target_transition"):
                if not isinstance(value[field], str) or not value[field]:
                    issues.append(f"{label} {field} is invalid")
            if value["source_node"] in seen_source:
                issues.append(f"duplicate source transition {value['source_node']}")
            if value["target_transition"] in seen_target:
                issues.append(f"duplicate target transition {value['target_transition']}")
            seen_source.add(value["source_node"])
            seen_target.add(value["target_transition"])
            issues.extend(
                property_assertion_issues(
                    value.get("property_assertions", []),
                    f"{label}.property_assertions",
                )
            )
    entry = mapping.get("entry")
    if not isinstance(entry, dict) or set(entry) != {"source_node", "target_state"}:
        issues.append("entry is invalid")
    elif any(not isinstance(entry[key], str) or not entry[key] for key in entry):
        issues.append("entry values are invalid")
    if not isinstance(mapping.get("comment_groups"), list):
        issues.append("comment_groups must be an array")
    policy = mapping.get("comment_policy", "require")
    if policy not in COMMENT_POLICIES:
        issues.append("comment_policy is invalid")
    if policy == "ignore" and (
        not isinstance(mapping.get("comment_exception"), str)
        or not mapping["comment_exception"]
    ):
        issues.append("comment_exception is required when comment_policy is ignore")
    return issues


def target_start_state(text):
    match = re.search(r'\bstart_state\s*:\s*"([^"]+)"', text)
    if match is None:
        raise ValueError("target FSM start_state is missing")
    return match.group(1)


def audit(root, mapping, source_path=None):
    issues = map_shape_issues(mapping)
    report = {"issues": issues, "differences": [], "pass": False}
    if issues:
        return report
    source_contract_path = root / mapping["source_contract"]
    target_path = root / mapping["target_fsm"]
    if not source_contract_path.is_file() or not target_path.is_file():
        report["issues"] = ["source contract or target FSM is missing"]
        return report
    contract = read_json(source_contract_path)
    if contract.get("format") != SOURCE_FORMAT:
        report["issues"] = ["source contract format is invalid"]
        return report
    target_text = target_path.read_text(encoding="utf-8")
    target = capture_fsm(target_text)
    source_state_values = source_states(contract)
    source_transition_values = source_transitions(contract)
    source_entry_values = source_entries(contract)
    source_state_by_path = {node["object_path"]: node for node in source_state_values}
    source_transition_by_path = {
        node["object_path"]: node for node in source_transition_values
    }
    target_state_by_id = {value["id"]: value for value in target["states"]}
    target_transition_by_id = {value["id"]: value for value in target["transitions"]}
    target_transition_entries = raw_fsm_entries(target_text, "transitions")
    static_contract_runtime_path = mapping.get("transition_static_contract_runtime_path")
    static_contract_runtime_text = None
    differences = []
    if static_contract_runtime_path is not None:
        runtime_file = root / static_contract_runtime_path
        if runtime_file.is_file():
            static_contract_runtime_text = runtime_file.read_text(encoding="utf-8")
        else:
            differences = [
                {
                    "transition_static_contract_runtime_path_missing": static_contract_runtime_path
                }
            ]
    target_state_positions = {
        value["id"]: [float(value["position"][0]), float(value["position"][1])]
        for value in target["state_positions"]["entries"]
        if value["position"] is not None
    }
    state_map = {value["source_node"]: value for value in mapping["state_map"]}
    transition_map = {
        value["source_node"]: value for value in mapping["transition_map"]
    }
    report["differences"] = differences
    if set(state_map) != set(source_state_by_path):
        differences.append(
            {
                "state_map_coverage": {
                    "missing": sorted(set(source_state_by_path) - set(state_map)),
                    "unknown": sorted(set(state_map) - set(source_state_by_path)),
                }
            }
        )
    if set(transition_map) != set(source_transition_by_path):
        differences.append(
            {
                "transition_map_coverage": {
                    "missing": sorted(
                        set(source_transition_by_path) - set(transition_map)
                    ),
                    "unknown": sorted(
                        set(transition_map) - set(source_transition_by_path)),
                }
            }
        )
    expected_state_order = [
        state_map[node["object_path"]]["target_state"]
        for node in source_state_values
        if node["object_path"] in state_map
    ]
    actual_state_order = [value["id"] for value in target["states"]]
    if expected_state_order != actual_state_order:
        differences.append(
            {
                "state_order": {
                    "source_mapped_target_order": expected_state_order,
                    "target_order": actual_state_order,
                }
            }
        )
    expected_transition_order = [
        transition_map[node["object_path"]]["target_transition"]
        for node in source_transition_values
        if node["object_path"] in transition_map
    ]
    actual_transition_order = [value["id"] for value in target["transitions"]]
    if expected_transition_order != actual_transition_order:
        differences.append(
            {
                "transition_order": {
                    "source_mapped_target_order": expected_transition_order,
                    "target_order": actual_transition_order,
                }
            }
        )
    for source_path_value, value in state_map.items():
        source = source_state_by_path.get(source_path_value)
        target_state = target_state_by_id.get(value["target_state"])
        if source is None or target_state is None:
            continue
        source_position = [float(source["position_x"]), float(source["position_y"])]
        if target_state["label"] != source["title"]:
            differences.append({"state_label": value})
        if target_state["graph"] != value["target_graph"]:
            differences.append({"state_graph": value})
        if target_state_positions.get(value["target_state"]) != source_position:
            differences.append(
                {
                    "state_position": {
                        "source_node": source_path_value,
                        "source": source_position,
                        "target": target_state_positions.get(value["target_state"]),
                    }
                }
            )
    if len(source_entry_values) != 1:
        differences.append({"source_entry": "missing or ambiguous"})
    else:
        entry = source_entry_values[0]
        entry_target = source_entry_target(contract, entry)
        mapped_entry = mapping["entry"]
        if mapped_entry["source_node"] != entry["object_path"]:
            differences.append({"entry_source": mapped_entry})
        elif len(entry_target) != 1 or entry_target[0] not in state_map:
            differences.append({"entry_target": entry_target})
        elif mapped_entry["target_state"] != state_map[entry_target[0]]["target_state"]:
            differences.append({"entry_mapping": mapped_entry})
        elif target_start_state(target_text) != mapped_entry["target_state"]:
            differences.append({"start_state": mapped_entry})
    for source_path_value, value in transition_map.items():
        source = source_transition_by_path.get(source_path_value)
        target_transition = target_transition_by_id.get(value["target_transition"])
        if source is None or target_transition is None:
            continue
        incoming, outgoing = source_transition_endpoints(contract, source)
        if len(incoming) != 1 or len(outgoing) != 1:
            differences.append(
                {
                    "transition_source_endpoints": {
                        "source_node": source_path_value,
                        "incoming": incoming,
                        "outgoing": outgoing,
                    }
                }
            )
            continue
        expected_source = state_map.get(incoming[0], {}).get("target_state")
        expected_target = state_map.get(outgoing[0], {}).get("target_state")
        if (
            target_transition["source"] != expected_source
            or target_transition["target"] != expected_target
        ):
            differences.append(
                {
                    "transition_endpoints": {
                        "source_node": source_path_value,
                        "target_transition": value["target_transition"],
                        "expected_source": expected_source,
                        "expected_target": expected_target,
                        "actual_source": target_transition["source"],
                        "actual_target": target_transition["target"],
                    }
                }
            )
        source_static = source_transition_static_properties(source)
        source_text = "\n".join(
            f"{property_value.get('name')}={property_value.get('exported_text', '')}"
            for property_value in source.get("properties", [])
            if isinstance(property_value, dict)
        )
        assertions = [
            *[
                assertion
                for assertion in mapping.get("transition_default_assertions", [])
                if assertion["source_contains"] in source_text
            ],
            *value.get("property_assertions", []),
        ]
        assertion_sources = {assertion.get("source_contains") for assertion in assertions}
        source_static_contract = source_transition_static_contract(source)
        static_contract_matches = (
            static_contract_runtime_text is not None
            and source_static_contract in static_contract_runtime_text
        )
        if static_contract_runtime_path is not None and not static_contract_matches:
            differences.append(
                {
                    "transition_static_contract_missing": {
                        "source_node": source_path_value,
                        "source_static_contract": source_static_contract,
                        "target_runtime_path": static_contract_runtime_path,
                    }
                }
            )
        missing_static = [] if static_contract_matches else [
            source_value
            for source_value in source_static
            if source_value not in assertion_sources
        ]
        if missing_static:
            differences.append(
                {
                    "transition_static_property_coverage": {
                        "source_node": source_path_value,
                        "missing": missing_static,
                    }
                }
            )
        target_entry = target_transition_entries.get(value["target_transition"], "")
        for assertion in assertions:
            row = {
                "source_node": source_path_value,
                "target_transition": value["target_transition"],
                **assertion,
            }
            if assertion["source_contains"] not in source_text:
                differences.append({"transition_source_property_assertion_missing": row})
            runtime_path = assertion.get("target_runtime_path")
            if runtime_path is None:
                target_text_for_assertion = target_entry
            else:
                runtime_file = root / runtime_path
                target_text_for_assertion = (
                    runtime_file.read_text(encoding="utf-8")
                    if runtime_file.is_file()
                    else ""
                )
            if runtime_path is not None and not target_text_for_assertion:
                differences.append({"transition_target_runtime_path_missing": row})
            if "target_exception" not in assertion and assertion["target_contains"] not in target_text_for_assertion:
                differences.append({"transition_target_property_assertion_missing": row})
    comments = source_comments(contract)
    if comments and mapping.get("comment_policy", "require") == "require":
        differences.append(
            {
                "comment_groups": [
                    {
                        "source_node": node["object_path"],
                        "position": [node["position_x"], node["position_y"]],
                    }
                    for node in comments
                ]
            }
        )
    report.update(
        {
            "source_contract": mapping["source_contract"],
            "target_fsm": mapping["target_fsm"],
            "source_states": len(source_state_values),
            "target_states": len(target["states"]),
            "source_transitions": len(source_transition_values),
            "target_transitions": len(target["transitions"]),
            "source_comments": len(comments),
            "pass": not differences,
        }
    )
    if source_path is not None:
        verify_source_contract(root, mapping, source_path)
        report["source_contract_fresh"] = True
    return report


def run_self_test():
    source = {
        "format": SOURCE_FORMAT,
        "source_graph": {
            "nodes": [
                {
                    "object_path": "/Asset:Fsm.Entry",
                    "class_path": "/Script/AnimGraph.AnimStateEntryNode",
                    "title": "Fsm",
                    "position_x": 0,
                    "position_y": 0,
                },
                {
                    "object_path": "/Asset:Fsm.Idle",
                    "class_path": "/Script/AnimGraph.AnimStateNode",
                    "title": "Idle",
                    "position_x": 64,
                    "position_y": 0,
                },
                {
                    "object_path": "/Asset:Fsm.Run",
                    "class_path": "/Script/AnimGraph.AnimStateNode",
                    "title": "Run",
                    "position_x": 128,
                    "position_y": 0,
                },
                {
                    "object_path": "/Asset:Fsm.Go",
                    "class_path": "/Script/AnimGraph.AnimStateTransitionNode",
                    "title": "Go",
                    "position_x": 96,
                    "position_y": 32,
                    "properties": [
                        {
                            "name": "CrossfadeDuration",
                            "exported_text": "0.100000",
                        },
                        {
                            "name": "PriorityOrder",
                            "exported_text": "0.100000",
                        },
                        {
                            "name": "SyncGroupNameToRequireValidMarkersRule",
                            "exported_text": "None",
                        }
                    ],
                },
            ]
        },
        "links": [
            {"source": {"node_object_path": "/Asset:Fsm.Entry"}, "target": {"node_object_path": "/Asset:Fsm.Idle"}},
            {"source": {"node_object_path": "/Asset:Fsm.Idle"}, "target": {"node_object_path": "/Asset:Fsm.Go"}},
            {"source": {"node_object_path": "/Asset:Fsm.Go"}, "target": {"node_object_path": "/Asset:Fsm.Run"}},
        ],
    }
    fsm = (
        "(\n"
        'states: [(id: "idle", label: "Idle", graph: "graphs/idle.animgraph.ron"), (id: "run", label: "Run", graph: "graphs/run.animgraph.ron")],\n'
        'transitions: [(id: "go", source: "idle", target: "run", data: (timed: Some(0.1)))],\n'
        'start_state: "idle",\n'
        'node_spec: (input_times: [], input_data: {}, output_time: false, output_data: { "pose": Pose }, input_order: {}, output_order: { Data("pose"): 0 }),\n'
        'editor_metadata: (states: { "idle": (64.0, 0.0), "run": (128.0, 0.0) }),\n'
        ")\n"
    )
    mapping = {
        "format": FORMAT,
        "source_contract": "source.json",
        "target_fsm": "target.fsm.ron",
        "state_map": [
            {"source_node": "/Asset:Fsm.Idle", "target_state": "idle", "target_graph": "graphs/idle.animgraph.ron"},
            {"source_node": "/Asset:Fsm.Run", "target_state": "run", "target_graph": "graphs/run.animgraph.ron"},
        ],
        "transition_map": [{"source_node": "/Asset:Fsm.Go", "target_transition": "go", "property_assertions": [{"source_contains": "CrossfadeDuration=0.100000", "target_contains": "timed: Some(0.1)"}, {"source_contains": "PriorityOrder=0.100000", "target_contains": "timed: Some(0.1)"}, {"source_contains": "SyncGroupNameToRequireValidMarkersRule=None", "target_exception": "Fixture target has no marker gate.", "target_runtime_path": "target.fsm.ron"}]}],
        "entry": {"source_node": "/Asset:Fsm.Entry", "target_state": "idle"},
        "comment_groups": [],
        "comment_policy": "ignore",
        "comment_exception": "fixture",
    }
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        (root / "source.json").write_text(json.dumps(source), encoding="utf-8")
        (root / "target.fsm.ron").write_text(fsm, encoding="utf-8")
        report = audit(root, mapping)
        assert report["pass"]
        missing_static = json.loads(json.dumps(mapping))
        missing_static["transition_map"][0]["property_assertions"] = []
        assert not audit(root, missing_static)["pass"]
        missing_static_field = json.loads(json.dumps(mapping))
        missing_static_field["transition_map"][0].pop("property_assertions")
        assert not audit(root, missing_static_field)["pass"]
        bundled_static = json.loads(json.dumps(mapping))
        bundled_static["transition_map"][0]["property_assertions"] = []
        bundled_static["transition_static_contract_runtime_path"] = "runtime.rs"
        static_contract = source_transition_static_contract(source["source_graph"]["nodes"][3])
        (root / "runtime.rs").write_text(
            f"runtime_binding runtime_default_false {static_contract}",
            encoding="utf-8",
        )
        assert audit(root, bundled_static)["pass"]
        defaults = json.loads(json.dumps(mapping))
        defaults["transition_default_assertions"] = [
            defaults["transition_map"][0]["property_assertions"].pop()
        ]
        assert audit(root, defaults)["pass"]
        ambiguous_static = json.loads(json.dumps(mapping))
        ambiguous_static["transition_map"][0]["property_assertions"] = [
            {
                "source_contains": "0.100000",
                "target_contains": "timed: Some(0.1)",
            }
        ]
        assert not audit(root, ambiguous_static)["pass"]
        invalid_exception = json.loads(json.dumps(mapping))
        invalid_exception["transition_map"][0]["property_assertions"][0] = {
            "source_contains": "CrossfadeDuration=0.100000",
            "target_exception": "Invalid for a non-None source value.",
            "target_runtime_path": "target.fsm.ron",
        }
        assert not audit(root, invalid_exception)["pass"]
        raw_source = {
            "asset_path": "/Asset",
            "asset_objects": [],
            "graphs": [
                {
                    "index": 0,
                    "object_path": "/Asset:Fsm",
                    "class_path": "/Script/AnimGraph.AnimationStateMachineGraph",
                    "name": "Fsm",
                    "properties": [],
                    "nodes": [],
                }
            ],
        }
        raw_path = root / "raw.json"
        raw_path.write_text(json.dumps(raw_source), encoding="utf-8")
        source_contract = contract(raw_source, "Fsm")
        (root / "fresh.json").write_text(json.dumps(source_contract), encoding="utf-8")
        verify_source_contract(
            root,
            {"source_contract": "fresh.json"},
            raw_path,
        )
    return {
        "state_and_transition_topology": "PASS",
        "transition_positions_out_of_scope": "PASS",
        "native_source_contract_rechecked": "PASS",
        "transition_static_property_assertions_required": "PASS",
        "missing_transition_assertion_field_rejected_by_coverage": "PASS",
        "same_value_different_property_rejected": "PASS",
        "non_none_transition_exception_rejected": "PASS",
        "shared_transition_default_assertions_supported": "PASS",
        "full_transition_static_contract_runtime_evidence_supported": "PASS",
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--map", type=Path)
    parser.add_argument("--source", type=Path)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
        return 0
    if args.map is None or not args.check:
        raise ValueError("map and --check are required")
    root = args.root.resolve()
    source_path = None
    if args.source is not None:
        source_path = args.source if args.source.is_absolute() else root / args.source
        if not source_path.is_file():
            raise ValueError("source extraction is missing")
    report = audit(root, read_json(root / args.map), source_path)
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if report["pass"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
