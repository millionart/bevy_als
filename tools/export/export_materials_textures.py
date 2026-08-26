import json
import os

import unreal

ROOT = os.environ.get("BEVY_ALS_ROOT") or os.path.abspath(os.getcwd())
OUT_DIR = os.path.join(ROOT, "tmp", "export", "playground", "materials")
TEX_DIR = os.path.join(OUT_DIR, "textures")

TARGETS = {
    "M_Als_Tiles": "/ALS/ALSExtras/Environment/Materials/M_Als_Tiles.M_Als_Tiles",
    "M_Als_Main": "/ALS/ALS/MaterialLibrary/M_Als_Main.M_Als_Main",
}


def log(msg):
    unreal.log_warning("MATEX %s" % msg)


def expr_summary(e):
    cls = e.get_class().get_name()
    out = {"class": cls}
    fn = e.get_path_name().rsplit(".", 1)[-1]
    out["name"] = fn
    if "TextureSample" in cls or cls == "MaterialExpressionTextureSampleParameter2D":
        try:
            tex = e.get_editor_property("texture")
            if tex:
                out["texture"] = tex.get_path_name()
        except Exception:
            pass
    if "TextureSampleParameter" in cls:
        try:
            out["parameter"] = e.get_editor_property("parameter_name")
        except Exception:
            pass
    if "ScalarParameter" in cls:
        try:
            out["parameter"] = str(e.get_editor_property("parameter_name"))
            out["default"] = float(e.get_editor_property("default_value"))
        except Exception:
            pass
    if "VectorParameter" in cls:
        try:
            out["parameter"] = str(e.get_editor_property("parameter_name"))
            dv = e.get_editor_property("default_value")
            out["default"] = [dv.r, dv.g, dv.b, dv.a]
        except Exception:
            pass
    if "TextureSampleParameter" in cls:
        try:
            out["parameter"] = str(e.get_editor_property("parameter_name"))
        except Exception:
            pass
    if "Constant" == cls.split("_")[-1] or cls.endswith("MaterialExpressionConstant"):
        try:
            out["value"] = float(e.get_editor_property("r"))
        except Exception:
            pass
    return out


def walk_graph(mat):
    props = unreal.MaterialEditingLibrary.get_material_expressions(mat)
    exprs = []
    index_of = {}
    for i, e in enumerate(props):
        index_of[e.get_path_name()] = i
    outputs = {}
    for prop_name in (
        "base_color",
        "metallic",
        "specular",
        "roughness",
        "normal",
        "emissive_color",
        "opacity",
        "world_position_offset",
    ):
        try:
            out_expr = mat.get_editor_property(prop_name)
            if out_expr and hasattr(out_expr, "get_path_name"):
                pname = out_expr.get_path_name()
                if pname in index_of:
                    outputs[prop_name] = {"expr": index_of[pname], "output": "RGB"}
        except Exception:
            pass
    for i, e in enumerate(props):
        entry = expr_summary(e)
        conns = {}
        for pin in ("rgb", "rgba", "r", "g", "b", "a", "input", "a_in", "b_in", "base", "normal", "height", "uv", "x", "y", "const_a", "const_b", "position"):
            try:
                sub = e.get_editor_property(pin)
                if sub and hasattr(sub, "get_path_name") and sub.get_path_name() in index_of:
                    conns[pin] = {"expr": index_of[sub.get_path_name()],
                                  "out": getattr(sub, "output", None)}
            except Exception:
                pass
        if conns:
            entry["inputs"] = {k: v["expr"] for k, v in conns.items()}
        exprs.append(entry)
    return {"outputs": outputs, "nodes": exprs}


def export_texture(tex_obj, out_base):
    try:
        task = unreal.AssetExportTask()
        task.object = tex_obj
        exp_cls = None
        for n in dir(unreal):
            if n.lower().endswith("textureexporterpng"):
                exp_cls = getattr(unreal, n)
                break
        if exp_cls is None:
            for suffix in ("textureexportertga", "textureexporterbmp"):
                for n in dir(unreal):
                    if n.lower().endswith(suffix):
                        exp_cls = getattr(unreal, n)
                        break
                if exp_cls:
                    break
        ext = ".tga" if "tga" in str(exp_cls).lower() else ".png"
        task.exporter = exp_cls() if exp_cls else None
        task.filename = out_base + ext
        task.automated = True
        task.errors = []
        unreal.Exporter.run_asset_export_task(task)
        if os.path.isfile(out_base + ext):
            return os.path.basename(out_base + ext)
    except Exception as exc:
        log("tex export fail %s: %s" % (tex_obj.get_name(), exc))
    return None


def main():
    os.makedirs(TEX_DIR, exist_ok=True)
    result = {}
    for short, pkg in TARGETS.items():
        obj = unreal.load_object(None, pkg)
        if not obj:
            result[short] = {"error": "load fail"}
            continue
        graph = walk_graph(obj)
        textures = {}
        for node in graph["nodes"]:
            tp = node.pop("_texpath", None)
        result[short] = graph

        for i, node in enumerate(graph["nodes"]):
            if "texture" not in node:
                continue
            tex_path = node["texture"]
            if tex_path.startswith("/Engine/"):
                continue
            key = tex_path.rsplit("/", 1)[-1].split(".")[0]
            if key in textures:
                node["_file"] = textures[key]
                continue
            tex_obj = unreal.load_object(None, tex_path)
            saved = export_texture(tex_obj, os.path.join(TEX_DIR, key)) if tex_obj else None
            textures[key] = saved
            node["_file"] = saved

    params_out = {}
    for short, pkg in TARGETS.items():
        obj = unreal.load_object(None, pkg)
        if not obj:
            continue
        info = {}
        try:
            lay = unreal.MaterialEditingLibrary.get_material_property_input_node(
                obj, unreal.MaterialProperty.MP_NORMAL)
        except Exception:
            lay = None
        graph = result["materials"].get(short) if "materials" in result else None
        nodes = graph["nodes"] if graph else []
        for node in nodes:
            p = node.get("parameter")
            if not p:
                continue
            if node["class"].endswith("VectorParameter"):
                info[p] = [("vector", node.get("default"))]
            elif node["class"].endswith("ScalarParameter"):
                info[p] = [("scalar", node.get("default"))]
            elif "TextureSampleParameter" in node["class"]:
                info[p] = [("texture", node.get("texture")), ("file", node.get("_file"))]
        params_out[short] = info

    out = {"materials": result, "parameters": params_out}
    path = os.path.join(OUT_DIR, "materials.json")
    with open(path, "w", encoding="utf-8") as f:
        json.dump(out, f, indent=1)
    log("wrote %s (%d bytes)" % (path, os.path.getsize(path)))
    log("textures: %s" % sorted(os.listdir(TEX_DIR)))


main()
