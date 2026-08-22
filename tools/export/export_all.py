import os
import json

import unreal

_REPO = os.environ.get("BEVY_ALS_ROOT") or os.path.abspath(os.getcwd())
OUT_ROOT = os.path.join(_REPO, "tmp", "export", "staging")
LOG_PATH = os.path.join(_REPO, "tmp", "export", "export_log.json")


def log(msg):
    unreal.log_warning("M0EXPORT %s" % msg)


def resolve_type(suffix):
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
        return sorted(str(a.package_name) for a in assets
                      if str(a.package_name).startswith("/ALS/"))

    meshes = packages_by_class("SkeletalMesh")
    anims = packages_by_class("AnimSequence")
    blends = sorted(set(packages_by_class("BlendSpace"))
                    | set(packages_by_class("BlendSpace1D")))
    curves = packages_by_class("CurveFloat")
    log("counts mesh=%d anim=%d blend=%d curve=%d"
        % (len(meshes), len(anims), len(blends), len(curves)))

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
            unreal.Exporter.run_asset_export_task(task)
        except Exception as e:
            results.append({"asset": str(obj.get_path_name()), "out": out_file,
                            "ok": False, "err": str(e)})
            return
        ok = os.path.isfile(out_file) and os.path.getsize(out_file) > 0
        results.append({"asset": str(obj.get_path_name()), "out": out_file,
                        "ok": ok,
                        "size": os.path.getsize(out_file) if ok else 0})

    def load(pkg):
        return unreal.load_object(None, pkg + "." + pkg.rsplit("/", 1)[-1])

    for pkg in meshes:
        obj = load(pkg)
        if not obj:
            results.append({"asset": pkg, "ok": False, "err": "load fail"})
            continue
        name = pkg.rsplit("/", 1)[-1]
        group = "ALSCamera" if "/ALSCamera/" in pkg else "Character"
        run_export(obj, "%s/models/%s/%s.gltf" % (OUT_ROOT, group, name), sm_exp_cls)

    for pkg in anims:
        obj = load(pkg)
        if not obj:
            results.append({"asset": pkg, "ok": False, "err": "load fail"})
            continue
        marker = "/ALS/ALS/Animations/"
        if marker in pkg:
            rel = pkg.split(marker, 1)[-1]
        else:
            rel = pkg.replace("/ALS/", "").replace("/", "_")
        run_export(obj, "%s/animations/%s.gltf" % (OUT_ROOT, rel), anim_exp_cls)

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
            except Exception as e:
                entry["axes_err"] = str(e)
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
        results.append({"asset": pkg, "out": out, "ok": True, "kind": "blendspace",
                        "n_samples": len(samples)})

    for pkg in curves:
        obj = load(pkg)
        if not obj:
            continue
        keys_out = []
        errs = []
        try:
            t_min, t_max = obj.get_time_range()
            n = 128
            for i in range(n + 1):
                t = t_min + (t_max - t_min) * i / n
                keys_out.append({"t": float(t), "v": float(obj.get_float_value(t))})
        except Exception as e:
            errs.append(str(e))
        rel = pkg.replace("/ALS/", "").replace(".", "_")
        out = "%s/curves/%s.json" % (OUT_ROOT, rel)
        ensure_dir(os.path.dirname(out))
        with open(out, "w", encoding="utf-8") as f:
            json.dump({"asset": pkg,
                       "sampling": {"mode": "uniform", "n": len(keys_out),
                                    "t_min": keys_out[0]["t"] if keys_out else None,
                                    "t_max": keys_out[-1]["t"] if keys_out else None},
                       "keys": keys_out, "errors": errs}, f, indent=1)
        results.append({"asset": pkg, "out": out, "ok": not errs, "kind": "curve",
                        "n_keys": len(keys_out), "err": "; ".join(errs)})

    ok_n = sum(1 for r in results if r.get("ok"))
    with open(LOG_PATH, "w", encoding="utf-8") as f:
        json.dump(results, f, indent=1)
    log("DONE total=%d ok=%d fail=%d log=%s"
        % (len(results), ok_n, len(results) - ok_n, LOG_PATH))
    for r in results:
        if not r.get("ok"):
            log("FAIL %s -> %s : %s" % (r.get("asset"), r.get("out"), r.get("err")))


main()
