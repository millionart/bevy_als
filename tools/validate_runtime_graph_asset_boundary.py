import argparse
import json
import tempfile
from pathlib import Path


AUDIT_DIRECTORY_NAMES = {"source_conditions"}
AUDIT_FILE_PREFIXES = ("ue_source_transition_",)


def audit(assets_root: Path) -> list[str]:
    issues = []
    if not assets_root.is_dir():
        return [f"assets root is missing: {assets_root.as_posix()}"]
    for path in assets_root.rglob("*"):
        relative = path.relative_to(assets_root).as_posix()
        if path.is_dir() and path.name in AUDIT_DIRECTORY_NAMES:
            issues.append(f"audit directory is inside runtime assets: {relative}")
        if path.is_file() and path.suffix == ".ron" and path.name.startswith(AUDIT_FILE_PREFIXES):
            issues.append(f"audit graph is inside runtime assets: {relative}")
    for path in assets_root.rglob("*.ron"):
        text = path.read_text(encoding="utf-8")
        if "source_conditions/" in text or "source_conditions\\" in text:
            issues.append(f"runtime asset references audit directory: {path.relative_to(assets_root).as_posix()}")
        if "ue_source_transition_" in text:
            issues.append(f"runtime asset references audit graph: {path.relative_to(assets_root).as_posix()}")
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
    return {
        "audit_only_graphs_rejected_from_runtime_assets": "PASS",
        "runtime_reference_to_audit_graph_rejected": "PASS",
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
