import argparse
import json
import re
from pathlib import Path


CURVES = [
    "PoseStanding",
    "PoseCrouching",
    "PoseMoving",
    "RotationYawOffset",
    "FootLeftLock",
    "FootRightLock",
    "AllowTransitions",
    "RotationYawSpeed",
    "PoseGrounded",
    "FootLeftIk",
    "FootRightIk",
    "PoseInAir",
    "LayerHead",
    "LayerHeadAdditive",
    "LayerHeadSlot",
    "LayerArmLeft",
    "LayerArmLeftAdditive",
    "LayerArmLeftLocalSpace",
    "LayerArmLeftSlot",
    "LayerArmRight",
    "LayerArmRightAdditive",
    "LayerArmRightLocalSpace",
    "LayerArmRightSlot",
    "LayerHandLeft",
    "LayerHandRight",
    "LayerSpine",
    "LayerSpineAdditive",
    "LayerSpineSlot",
    "LayerPelvis",
    "LayerPelvisSlot",
    "LayerLegs",
    "LayerLegsSlot",
    "HandLeftIk",
    "HandRightIk",
    "HipsDirectionLock",
    "PoseAiming",
    "ViewBlock",
]

CONTRACTS = {
    "ALI_Overlay": "ali_overlay.animgraph.ron",
    "AB_Als": "ab_als.animgraph.ron",
    "AB_Als_Default": "overlays/default.animgraph.ron",
    "AB_Als_Barrel": "overlays/barrel.animgraph.ron",
    "AB_Als_Binoculars": "overlays/binoculars.animgraph.ron",
    "AB_Als_Bow": "overlays/bow.animgraph.ron",
    "AB_Als_Box": "overlays/box.animgraph.ron",
    "AB_Als_Feminine": "overlays/feminine.animgraph.ron",
    "AB_Als_HandsTied": "overlays/hands_tied.animgraph.ron",
    "AB_Als_Injured": "overlays/injured.animgraph.ron",
    "AB_Als_Masculine": "overlays/masculine.animgraph.ron",
    "AB_Als_PistolOneHanded": "overlays/pistol_one_handed.animgraph.ron",
    "AB_Als_PistolTwoHanded": "overlays/pistol_two_handed.animgraph.ron",
    "AB_Als_Rifle": "overlays/rifle.animgraph.ron",
    "AB_Als_Torch": "overlays/torch.animgraph.ron",
    "AB_Als_Standing": "stances/standing.animgraph.ron",
    "AB_Als_Crouching": "stances/crouching.animgraph.ron",
    "AB_Als_Grounded": "grounded.animgraph.ron",
    "AB_Als_Locomotion": "locomotion.animgraph.ron",
    "AB_Als_Layering": "layering.animgraph.ron",
    "AB_Als_View": "view.animgraph.ron",
    "AB_Als_Head": "head.animgraph.ron",
    "AB_Als_Ragdolling": "ragdolling.animgraph.ron",
}

OVERLAY_CONTRACTS = [
    "overlays/default.animgraph.ron",
    "overlays/masculine.animgraph.ron",
    "overlays/feminine.animgraph.ron",
    "overlays/injured.animgraph.ron",
    "overlays/hands_tied.animgraph.ron",
    "overlays/rifle.animgraph.ron",
    "overlays/pistol_one_handed.animgraph.ron",
    "overlays/pistol_two_handed.animgraph.ron",
    "overlays/bow.animgraph.ron",
    "overlays/torch.animgraph.ron",
    "overlays/binoculars.animgraph.ron",
    "overlays/box.animgraph.ron",
    "overlays/barrel.animgraph.ron",
]

ROOT_LINKS = [
    "graphs/ab_als/grounded.animgraph.ron",
    "graphs/ab_als/locomotion.animgraph.ron",
    "graphs/ab_als/layering.animgraph.ron",
    "graphs/ab_als/head.animgraph.ron",
    "graphs/ab_als/ragdolling.animgraph.ron",
]


def text(path):
    return path.read_text(encoding="utf-8")


def graph_contract(path):
    value = text(path)
    return (
        "output_time: true" in value
        and '"pose": Pose' in value
        and "AB_Als_Monolithic" not in value
        and "locomotion_overlay" not in value
    )


def checklist_evidence(value, identifier):
    pattern = re.compile(
        rf"^- \[x\] BF100-{identifier:02d}\b(?P<body>.*?)(?=^- \[[ x]\] BF100-|\Z)",
        re.MULTILINE | re.DOTALL,
    )
    match = pattern.search(value)
    if match is None:
        return False
    body = match.group("body")
    return "事实" in body and "Gate" in body


