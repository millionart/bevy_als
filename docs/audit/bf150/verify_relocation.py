import hashlib
import json
from pathlib import Path
import re
import subprocess


BASE = "0e5cce6dc7520218fa3c76982c3d85c9219ea078"
RENAMES = {
    "RagdollSpatialQuery": "SceneQuery",
    "RagdollShapeCollisionMode": "ShapeCollisionMode",
    "RigCookedMeshRay": "CookedMeshRayData",
    "RagdollGyroscopicTorqueDisabled": "SourceGyroscopicTorqueDisabled",
    "SourceRagdollFrameRotation": "SourceBodyFrameRotation",
    "integrate_source_ragdoll_rotation": "integrate_source_body_rotation",
    "disable_source_ragdoll_rotation": "disable_source_body_rotation",
    "disable_ragdoll_gyroscopic_torque": "disable_source_gyroscopic_torque",
    "prepare_profile_sleep_islands": "prepare_source_sleep_islands",
    "update_profile_sleep": "update_source_sleep",
    "initialize_profile_dof": "initialize_source_body_dof",
    "condition_profile_solver_inertia": "condition_source_solver_inertia",
    "solve_profile_dof": "solve_source_body_dof",
}
PATHS = {
    "extensions::control_rig_query::": "control_rig_query::",
    "extensions::ragdoll_physics_profile::apply_source_linear_damping": "extensions::physics_asset::apply_source_linear_damping",
    "extensions::ragdoll_physics_profile::integrate_source_ragdoll_rotation": "extensions::source_body_dynamics::integrate_source_ragdoll_rotation",
    "extensions::ragdoll_physics_profile::capture_source_sleep_wakes": "extensions::source_body_dynamics::capture_source_sleep_wakes",
}
FUNCTION = re.compile(r"(?m)^( *)(?:pub(?:\([^)]*\))? )?fn ([a-zA-Z0-9_]+)\(")
TEST = re.compile(r"(?m)^\s*#\[test\]\n(?:\s*#\[[^\n]*\]\n)*\s*fn (\w+)\(")


def original(path):
    return subprocess.check_output(["git", "show", f"{BASE}:{path}"], text=True)


def production(code):
    return code.split("#[cfg(test)]\nmod tests {", 1)[0]


def implementations(code):
    found = {}
    for match in FUNCTION.finditer(production(code)):
        indent, name = match.groups()
        end = code.index("\n" + indent + "}", match.end())
        assert name not in found, name
        found[name] = code[match.start():end + len(indent) + 2]
    return found


def canonical(code):
    for old, new in PATHS.items():
        code = code.replace(old, new)
    code = re.sub(r"\b\w+\b", lambda match: RENAMES.get(match[0], match[0]), code)
    code = code.replace("spatial: SpatialQuery,", "spatial: SceneQuery,")
    code = re.sub(r"(?m)^ *(?:pub(?:\([^)]*\))? )(?=fn )", "", code)
    return re.sub(r",(?=[)\]])", "", re.sub(r"\s+", "", code))


def main():
    moves = {
        "src/avian_ext/control_rig_query.rs": [
            "src/als/character/control_rig_query.rs",
            "src/avian_ext/shape_raycast.rs",
        ],
        "src/avian_ext/control_rig_mesh_ray.rs": ["src/avian_ext/cooked_mesh_raycast.rs"],
        "src/avian_ext/ragdoll_physics_profile.rs": [
            "src/animation_graph_ext/physics_asset.rs",
            "src/avian_ext/scene_query.rs",
            "src/avian_ext/source_body_dynamics.rs",
        ],
        "src/avian_ext/ragdoll_angular_drive.rs": ["src/animation_graph_ext/ragdoll_angular_drive.rs"],
        "src/avian_ext/penetration_recovery.rs": ["src/als_camera/penetration_recovery.rs"],
    }
    checks = {}
    for old, destinations in moves.items():
        before = original(old)
        after = [Path(path).read_text() for path in destinations]
        previous = {RENAMES.get(name, name): canonical(body) for name, body in implementations(before).items()}
        relocated = {name: canonical(body) for text in after for name, body in implementations(text).items()}
        changed = [name for name in previous if previous[name] != relocated.get(name)]
        assert not changed, (old, changed)
        prior_tests = sorted(TEST.findall(before))
        assert prior_tests == sorted(name for text in after for name in TEST.findall(text)), old
        checks[old] = {"functions": len(previous), "tests": len(prior_tests), "destinations": destinations}
    units = Path("src/control_rig_ext/control_rig_units.rs").read_text()
    assert units == original("src/control_rig/control_rig_units.rs")
    timing = "src/als/character/movement/source_physics_timing.rs"
    assert canonical(implementations(original(timing))["build"]) == canonical(
        implementations(Path(timing).read_text())["build"]
    )
    subprocess.run(["git", "diff", "--quiet", BASE, "--", "Cargo.toml", "Cargo.lock", "assets"], check=True)
    assert not Path("src/control_rig").exists()
    for path in Path("src/avian_ext").glob("*.rs"):
        code = production(path.read_text())
        assert not re.search(r"\b(AnimationGraphPlayer|SpawnedRagdoll|RigQueryContext|RagdollSpatialQuery)\b", code), path
    result = {"base": BASE, "relocations": checks, "control_rig_unit_sha256": hashlib.sha256(units.encode()).hexdigest(),
              "physics_timing_registration_equal": True, "assets_and_dependency_locks_equal": True}
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
