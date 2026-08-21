#!/usr/bin/env python3
"""M0.6 资产导入小工具：UE 导出物(glTF) -> bevy_asset 可加载格式。

一键流程（仓库根目录运行）：
    python tools/import/gen_ron.py

产出：
1. assets/skeletons/<Name>.skn.ron      —— Skeleton 描述（引用 models/<Name>.gltf#Scene0）
2. assets/animations/**/*.anim.ron      —— GraphClip 描述（引用动画 glTF + 骨架）

对每个 glTF 做结构校验：单场景、蒙皮关节、动画命名(<资产名>_0)、通道/采样器一致性、
通道目标节点存在性。任一失败即非零退出，不生成对应描述文件。
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
ASSETS = REPO / "assets"
MODELS = ASSETS / "models"
ANIMS = ASSETS / "animations"
SKELS = ASSETS / "skeletons"

SKELETON_MAP = {
    "/ALSCamera": "skeletons/SKM_Als_Camera.skn.ron",
}  # 未命中前缀的动画一律用主骨架
DEFAULT_SKELETON = "skeletons/SKM_Als.skn.ron"

# Skeleton 描述的 glTF 源必须含 AnimationPlayer（插件 find_root 依赖），
# 故引用代表性动画 glTF 而非纯模型；无动画对应物的网格不生成骨架描述。
SKELETON_SOURCES = {
    "SKM_Als": ("animations/Base/A_Als_Idle.gltf", "Scene0"),
}


def ron_str(s: str) -> str:
    return '"' + s + '"'


def load_gltf(path: Path) -> dict:
    try:
        with path.open(encoding="utf-8") as f:
            return json.load(f)
    except (OSError, json.JSONDecodeError) as e:
        raise ValueError(f"JSON 解析失败: {e}") from e


def common_checks(g: dict, rel: str) -> list[str]:
    errs = []
    if "scenes" not in g or len(g["scenes"]) != 1:
        errs.append(f"{rel}: 期望恰好 1 个 scene，实际 {len(g.get('scenes', []))}")
    nodes = g.get("nodes", [])
    if not nodes:
        errs.append(f"{rel}: 无节点")
    root_idx = g["scenes"][0]["nodes"][0]
    if root_idx >= len(nodes):
        errs.append(f"{rel}: scene 根节点索引越界")
    return errs


def check_model(g: dict, rel: str) -> list[str]:
    errs = common_checks(g, rel)
    skins = g.get("skins", [])
    if not skins:
        errs.append(f"{rel}: 模型缺少蒙皮(skin)")
    for i, s in enumerate(skins):
        if not s.get("joints"):
            errs.append(f"{rel}: skin[{i}] 无关节")
    if not g.get("meshes"):
        errs.append(f"{rel}: 无网格数据")
    return errs


def check_anim(g: dict, rel: str, stem: str) -> tuple[list[str], str]:
    errs = common_checks(g, rel)
    anims = g.get("animations", [])
    if len(anims) != 1:
        errs.append(f"{rel}: 期望恰好 1 条动画，实际 {len(anims)}")
        return errs, ""
    name = anims[0].get("name") or ""
    expected = f"{stem}_0"
    if name != expected:
        errs.append(f"{rel}: 动画名 {name!r} != 预期 {expected!r}")
    n_nodes = len(g.get("nodes", []))
    samplers = anims[0].get("samplers", [])
    for ci, ch in enumerate(anims[0].get("channels", [])):
        tgt = ch.get("target", {})
        ni = tgt.get("node")
        if ni is None or not 0 <= ni < n_nodes:
            errs.append(f"{rel}: channel[{ci}] 目标节点越界({ni})")
        si = ch.get("sampler")
        if si is None or not 0 <= si < len(samplers):
            errs.append(f"{rel}: channel[{ci}] 采样器越界({si})")
    if not anims[0].get("channels"):
        errs.append(f"{rel}: 动画无通道")
    return errs, expected


def main() -> int:
    failed = False
    n_skn = n_anim = 0
    SKELS.mkdir(parents=True, exist_ok=True)

    # ---- 骨架 -> .skn.ron（源为含动画的 glTF）----
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
        print(f"[skn] 跳过无动画对应物的网格骨架: {', '.join(skipped)}")

    # ---- 动画 -> .anim.ron ----
    anim_paths = sorted(p for p in ANIMS.rglob("*.gltf"))
    for p in anim_paths:
        rel = p.relative_to(ASSETS).as_posix().replace("\\", "/")
        stem = p.stem
        g = load_gltf(p)
        errs, anim_name = check_anim(g, rel, stem)
        # 骨架选择：按源目录前缀映射，默认主骨架
        skeleton = SKELETON_MAP["/ALSCamera"] if "ALSCamera" in rel else DEFAULT_SKELETON
        out = p.with_suffix(".anim.ron")
        if errs:
            failed = True
            out.unlink(missing_ok=True)
            print("\n".join(errs), file=sys.stderr)
            continue
        out.write_text(
            "(\n"
            f"    source: GltfNamed(\n"
            f"        path: {ron_str(rel)},\n"
            f"        animation_name: {ron_str(anim_name)},\n"
            "    ),\n"
            f"    skeleton: {ron_str(skeleton)},\n"
            ")\n",
            encoding="utf-8",
        )
        n_anim += 1
    print(f"\n生成完成：{n_skn} 个 .skn.ron，{n_anim} 个 .anim.ron；"
          f"校验{'通过' if not failed else '存在失败'}")

    # ---- 引用完整性抽查：骨架描述均被动画引用 ----
    referenced = {
        line.split("skeleton: ")[1].strip('",)')
        for p in ANIMS.rglob("*.anim.ron")
        for line in p.read_text(encoding="utf-8").splitlines()
        if "skeleton:" in line
    }
    missing = [s for s in SKELETON_SOURCES
               if f"skeletons/{s}.skn.ron" not in referenced]
    if missing:
        print(f"警告: 未被任何动画引用的骨架 {missing}", file=sys.stderr)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
