import argparse
import json
import os
import re
import subprocess
from pathlib import Path


TEST_NAME = "self_transition_arrow_uses_finite_geometry"


def valid_workspace(path):
    return path.is_dir() and (path / "Cargo.toml").is_file()


def find_workspace(root, explicit):
    candidates = []
    if explicit is not None:
        candidates.append(("argument", explicit))
    environment_path = os.environ.get("BEVY_ANIMATION_GRAPH_WORKSPACE")
    if environment_path:
        candidates.append(("environment", Path(environment_path)))
    candidates.append(("sibling", root.parent / "bevy_animation_graph"))
    for source, candidate in candidates:
        resolved = candidate.expanduser().resolve()
        if valid_workspace(resolved):
            return source, resolved
    raise FileNotFoundError(
        "animation graph workspace not found; set BEVY_ANIMATION_GRAPH_WORKSPACE or pass --editor-workspace"
    )


def cargo_prefix():
    stable = ["rustup", "run", "stable", "cargo"]
    try:
        result = subprocess.run(
            [*stable, "--version"],
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError:
        result = None
    return stable if result is not None and result.returncode == 0 else ["cargo"]


def locked_editor_revision(root):
    lock = (root / "Cargo.lock").read_text(encoding="utf-8")
    for package in lock.split("[[package]]"):
        if 'name = "bevy_animation_graph_editor"' not in package:
            continue
        match = re.search(r'source = "git\+[^"]+#([0-9a-f]{40})"', package)
        if match is not None:
            return match.group(1)
    raise ValueError("bevy_animation_graph_editor git revision not found in Cargo.lock")


def git_output(workspace, arguments):
    result = subprocess.run(
        ["git", *arguments],
        cwd=workspace,
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        raise RuntimeError(output_tail(result.stdout + result.stderr))
    return result.stdout.strip()


def verify_workspace_revision(root, workspace, allow_drift):
    locked = locked_editor_revision(root)
    current = git_output(workspace, ["rev-parse", "HEAD"])
    dirty = bool(git_output(workspace, ["status", "--porcelain"]))
    if not allow_drift and (current != locked or dirty):
        raise ValueError(
            "editor workspace does not match the clean Cargo.lock revision; commit/update the dependency or pass --allow-workspace-drift"
        )
    return current == locked, not dirty


def output_tail(value):
    lines = [line for line in value.splitlines() if line.strip()]
    return "\n".join(lines[-20:])


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--editor-workspace", type=Path)
    parser.add_argument("--jobs", type=int, default=2)
    parser.add_argument("--allow-workspace-drift", action="store_true")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    try:
        source, workspace = find_workspace(root, args.editor_workspace)
        revision_match, workspace_clean = verify_workspace_revision(
            root, workspace, args.allow_workspace_drift
        )
    except (FileNotFoundError, RuntimeError, ValueError) as error:
        print(json.dumps({"pass": False, "error": str(error)}, indent=2))
        return 1
    command = [
        *cargo_prefix(),
        "test",
        "-p",
        "bevy_animation_graph_editor",
        TEST_NAME,
        "--jobs",
        str(args.jobs),
        "--quiet",
    ]
    try:
        result = subprocess.run(
            command,
            cwd=workspace,
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError as error:
        print(json.dumps({"pass": False, "error": str(error)}, indent=2))
        return 1
    report = {
        "pass": result.returncode == 0,
        "workspace_source": source,
        "revision_match": revision_match,
        "workspace_clean": workspace_clean,
        "test": TEST_NAME,
    }
    if result.returncode != 0:
        report["diagnostic"] = output_tail(result.stdout + result.stderr)
    print(json.dumps(report, indent=2))
    return result.returncode


if __name__ == "__main__":
    raise SystemExit(main())
