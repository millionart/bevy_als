# M0.5 一次性批量导出（headless commandlet，pythonscript）
# 用法: powershell -File tools/export/run_cmdlet.ps1 tools/export/export_all.py
# 仓库根经 BEVY_ALS_ROOT 环境变量或启动器工作目录注入，禁止硬编码本地路径
import os
import json

import unreal

_REPO = os.environ.get("BEVY_ALS_ROOT") or os.path.abspath(os.getcwd())
OUT_ROOT = os.path.join(_REPO, "tmp", "export", "staging")
LOG_PATH = os.path.join(_REPO, "tmp", "export", "export_log.json")


def log(msg):
    unreal.log_warning("M0EXPORT %s" % msg)


def resolve_type(suffix):
    """按后缀在 unreal 模块里大小写不敏感地找类型，规避命名差异。"""
    target = suffix.lower()
    for n in dir(unreal):
        if n.lower().endswith(target):
            return getattr(unreal, n)
    raise RuntimeError("type not found: %s" % suffix)


def ensure_dir(path):
    os.makedirs(path, exist_ok=True)
    return path


def main():
    ar = unreal.AssetRegistryHelpers.get_asset_registry()
    ar.wait_for_completion()
    try:
        ar.scan_paths_synchronous(["/ALS/"], True)
    except Exception as e:
        log("scan exc=%s" % e)

    def packages_by_class(cls_name):
        try:
            tp = unreal.TopLevelAssetPath("/Script/Engine", cls_name)
            assets = ar.get_assets_by_class(tp, True)
        except Exception as e:
            log("enum %s exc=%s" % (cls_name, e))
            return []
        return sorted(str(a.package_name) for a in assets)

    meshes = packages_by_class("SkeletalMesh")
    skeletons = packages_by_class("Skeleton")
    anims = packages_by_class("AnimSequence")
    blends = [p for p in packages_by_class("BlendSpace") + packages_by_class("BlendSpace1D")]
    curves = packages_by_class("CurveFloat")
    log("counts mesh=%d skel=%d anim=%d blend=%d curve=%d"
        % (len(meshes), len(skeletons), len(anims), len(blends), len(curves)))

    # ---- 导出选项：占位数据优先，跳过材质烘焙纹理 ----
    opts_cls = resolve_type("glTFExportOptions")
    opts = opts_cls()
    try:
        tex_fmt_enum = None
        for n in dir(unreal):
            if n.lower().endswith("gltftextureimageformat"):
                tex_fmt_enum = getattr(unreal, n)
                break
        if tex_fmt_enum is not None:
            none_val = [v for v in dir(tex_fmt_enum) if v.lower() == "none"]
            if none_val:
                opts.texture_image_format = getattr(tex_fmt_enum, none_val[0])
    except Exception as e:
        log("texfmt exc=%s" % e)

    sm_exp_cls = resolve_type("glTFSkeletalMeshExporter")
    anim_exp_cls = resolve_type("glTFAnimSequenceExporter")

    results = []

    def run_export(obj, out_file, exporter_cls):
        ensure_dir(os.path.dirname(out_file))
        task = unreal.AssetExportTask()
        task.object = obj
        task.exporter = exporter_cls()
        task.filename = out_file
        task.options = opts
        task.automated = True
        task.errors = []
        try:
            unreal.ExporterLibrary.run_asset_export_task(task)
        except Exception as e:
            results.append({"asset": str(obj.get_path_name()), "out": out_file,
                            "ok": False, "err": str(e)})
            return
        ok = os.path.isfile(out_file) and os.path.getsize(out_file) > 0
        results.append({"asset": str(obj.get_path_name()), "out": out_file,
                        "ok": ok,
                        "size": os.path.getsize(out_file) if ok else 0})

    def load(pkg):
        obj = unreal.load_object(None, pkg + "." + pkg.rsplit("/", 1)[-1])
        return obj

    # ---- 模型（含骨架蒙皮）----
    for pkg in meshes:
        obj = load(pkg)
        if not obj:
            results.append({"asset": pkg, "ok": False, "err": "load fail"})
            continue
        name = pkg.rsplit("/", 1)[-1]
        group = "ALSCamera" if "/ALSCamera/" in pkg else "Character"
        run_export(obj, "%s/models/%s/%s.gltf" % (OUT_ROOT, group, name), sm_exp_cls)

    # ---- 动画序列（镜像 ALS 目录结构）----
    for pkg in anims:
        obj = load(pkg)
        if not obj:
            results.append({"asset": pkg, "ok": False, "err": "load fail"})
            continue
        rel = pkg.split("/ALS/ALS/Animations/", 1)[-1]
        out = "%s/animations/%s.gltf" % (OUT_ROOT, rel)
        run_export(obj, out, anim_exp_cls)

    # ---- BlendSpace 定义 JSON（glTF 无此概念）----
    for pkg in blends:
        obj = load(pkg)
        if not obj:
            continue
        entry = {"asset": pkg}
        for prop in ("blend_parameters", "interpolation_param"):
            try:
                params = obj.get_editor_property(prop)
                axes = []
                for p in params:
                    axes.append({
                        "name": str(p.get_editor_property("display_name")),
                        "min": float(p.get_editor_property("min")),
                        "max": float(p.get_editor_property("max")),
                        "grid_num": int(p.get_editor_property("grid_num")),
                    })
                entry["axes"] = axes
                break
            except Exception:
                continue
        samples = []
        try:
            for s in obj.get_editor_property("sample_data"):
                anim = s.get_editor_property("animation")
                sv = s.get_editor_property("sample_value")
                samples.append({
                    "animation": str(anim.get_path_name()) if anim else None,
                    "value": [float(sv.x), float(sv.y), float(sv.z)],
                    "rate_scale": float(s.get_editor_property("rate_scale")),
                })
        except Exception as e:
            entry["samples_err"] = str(e)
        entry["samples"] = samples
        rel = pkg.replace("/ALS/", "").replace(".", "_")
        out = "%s/blendspaces/%s.json" % (OUT_ROOT, rel)
        ensure_dir(os.path.dirname(out))
        with open(out, "w", encoding="utf-8") as f:
            json.dump(entry, f, indent=1)
        results.append({"asset": pkg, "out": out, "ok": True, "kind": "blendspace"})

    # ---- 曲线数值表 JSON ----
    for pkg in curves:
        obj = load(pkg)
        if not obj:
            continue
        keys_out = []
        err = None
        for prop in ("editor_curve_data", "float_curve"):
            try:
                rc = obj.get_editor_property(prop)
                for k in rc.keys:
                    interp = k.interp_mode
                    try:
                        interp = int(interp)
                    except Exception:
                        interp = str(interp)
                    keys_out.append({
                        "t": float(k.input_value),
                        "v": float(k.output_value),
                        "interp": interp,
                    })
                break
            except Exception as e:
                err = str(e)
        rel = pkg.replace("/ALS/", "").replace(".", "_")
        out = "%s/curves/%s.json" % (OUT_ROOT, rel)
        ensure_dir(os.path.dirname(out))
        with open(out, "w", encoding="utf-8") as f:
            json.dump({"asset": pkg, "keys": keys_out, "err": err}, f, indent=1)
        results.append({"asset": pkg, "out": out, "ok": not err, "kind": "curve",
                        "n_keys": len(keys_out)})

    # ---- 骨架骨骼层级 JSON（供 M4.1 映射表用）----
    for pkg in skeletons:
        obj = load(pkg)
        if not obj:
            continue
        bones = []
        try:
            tree = obj.get_editor_property("bone_tree")
            for i, node in enumerate(tree):
                pname = ""
                pidx = int(node.get_editor_property("parent_index"))
                if pidx >= 0:
                    pname = str(tree[pidx].get_editor_property("name"))
                bones.append({
                    "index": i,
                    "name": str(node.get_editor_property("name")),
                    "parent_index": pidx,
                    "parent": pname,
                })
        except Exception as e:
            results.append({"asset": pkg, "ok": False, "err": "bones: %s" % e})
            continue
        rel = pkg.replace("/ALS/", "").replace(".", "_")
        out = "%s/skeletons/%s_bones.json" % (OUT_ROOT, rel)
        ensure_dir(os.path.dirname(out))
        with open(out, "w", encoding="utf-8") as f:
            json.dump({"asset": pkg, "bone_count": len(bones), "bones": bones},
                      f, indent=1)
        results.append({"asset": pkg, "out": out, "ok": True, "kind": "skeleton",
                        "n_bones": len(bones)})

    # ---- 汇总 ----
    ok_n = sum(1 for r in results if r.get("ok"))
    with open(LOG_PATH, "w", encoding="utf-8") as f:
        json.dump(results, f, indent=1)
    log("DONE total=%d ok=%d fail=%d log=%s"
        % (len(results), ok_n, len(results) - ok_n, LOG_PATH))
    for r in results:
        if not r.get("ok"):
            log("FAIL %s -> %s : %s" % (r.get("asset"), r.get("out"), r.get("err")))


main()
