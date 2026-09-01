import argparse
import hashlib
import json
from pathlib import Path

from apply_source_layout import (
    all_source_nodes,
    place_nodes,
    source_nodes,
    source_owner,
    target_edges,
    target_nodes,
)
from audit_ab_als_root import audit as audit_root
from audit_cr_als import validate_profile, validate_skeleton, validate_source_raw
from audit_default_overlay import audit as audit_default
from audit_source_graph_coverage import payload as coverage_payload
from capture_animgraph_ron_topology import parse_positions


ROOT = Path(__file__).resolve().parents[2]
SOURCE_ROOT = ROOT / "target" / "bf109" / "source_native"
TARGET_ROOT = ROOT / "assets" / "graphs" / "ab_als"
DEFAULT_REPORT = ROOT / "target" / "bf109" / "audits" / "BF109.final.json"


GENERIC_CASES = [
    ("AB_Als_Standing", "AB_Als_Standing.ue.json", "standing_source_layout_map.json"),
    ("AB_Als_Crouching", "AB_Als_Crouching.ue.json", "crouching_source_layout_map.json"),
    ("AB_Als_Grounded", "AB_Als_Grounded.ue.json", "grounded_source_layout_map.json"),
    ("AB_Als_Locomotion", "AB_Als_Locomotion.ue.json", "locomotion_source_layout_map.json"),
    ("AB_Als_Layering", "AB_Als_Layering.ue.json", "layering_source_layout_map.json"),
    ("AB_Als_Head", "AB_Als_Head.ue.json", "head_source_layout_map.json"),
    ("AB_Als_Ragdolling", "AB_Als_Ragdolling.ue.json", "ragdolling_source_layout_map.json"),
    ("ALI_Overlay", "ALI_Overlay.ue.json", "ali_overlay_source_layout_map.json"),
]


REQUIRED_SOURCES = [
    "AB_Als_Standing.ue.json",
    "AB_Als_Crouching.ue.json",
    "AB_Als_Grounded.ue.json",
    "AB_Als_Locomotion.ue.json",
    "AB_Als_Layering.ue.json",
    "AB_Als_Head.ue.json",
    "AB_Als_Ragdolling.ue.json",
    "ALI_Overlay.ue.json",
    "PA_Als.ue.json",
    "CR_Als.ue.json",
    "AB_Als_Default.ue.json",
    "AB_Als.ue.json",
    "SKM_Als.ue.json",
]


def canonical_hash(value):
    encoded = json.dumps(value, ensure_ascii=False, separators=(",", ":"), sort_keys=True).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def source_summary(name, source):
    return {
        "name": name,
        "asset_path": source["asset_path"],
        "asset_class": source["asset_class"],
        "sha256": canonical_hash(source),
        "graphs": len(source.get("graphs", [])),
        "nodes": sum(len(graph["nodes"]) for graph in source.get("graphs", [])),
        "pins": sum(
            len(node["pins"])
            for graph in source.get("graphs", [])
            for node in graph["nodes"]
        ),
        "asset_objects": len(source.get("asset_objects", [])),
    }


def generic_audit(source, mapping):
    source_graphs = {graph["object_path"]: graph for graph in source["graphs"]}
    source_by_path = all_source_nodes(source_graphs)
    differences = []
    for entry in mapping["graphs"]:
        source_path = f'{mapping["source_asset"]}:{entry["source_suffix"]}'
        source_graph = source_graphs.get(source_path)
        if source_graph is None:
            differences.append(f"missing source graph {source_path}")
            continue
        if entry.get("graph_scope_only"):
            continue
        target_path = TARGET_ROOT / entry["target"]
        target_text = target_path.read_text(encoding="utf-8")
        positions, _ = place_nodes(
            target_nodes(target_text),
            target_edges(target_text),
            source_nodes(source_graph),
            source_by_path,
            source_owner(source_graph, source_by_path),
            entry.get("node_overrides", []),
        )
        actual = {
            value["id"]: value["position"]
            for value in parse_positions(target_text, "node_positions")["entries"]
        }
        if actual != positions:
            differences.append(f"positions {entry['target']}")
    coverage = coverage_payload(source, TARGET_ROOT, mapping)
    for key in (
        "missing_source_graphs",
        "missing_source_anchors",
        "unexplained_authored_graphs",
        "unexplained_source_nodes",
    ):
        if coverage[key]:
            differences.append(key)
    return differences, coverage


def pa_audit(source):
    physics = source.get("physics_asset", {})
    target = json.loads((ROOT / "assets" / "ragdolls" / "pa_als.physics.json").read_text(encoding="utf-8"))
    source_shapes = sum(len(body["shapes"]) for body in physics.get("bodies", []))
    differences = []
    if len(physics.get("bodies", [])) != 21 or len(target.get("bodies", [])) != 21:
        differences.append("bodies")
    if source_shapes != 22:
        differences.append("source shapes")
    if len(physics.get("constraints", [])) != 20 or len(target.get("constraints", [])) != 20:
        differences.append("constraints")
    if len(physics.get("collision_disabled_pairs", [])) != 49 or len(target.get("collision_disabled_pairs", [])) != 49:
        differences.append("collision disabled pairs")
    return differences, {
        "source_bodies": len(physics.get("bodies", [])),
        "source_shapes": source_shapes,
        "source_constraints": len(physics.get("constraints", [])),
        "source_collision_disabled_pairs": len(physics.get("collision_disabled_pairs", [])),
    }


