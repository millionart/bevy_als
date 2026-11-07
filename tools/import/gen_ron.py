#!/usr/bin/env python3
"""M0.6 asset import tool: UE exports (glTF) -> bevy_asset loadable RON descriptors. Run from repo root: python tools/import/gen_ron.py"""
from __future__ import annotations

import json
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
ASSETS = REPO / "assets"
MODELS = ASSETS
ANIMS = ASSETS / "ALS"
SKELS = ASSETS / "ALS" / "Character"

SKELETON_MAP = {
    "ALSCamera/": "ALSCamera/SKM_Als_Camera.static.skn.ron",
    "ALS/OverlayObjects/Bow/": "ALS/OverlayObjects/Bow/SKM_Als_Bow.skn.ron",
}
DEFAULT_SKELETON = "ALS/Character/SKM_Als.skn.ron"

SKELETON_SOURCES = {
    "SKM_Als": ("ALS/Animations/Base/A_Als_Idle.gltf", "Scene0"),
}


def ron_str(s: str) -> str:
    return '"' + s + '"'


def load_gltf(path: Path) -> dict:
    try:
        with path.open(encoding="utf-8") as f:
            return json.load(f)
    except (OSError, json.JSONDecodeError) as e:
        raise ValueError(f"failed to parse JSON: {e}") from e


def common_checks(g: dict, rel: str) -> list[str]:
    errs = []
    if "scenes" not in g or len(g["scenes"]) != 1:
        errs.append(f"{rel}: expected exactly 1 scene, got {len(g.get('scenes', []))}")
    nodes = g.get("nodes", [])
    if not nodes:
        errs.append(f"{rel}: no nodes")
    root_idx = g["scenes"][0]["nodes"][0]
    if root_idx >= len(nodes):
        errs.append(f"{rel}: scene root node index out of range")
    return errs


def check_model(g: dict, rel: str) -> list[str]:
    errs = common_checks(g, rel)
    skins = g.get("skins", [])
    if not skins:
        errs.append(f"{rel}: model has no skin")
    for i, s in enumerate(skins):
        if not s.get("joints"):
            errs.append(f"{rel}: skin[{i}] no joints")
    if not g.get("meshes"):
        errs.append(f"{rel}: no mesh data")
    return errs


def check_anim(g: dict, rel: str, stem: str) -> tuple[list[str], str]:
    errs = common_checks(g, rel)
    anims = g.get("animations", [])
    if len(anims) != 1:
        errs.append(f"{rel}: expected exactly 1 animation, got {len(anims)}")
        return errs, ""
    name = anims[0].get("name") or ""
    expected = f"{stem}_0"
    if name != expected:
        errs.append(f"{rel}: animation name {name!r} != expected {expected!r}")
    n_nodes = len(g.get("nodes", []))
    samplers = anims[0].get("samplers", [])
    for ci, ch in enumerate(anims[0].get("channels", [])):
        tgt = ch.get("target", {})
        ni = tgt.get("node")
        if ni is None or not 0 <= ni < n_nodes:
            errs.append(f"{rel}: channel[{ci}] target node out of range({ni})")
        si = ch.get("sampler")
        if si is None or not 0 <= si < len(samplers):
            errs.append(f"{rel}: channel[{ci}] sampler out of range({si})")
    if not anims[0].get("channels"):
        errs.append(f"{rel}: animation has no channels")
    return errs, expected


def main() -> int:
    failed = False
    n_skn = n_anim = 0
    SKELS.mkdir(parents=True, exist_ok=True)

    for name, (rel, label) in SKELETON_SOURCES.items():
        out = SKELS / f"{name}.skn.ron"
        out.write_text(
            "(\n"
            f"    source: Gltf(source: {ron_str(rel)}, label: {ron_str(label)}),\n"
            ")\n",
            encoding="utf-8",
        )
        n_skn += 1
        print(f"[skn] {out.relative_to(REPO)} <- {rel}")
    skipped = sorted(p.stem for p in MODELS.rglob("*.gltf")
                     if p.stem not in SKELETON_SOURCES)
    if skipped:
        print(f"[skn] skipped mesh skeletons without animated counterpart: {', '.join(skipped)}")

    anim_paths = sorted(p for p in ANIMS.rglob("*.gltf"))
    for p in anim_paths:
        rel = p.relative_to(ASSETS).as_posix().replace("\\", "/")
        stem = p.stem
        g = load_gltf(p)
        if not g.get("animations"):
            continue
        errs, anim_name = check_anim(g, rel, stem)
        source_rel = rel
        runtime = ASSETS / "ALSRuntime" / p.relative_to(ANIMS)
        if runtime.is_file():
            source_rel = runtime.relative_to(ASSETS).as_posix()
            runtime_errs, _ = check_anim(load_gltf(runtime), source_rel, stem)
            errs.extend(runtime_errs)
        skeleton = next((value for prefix, value in SKELETON_MAP.items() if rel.startswith(prefix)), DEFAULT_SKELETON)
        out = p.with_suffix(".anim.ron")
        if errs:
            failed = True
            out.unlink(missing_ok=True)
            print("\n".join(errs), file=sys.stderr)
            continue
        out.write_text(
            "(\n"
            f"    source: GltfNamed(\n"
            f"        path: {ron_str(source_rel)},\n"
            f"        animation_name: {ron_str(anim_name)},\n"
            "    ),\n"
            f"    skeleton: {ron_str(skeleton)},\n"
            ")\n",
            encoding="utf-8",
        )
        n_anim += 1
    print(f"\ndone: {n_skn} skn.ron, {n_anim} anim.ron; "
          f"validation {'passed' if not failed else 'FAILED'}")

    referenced = {
        line.split("skeleton: ")[1].strip('",)')
        for p in ANIMS.rglob("*.anim.ron")
        for line in p.read_text(encoding="utf-8").splitlines()
        if "skeleton:" in line
    }
    missing = [s for s in SKELETON_SOURCES
               if f"ALS/Character/{s}.skn.ron" not in referenced]
    if missing:
        print(f"warning: skeletons referenced by no animation {missing}", file=sys.stderr)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
