import json
import os

import unreal

_REPO = os.environ.get("BEVY_ALS_ROOT") or os.path.abspath(os.getcwd())
JSON_IN = os.path.join(_REPO, "tmp", "export", "playground", "playground_actors.json")
OUT_DIR = os.path.join(_REPO, "tmp", "export", "playground", "meshes")
LOG_PATH = os.path.join(_REPO, "tmp", "export", "playground", "mesh_export_log.json")


def log(msg):
    unreal.log_warning("PGEXPORT %s" % msg)


def resolve_type(suffix):
    target = suffix.lower()
    for n in dir(unreal):
        if n.lower().endswith(target):
            return getattr(unreal, n)
    raise RuntimeError("type not found: %s" % suffix)


def ensure_dir(path):
    os.makedirs(path, exist_ok=True)
    return path


def collect_unique_mesh_paths():
    with open(JSON_IN, encoding="utf-8") as f:
        rows = json.load(f)
    found = {}
    for r in rows:
        candidates = []
        if r.get("mesh"):
            candidates.append(r["mesh"])
        for c in r.get("components", []):
            if c.get("mesh"):
                candidates.append(c["mesh"])
        for path in candidates:
            if "/Engine/" in path:
                continue
            if ".SM_" not in path:
                continue
            base = path.rsplit(".", 1)[0].rsplit("/", 1)[-1]
            if base.startswith("SM_Als_"):
                found.setdefault(base, path)
    return found


def main():
    ensure_dir(OUT_DIR)
    meshes = collect_unique_mesh_paths()
    log("unique SM_Als meshes=%d" % len(meshes))

    ar = unreal.AssetRegistryHelpers.get_asset_registry()
    try:
        ar.wait_for_completion()
        ar.scan_paths_synchronous(["/ALS/"], True)
    except Exception as e:
        log("registry exc=%s" % e)

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

    sm_exp_cls = resolve_type("glTFStaticMeshExporter")

    def load(pkg):
        return unreal.load_object(None, pkg + "." + pkg.rsplit("/", 1)[-1])

    results = []
    for name in sorted(meshes):
        pkg = meshes[name]
        obj = load(pkg)
        out_file = os.path.join(OUT_DIR, name + ".gltf")
        if not obj:
            results.append({"asset": pkg, "out": out_file, "ok": False,
                            "err": "load fail"})
            log("FAIL load %s" % pkg)
            continue
        task = unreal.AssetExportTask()
        task.object = obj
        task.exporter = sm_exp_cls()
        task.filename = out_file
        task.options = opts
        task.automated = True
        task.errors = []
        try:
            unreal.Exporter.run_asset_export_task(task)
        except Exception as e:
            results.append({"asset": pkg, "out": out_file, "ok": False,
                            "err": str(e)})
            log("FAIL export %s : %s" % (pkg, e))
            continue
        ok = os.path.isfile(out_file) and os.path.getsize(out_file) > 0
        size = os.path.getsize(out_file) if ok else 0
        results.append({"asset": pkg, "out": out_file, "ok": ok,
                        "size": size})
        log("%s %s size=%d" % ("OK  " if ok else "FAIL", name, size))

    ok_n = sum(1 for r in results if r.get("ok"))
    with open(LOG_PATH, "w", encoding="utf-8") as f:
        json.dump(results, f, indent=1)
    log("DONE total=%d ok=%d fail=%d -> %s"
        % (len(results), ok_n, len(results) - ok_n, OUT_DIR))
    for r in results:
        if not r.get("ok"):
            log("FAILSUMMARY %s : %s" % (r.get("asset"), r.get("err")))


main()