def run():
    differences = []
    sources = {}
    for filename in REQUIRED_SOURCES:
        path = SOURCE_ROOT / filename
        if not path.is_file():
            differences.append(f"missing source output {filename}")
            continue
        source = json.loads(path.read_text(encoding="utf-8"))
        sources[filename] = source
    summaries = [source_summary(name, source) for name, source in sources.items()]
    generic = {}
    for name, filename, mapping_name in GENERIC_CASES:
        source = sources.get(filename)
        if source is None:
            continue
        mapping = json.loads((ROOT / "tools" / "import" / mapping_name).read_text(encoding="utf-8"))
        case_differences, coverage = generic_audit(source, mapping)
        generic[name] = {"differences": case_differences, "coverage": coverage}
        differences.extend(f"{name} {value}" for value in case_differences)
    default_source = sources.get("AB_Als_Default.ue.json")
    if default_source is not None:
        default_contract = json.loads(
            (ROOT / "docs" / "bf109" / "source_contracts" / "AB_Als_Default.source.json").read_text(encoding="utf-8")
        )
        default_mapping = json.loads((ROOT / "tools" / "import" / "default_source_layout_map.json").read_text(encoding="utf-8"))
        default_differences, default_report = audit_default(
            default_source,
            default_contract,
            (TARGET_ROOT / "overlays" / "default.animgraph.ron").read_text(encoding="utf-8"),
            default_mapping,
        )
        differences.extend(f"AB_Als_Default {value}" for value in default_differences)
    else:
        default_report = {}
    root_source = sources.get("AB_Als.ue.json")
    if root_source is not None:
        root_contract = json.loads(
            (ROOT / "docs" / "bf109" / "source_contracts" / "AB_Als.source.json").read_text(encoding="utf-8")
        )
        root_mapping = json.loads((ROOT / "tools" / "import" / "ab_als_source_layout_map.json").read_text(encoding="utf-8"))
        root_differences, root_report = audit_root(
            root_source,
            root_contract,
            (TARGET_ROOT / "ab_als.animgraph.ron").read_text(encoding="utf-8"),
            root_mapping,
        )
        differences.extend(f"AB_Als {value}" for value in root_differences)
    else:
        root_report = {}
    cr_source = sources.get("CR_Als.ue.json")
    mesh_source = sources.get("SKM_Als.ue.json")
    if cr_source is not None and mesh_source is not None:
        cr_differences = []
        profile = json.loads((ROOT / "assets" / "control_rigs" / "cr_als.profile.json").read_text(encoding="utf-8"))
        runtime = json.loads((ROOT / "assets" / "control_rigs" / "cr_als.runtime.json").read_text(encoding="utf-8"))
        validate_source_raw(cr_source, cr_differences)
        validate_profile(
            cr_source,
            profile,
            runtime,
            (ROOT / "src" / "control_rig_apply.rs").read_text(encoding="utf-8"),
            (ROOT / "src" / "foot_ik_apply.rs").read_text(encoding="utf-8"),
            cr_differences,
        )
        validate_skeleton(
            mesh_source,
            json.loads((ROOT / "assets" / "animations" / "Base" / "A_Als_Idle.gltf").read_text(encoding="utf-8")),
            cr_differences,
        )
        differences.extend(f"CR_Als {value}" for value in cr_differences)
        cr_report = {
            "differences": cr_differences,
            "source_graphs": len(cr_source["graphs"]),
            "source_nodes": sum(len(graph["nodes"]) for graph in cr_source["graphs"]),
            "source_pins": sum(len(node["pins"]) for graph in cr_source["graphs"] for node in graph["nodes"]),
            "source_rig_vm_links": sum(
                item["class_path"] == "/Script/RigVMDeveloper.RigVMLink"
                for item in cr_source["asset_objects"]
            ),
        }
    else:
        cr_report = {}
    pa_source = sources.get("PA_Als.ue.json")
    if pa_source is not None:
        pa_differences, pa_report = pa_audit(pa_source)
        differences.extend(f"PA_Als {value}" for value in pa_differences)
    else:
        pa_report = {}
    return {
        "sources": summaries,
        "generic": generic,
        "default": default_report,
        "root": root_report,
        "control_rig": cr_report,
        "physics_asset": pa_report,
        "differences": differences,
        "pass": not differences,
    }


def self_test():
    value = {"value": [1, 2, 3]}
    if canonical_hash(value) != canonical_hash({"value": [1, 2, 3]}):
        raise AssertionError("canonical hash")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--report", type=Path, default=DEFAULT_REPORT)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    self_test()
    if args.self_test:
        print("BF109 final audit self-test PASS")
        return
    report = run()
    if not args.check:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(
        "BF109 final audit "
        f"sources={len(report['sources'])} "
        f"generic_cases={len(report['generic'])} "
        f"differences={len(report['differences'])} "
        f"-> {'PASS' if report['pass'] else 'FAIL'}"
    )
    if report["differences"]:
        for difference in report["differences"]:
            print(f"DIFF {difference}")
        raise SystemExit(1)


if __name__ == "__main__":
    main()
