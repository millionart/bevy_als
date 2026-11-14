import argparse
import json
import tempfile
from pathlib import Path


AUDIT_DIRECTORY_NAMES = {"source_conditions"}
AUDIT_FILE_PREFIXES = ("ue_source_transition_",)
RETIRED_OVERLAY_PATHS = ("graphs/ab_als/overlays/", "graphs/overlay_objects/bow.animgraph.ron", "graphs/ab_als/ab_als.animgraph.ron")
FORMAL_ROOTS = {"ALS", "ALSCamera", "ALSExtras", "ALSRuntime", "Engine", "Bevy"}
ASSET_ROOTS = FORMAL_ROOTS | {"Validation", "docs"}


def audit(assets_root: Path) -> list[str]:
    issues = []
    if not assets_root.is_dir():
        return [f"assets root is missing: {assets_root.as_posix()}"]
    for path in assets_root.rglob("*"):
        relative = path.relative_to(assets_root).as_posix()
        owner = relative.split("/", 1)[0]
        if path.is_file() and owner not in ASSET_ROOTS and relative != "README.md" and path.name != ".DS_Store":
            issues.append(f"asset has no source module: {relative}")
        if owner in FORMAL_ROOTS and "generated" in relative.lower():
            issues.append(f"formal asset has a generated path: {relative}")
        if owner in {"Validation", "docs"}:
            continue
        if path.is_dir() and path.name in AUDIT_DIRECTORY_NAMES:
            issues.append(f"audit directory is inside runtime assets: {relative}")
        if path.is_file() and path.suffix == ".ron" and path.name.startswith(AUDIT_FILE_PREFIXES):
            issues.append(f"audit graph is inside runtime assets: {relative}")
        if path.is_file() and any(relative.startswith(prefix) for prefix in RETIRED_OVERLAY_PATHS):
            issues.append(f"retired manual overlay asset: {relative}")
    for path in assets_root.rglob("*.ron"):
        if path.relative_to(assets_root).parts[0] in {"Validation", "docs"}:
            continue
        text = path.read_text(encoding="utf-8")
        if "generated/" in text or "Validation/" in text:
            issues.append(f"formal asset references generated or validation content: {path.relative_to(assets_root).as_posix()}")
        if "source_conditions/" in text or "source_conditions\\" in text:
            issues.append(f"runtime asset references audit directory: {path.relative_to(assets_root).as_posix()}")
        if "ue_source_transition_" in text:
            issues.append(f"runtime asset references audit graph: {path.relative_to(assets_root).as_posix()}")
        if any(prefix in text for prefix in RETIRED_OVERLAY_PATHS):
            issues.append(f"runtime asset references retired manual overlay: {path.relative_to(assets_root).as_posix()}")
    return sorted(set(issues))


def run_self_test() -> dict:
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory) / "assets"
        root.mkdir(parents=True)
        assert not audit(root)
        audit_dir = root / "graphs" / "source_conditions"
        audit_dir.mkdir(parents=True)
        (audit_dir / "ue_source_transition_00.animgraph.ron").write_text("()\n", encoding="utf-8")
        issues = audit(root)
        assert any("audit directory" in issue for issue in issues)
        assert any("audit graph" in issue for issue in issues)
        runtime = root / "graphs" / "runtime.animgraph.ron"
        runtime.parent.mkdir(parents=True, exist_ok=True)
        runtime.write_text('graph: "source_conditions/test"\n', encoding="utf-8")
        assert any("references audit directory" in issue for issue in audit(root))
        old = root / "graphs/ab_als/overlays/old.animgraph.ron"
        old.parent.mkdir(parents=True)
        old.write_text("()\n", encoding="utf-8")
        runtime.write_text('graph: "graphs/ab_als/overlays/old.animgraph.ron"\n', encoding="utf-8")
        assert any("retired manual overlay asset" in issue for issue in audit(root))
        assert any("references retired manual overlay" in issue for issue in audit(root))
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        formal = root / "ALS/Character/AB_Als/AnimGraph.animgraph.ron"
        formal.parent.mkdir(parents=True)
        formal.write_text("()\n", encoding="utf-8")
        fixture = root / "Validation/Graphs/old.animgraph.ron"
        fixture.parent.mkdir(parents=True)
        fixture.write_text("()\n", encoding="utf-8")
        runtime = root / "ALSRuntime/Animations/A_Test.gltf"
        runtime.parent.mkdir(parents=True)
        runtime.touch()
        (root / ".DS_Store").touch()
        (runtime.parent / ".DS_Store").touch()
        issues = audit(root)
        assert not issues, issues
        unknown = root / "Unknown/.DS_Store.backup"
        unknown.parent.mkdir()
        unknown.touch()
        assert audit(root) == ["asset has no source module: Unknown/.DS_Store.backup"]
        unknown.unlink()
        formal.write_text('graph: "Validation/Graphs/old.animgraph.ron"', encoding="utf-8")
        assert any("validation content" in issue for issue in audit(root))
        formal.write_text("()\n", encoding="utf-8")
        (formal.parent / "generated_root.animgraph.ron").write_text("()\n", encoding="utf-8")
        assert any("generated path" in issue for issue in audit(root))
        (runtime.parent / "generated_test.gltf").touch()
        assert "formal asset has a generated path: ALSRuntime/Animations/generated_test.gltf" in audit(root)
    return {
        "audit_only_graphs_rejected_from_runtime_assets": "PASS",
        "runtime_reference_to_audit_graph_rejected": "PASS",
        "retired_manual_overlay_and_reference_rejected": "PASS",
        "source_module_assets_and_separate_validation_fixtures": "PASS",
        "derived_als_assets_and_finder_metadata_classified": "PASS",
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--assets-root", type=Path, default=Path("assets"))
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(run_self_test(), indent=2))
        return 0
    issues = audit(args.assets_root)
    print(json.dumps({"issues": issues}, indent=2))
    return 0 if not issues or not args.check else 1


if __name__ == "__main__":
    raise SystemExit(main())