def scan_forbidden(root):
    offenders = []
    for relative in ("assets/graphs/ab_als", "assets/animated_scenes", "src", "examples/manual_playground.rs"):
        path = root / relative
        files = [path] if path.is_file() else path.rglob("*")
        for candidate in files:
            if not candidate.is_file() or candidate.suffix not in {".rs", ".ron"}:
                continue
            value = text(candidate)
            if any(token in value for token in ("AB_Als_Monolithic", "locomotion_overlay")):
                offenders.append(candidate.relative_to(root).as_posix())
    return sorted(set(offenders))


def report(root):
    graph_root = root / "assets/graphs/ab_als"
    checks = {}
    checks["contract_count"] = len(CONTRACTS) == 23
    checks["contract_files"] = all((graph_root / relative).is_file() for relative in CONTRACTS.values())
    checks["contract_pose_time"] = all(graph_contract(graph_root / relative) for relative in CONTRACTS.values())
    root_graph = text(graph_root / "ab_als.animgraph.ron")
    checks["root_links"] = all(link in root_graph for link in ROOT_LINKS)
    checks["root_overlay_links"] = all(f"graphs/ab_als/{relative}" in root_graph for relative in OVERLAY_CONTRACTS)
    checks["root_boundaries"] = all(
        token in root_graph
        for token in (
            "SkeletonAwareLayeredGraphNode",
            "InertializedLinkedAnimLayerNode",
            "ControlRigBoundaryNode",
            "AB_Als_Ragdolling",
        )
    )
    checks["root_curve_contract"] = all(f'"{curve}": F32' in root_graph for curve in CURVES)
    scenes = sorted((root / "assets/animated_scenes").glob("*.animscn.ron"))
    checks["scene_count"] = len(scenes) > 0
    checks["scene_root_only"] = all(
        "graphs/ab_als/ab_als.animgraph.ron" in text(scene)
        and "Monolithic" not in text(scene)
        for scene in scenes
    )
    assembly = text(root / "src/character_assembly.rs")
    manual = text(root / "examples/manual_playground.rs")
    checks["canonical_character_entry"] = (
        'ALS_LINKED_ANIMATED_SCENE: &str = "animated_scenes/als_ragdoll.animscn.ron"' in assembly
        and "attach_avatar(commands, assets, character, ALS_LINKED_ANIMATED_SCENE)" in assembly
        and "AlsPlayableCharacterPlugin" in manual
        and "spawn_als_character" in manual
    )
    offenders = scan_forbidden(root)
    checks["no_monolithic_or_legacy_root"] = not offenders
    checklist = text(root / "CHECKLIST.md")
    checks["checklist_evidence"] = all(checklist_evidence(checklist, identifier) for identifier in range(1, 25))
    audit = text(root / "docs/bf100-24-animation-audit.md")
    checks["resource_audit"] = (
        "Source sequence assets without a Bevy ClipNode reference: 0" in audit
        and "Destination TimePositionNode loop=true: 0" in audit
        and "Destination TimePositionNode loop unspecified: 0" in audit
        and "## Result: PASS" in audit
    )
    return checks, offenders, [scene.relative_to(root).as_posix() for scene in scenes]


def markdown(checks, offenders, scenes):
    lines = [
        "# BF100-25 Static Acceptance",
        "",
        "This report is generated by `tools/validate_ab_als_static.py` from the checked-in contract roots, scenes, checklist evidence, and BF100-24 audit report.",
        "",
        "| Check | Result |",
        "| --- | --- |",
    ]
    for name, value in checks.items():
        lines.append(f"| {name} | {'PASS' if value else 'FAIL'} |")
    lines.extend(["", "## Contract Roots", "", "| Contract | Bevy root |", "| --- | --- |"])
    for name, path in CONTRACTS.items():
        lines.append(f"| {name} | `assets/graphs/ab_als/{path}` |")
    lines.extend(["", "## Animated Scenes", "", "| Scene |", "| --- |"])
    for scene in scenes:
        lines.append(f"| `{scene}` |")
    lines.extend(["", "## Forbidden References", ""])
    lines.extend([f"- {entry}" for entry in offenders] or ["- None"])
    verdict = all(checks.values())
    lines.extend(["", f"## Result: {'PASS' if verdict else 'FAIL'}", ""])
    return "\n".join(lines), verdict


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--report", type=Path, default=Path("docs/bf100-25-static-acceptance.md"))
    args = parser.parse_args()
    root = args.root.resolve()
    checks, offenders, scenes = report(root)
    output, verdict = markdown(checks, offenders, scenes)
    report_path = args.report if args.report.is_absolute() else root / args.report
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(output, encoding="utf-8", newline="\n")
    print(json.dumps({"report": report_path.relative_to(root).as_posix(), "pass": verdict, "checks": checks}, indent=2))
    return 0 if verdict else 1


if __name__ == "__main__":
    raise SystemExit(main())
