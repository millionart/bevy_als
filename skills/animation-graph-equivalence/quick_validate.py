import argparse
import json
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
REQUIRED_TEXT = (
    "Establish source facts first",
    "A `TransitionResult` with no upstream visible node",
    "validate_animgraph_authoring.py",
    "run_ue_visible_animgraph_audit.py",
    "validate_runtime_graph_asset_boundary.py",
)
REQUIRED_TOOLS = (
    "tools/validate_animgraph_authoring.py",
    "tools/import/run_ue_visible_animgraph_audit.py",
    "tools/import/audit_ue_visible_fsm.py",
    "tools/validate_runtime_graph_asset_boundary.py",
)


def validate(root, skill_path):
    issues = []
    if not skill_path.is_file():
        return ["skill file is missing"]
    text = skill_path.read_text(encoding="utf-8")
    for value in REQUIRED_TEXT:
        if value not in text:
            issues.append(f"skill requirement is missing: {value}")
    for value in REQUIRED_TOOLS:
        if not (root / value).is_file():
            issues.append(f"required tool is missing: {value}")
    return issues


def run_self_test():
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        skill = root / "SKILL.md"
        skill.write_text("Establish source facts first", encoding="utf-8")
        assert validate(root, skill)
    return {
        "missing_requirement_rejected": "PASS",
        "missing_tool_rejected": "PASS",
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
        return 0
    issues = validate(ROOT, Path(__file__).resolve().with_name("SKILL.md"))
    print(json.dumps({"issues": issues}, ensure_ascii=False, indent=2))
    return 0 if not issues else 1


if __name__ == "__main__":
    raise SystemExit(main())
