import argparse
import json
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MAX_SKILL_LINES = 120
REQUIRED_HEADINGS = (
    "## Scope",
    "## Source evidence",
    "## Static parity",
    "## Runtime behavior",
    "## Failure reports and repair",
    "## Boundaries",
)
REQUIRED_TEXT = (
    "OpenEditorForAsset",
    "raw source exports",
    "target-space data",
    "actual FSM state",
    "primary_failure",
)
REQUIRED_TOOLS = (
    "tools/validate_animgraph_authoring.py",
    "tools/validate_animgraph_editor_assets.py",
    "tools/verify_animgraph_node_positions.py",
    "tools/import/run_ue_visible_animgraph_audit.py",
    "tools/import/audit_ue_visible_fsm.py",
    "tools/validate_runtime_graph_asset_boundary.py",
    "tools/run_animation_visual_validation.ps1",
    "tools/validate_animation_visual_evidence.py",
)


def validate(root, skill_path):
    if not skill_path.is_file():
        return ["skill file is missing"]
    text = skill_path.read_text(encoding="utf-8")
    lines = text.splitlines()
    issues = []
    if not text.startswith("---\n") or "\n---\n" not in text:
        issues.append("skill frontmatter is invalid")
    if len(lines) > MAX_SKILL_LINES:
        issues.append(f"skill exceeds {MAX_SKILL_LINES} lines")
    for value in REQUIRED_HEADINGS + REQUIRED_TEXT:
        if value not in text:
            issues.append(f"skill requirement is missing: {value}")
    for value in REQUIRED_TOOLS:
        if not (root / value).is_file():
            issues.append(f"required tool is missing: {value}")
    return issues


def fixture_skill():
    headings = "\n\n".join(REQUIRED_HEADINGS)
    terms = "\n".join(REQUIRED_TEXT + REQUIRED_TOOLS)
    return f"---\nname: fixture\ndescription: fixture\n---\n\n# Fixture\n\n{headings}\n\n{terms}\n"


def run_self_test():
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        skill = root / "SKILL.md"
        skill.write_text("## Scope", encoding="utf-8")
        issues = validate(root, skill)
        assert any("frontmatter" in issue for issue in issues)
        assert any("Failure reports and repair" in issue for issue in issues)
        skill.write_text(fixture_skill() + ("x\n" * MAX_SKILL_LINES), encoding="utf-8")
        issues = validate(root, skill)
        assert any("exceeds" in issue for issue in issues)
        skill.write_text(fixture_skill(), encoding="utf-8")
        issues = validate(root, skill)
        assert any("required tool is missing" in issue for issue in issues)
    return {
        "required_structure_rejected": "PASS",
        "oversized_skill_rejected": "PASS",
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
