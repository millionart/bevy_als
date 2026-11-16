import hashlib
import json
from pathlib import Path
import re
import runpy
import subprocess


BASE = "49673eac"
AUDIT = Path(__file__).resolve().parent
ROOT = AUDIT.parents[3]
CHECKS = runpy.run_path(str(AUDIT.parent / "verify_relocation.py"))
FUNCTION = CHECKS["FUNCTION"]
TEST = CHECKS["TEST"]
canonical = CHECKS["canonical"]


def read(path):
    return (ROOT / path).read_text()


def original(path):
    return subprocess.check_output(["git", "show", f"{BASE}:{path}"], cwd=ROOT, text=True)


def function(text, name):
    match, = (match for match in FUNCTION.finditer(text) if match[2] == name)
    end = text.index("\n" + match[1] + "}", match.end())
    return text[match.start():end + len(match[1]) + 2]


def main():
    manifest = subprocess.check_output(
        ["git", "ls-tree", "-r", "--name-only", BASE, "--", "src"], cwd=ROOT, text=True,
    ).splitlines()
    before_paths = {path for path in manifest if re.match(r"^src/[^/]+_ext/", path)}
    audit_rows = re.findall(r"(?m)^\| `([^`]+)` \|", (AUDIT / "README.md").read_text())
    audit_paths = {
        "src/" + (row if "/" in row else f"animation_graph_ext/{row}") for row in audit_rows
    }
    assert len(before_paths) == len(audit_rows) == 92 and before_paths == audit_paths

    rig_tests = "src/animation_graph_ext/control_rig_program_tests.rs"
    units = "src/control_rig_ext/control_rig_units.rs"
    whole_files = {
        "src/bevy_ext/overlay_materials.rs": "src/als/character/overlay_materials.rs",
        "src/animation_graph_ext/gameplay_tag_blend.rs": "src/als/nodes/gameplay_tag_blend.rs",
        "src/animation_graph_ext/gameplay_tag_layer_blend.rs": "src/als/nodes/gameplay_tag_layer_blend.rs",
    }
    old_fixture = "../../docs/audit/bf144/d214/weights.replay.json"
    new_fixture = "../" + old_fixture
    for previous, current in whole_files.items():
        source = read(current)
        if new_fixture in source:
            assert (ROOT / Path(previous).parent / old_fixture).resolve() == (
                ROOT / Path(current).parent / new_fixture
            ).resolve()
            source = source.replace(new_fixture, old_fixture, 1)
        assert source == original(previous), current
    remaining = {
        path.relative_to(ROOT).as_posix() for path in (ROOT / "src").glob("*_ext/*") if path.is_file()
    }
    assert remaining == before_paths - set(whole_files) - {rig_tests}

    old_tests = original(rig_tests)
    old_units = original(units)
    current_units = read(units)
    names = TEST.findall(old_tests)
    assert len(names) == 3
    assert set(TEST.findall(current_units)) == set(TEST.findall(old_units)) | set(names)
    assert CHECKS["production"](old_units) == CHECKS["production"](current_units)
    for name in names:
        previous = function(old_tests, name)
        current = function(current_units, name).replace("        use RigValue::*;\n", "", 1)
        assert canonical(previous) == canonical(current), name
    assert canonical(function(old_tests, names[-1])) != canonical(
        function(old_tests, names[-1]).replace("instructions.len(), 165", "instructions.len(), 164")
    )

    tnua_motion = "src/tnua_ext/motion.rs"
    actual_motion = "src/als/character/animation_instances/actual_motion.rs"
    prior_motion = original(tnua_motion)
    horizontal_types = prior_motion.split("#[derive(Debug, Clone, Copy, Default, PartialEq)]", 1)[1]
    assert "#[derive(Debug, Clone, Copy, Default, PartialEq)]" + horizontal_types.split("#[cfg(test)]", 1)[0] in read(actual_motion)
    horizontal_tests = [name for name in TEST.findall(prior_motion) if name.startswith("horizontal_motion_")]
    for name in ("falling_slide", "limit_slope_boost", *TEST.findall(read(tnua_motion))):
        assert canonical(function(prior_motion, name)) == canonical(function(read(tnua_motion), name)), name
    for name in ("horizontal_motion_from_world_velocity", *horizontal_tests):
        assert canonical(function(prior_motion, name)) == canonical(function(read(actual_motion), name)), name

    surface = "#[derive(Component, Clone, Debug)]\npub struct PhysicalSurface(pub String);\n"
    query = "src/avian_ext/scene_query.rs"
    decal = "src/bevy_ext/projected_decal.rs"
    assert read(query).replace("\n" + surface, "", 1) == original(query)
    assert read(decal) == original(decal).replace(
        surface, "pub use crate::extensions::scene_query::PhysicalSurface;\n", 1,
    )
    footprint = "src/als/notifies/footprint.rs"
    assert read(footprint).split("use crate::{", 1)[1] == original(footprint).split("use crate::{", 1)[1]
    extensions = "src/extensions.rs"
    first_export = "pub use als_curves::"
    assert read(extensions).split(first_export, 1)[1] == original(extensions).split(first_export, 1)[1]
    subprocess.run(["git", "diff", "--quiet", BASE, "--", "Cargo.toml", "Cargo.lock", "assets"], cwd=ROOT, check=True)

    manifest_path = "tools/animgraph_authoring/source_current_rig_program_lifecycle.authoring.json"
    lifecycle = json.loads(read(manifest_path))["focused_tests"][0]["argv"]
    assert lifecycle[-5:] == ["--", "--exact", *[f"extensions::control_rig_units::tests::{name}" for name in names]]
    print(json.dumps({
        "base": BASE, "reviewed_files": len(before_paths), "remaining_extension_files": len(remaining),
        "whole_file_moves": whole_files, "rig_tests": names, "horizontal_motion_tests": horizontal_tests,
        "rig_units_production_sha256": hashlib.sha256(CHECKS["production"](current_units).encode()).hexdigest(),
        "asset_dependency_and_registration_identity": True, "changed_fixture_assertion_rejected": True,
    }, indent=2))


if __name__ == "__main__":
    main()
