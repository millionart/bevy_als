#!/usr/bin/env python3
"""M4.1 bone map generator: parse SKM_Als.gltf skin[0].joints, dump bone list JSON to tmp/, validate ALS key bones and virtual bones presence. Run from repo root: python tools/import/gen_bone_map.py"""
from __future__ import annotations

import json
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
MODEL = REPO / "assets/ALS/Character/SKM_Als.gltf"
OUT_DIR = REPO / "tmp"
OUT_FILE = OUT_DIR / "bone-map.json"

EXPECTED_JOINT_COUNT = 79

ALS_KEY_BONES = [
    "root",
    "pelvis",
    "spine_01",
    "spine_02",
    "spine_03",
    "neck_01",
    "head",
    "clavicle_l",
    "upperarm_l",
    "lowerarm_l",
    "hand_l",
    "clavicle_r",
    "upperarm_r",
    "lowerarm_r",
    "hand_r",
    "thigh_l",
    "calf_l",
    "foot_l",
    "ball_l",
    "thigh_r",
    "calf_r",
    "foot_r",
    "ball_r",
    "ik_foot_root",
    "ik_foot_l",
    "ik_foot_r",
    "ik_hand_root",
    "ik_hand_gun",
    "ik_hand_l",
    "ik_hand_r",
]

VIRTUAL_BONES = [
    "VB foot_root",
    "VB foot_l",
    "VB foot_r",
    "VB hand_l_to_hand_r",
    "VB hand_r",
    "VB hand_l_to_ik_hand_gun",
    "VB ik_hand_r",
    "VB hand_r_to_hand_r",
    "VB hand_l",
    "VB hand_r_to_ik_hand_gun",
    "VB ik_hand_l",
]


def load_joints(path: Path) -> list[str]:
    with path.open(encoding="utf-8") as f:
        g = json.load(f)
    skins = g.get("skins", [])
    if not skins:
        raise ValueError(f"{path.name}: no skin")
    joints = skins[0].get("joints", [])
    nodes = g.get("nodes", [])
    names = []
    for j in joints:
        if not 0 <= j < len(nodes):
            raise ValueError(f"{path.name}: joint index {j} out of range")
        names.append(nodes[j].get("name") or f"node_{j}")
    return names


def main() -> int:
    bones = load_joints(MODEL)
    missing_key = [b for b in ALS_KEY_BONES if b not in bones]
    missing_vb = [b for b in VIRTUAL_BONES if b not in bones]
    duplicates = sorted({b for b in bones if bones.count(b) > 1})

    report = {
        "source": str(MODEL.relative_to(REPO)).replace("\\", "/"),
        "joint_count": len(bones),
        "expected_joint_count": EXPECTED_JOINT_COUNT,
        "bones": bones,
        "key_bones": ALS_KEY_BONES,
        "virtual_bones": VIRTUAL_BONES,
        "missing_key_bones": missing_key,
        "missing_virtual_bones": missing_vb,
        "duplicate_bones": duplicates,
    }

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    OUT_FILE.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")

    print(f"[bone-map] {report['source']}: {len(bones)} joints")
    for i, name in enumerate(bones):
        print(f"  {i:>3} {name}")
    print(f"[bone-map] key bones checked: {len(ALS_KEY_BONES)}, missing: {missing_key or 'none'}")
    print(f"[bone-map] virtual bones checked: {len(VIRTUAL_BONES)}, missing: {missing_vb or 'none'}")
    print(f"[bone-map] duplicates: {duplicates or 'none'}")
    print(f"[bone-map] written {OUT_FILE.relative_to(REPO)}")

    failed = bool(missing_key or missing_vb or duplicates or len(bones) != EXPECTED_JOINT_COUNT)
    print(f"[bone-map] validation {'passed' if not failed else 'FAILED'}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
