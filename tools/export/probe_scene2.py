import json
import math
import os

import unreal

OUT = os.path.join(
    os.environ.get("BEVY_ALS_ROOT", os.getcwd()), "tmp", "export", "playground"
)
LEVEL_PATH = "/ALS/ALSExtras/Levels/L_Als_Playground"
SKEL_MESH_PATH = "/ALS/ALSExtras/Characters/SKM_Als.SKM_Als"

os.makedirs(OUT, exist_ok=True)


def log(msg):
    unreal.log_warning("SCENE2 %s" % msg)


def v3(v):
    return {"x": round(v.x, 3), "y": round(v.y, 3), "z": round(v.z, 3)}


def rot_of(t):
    r = t.get_actor_rotation() if hasattr(t, "get_actor_rotation") else t.relative_rotation
    return {
        "pitch": round(r.pitch, 2),
        "yaw": round(r.yaw, 2),
        "roll": round(r.roll, 2),
    }


def try_call(obj, names, *args):
    for n in names:
        if hasattr(obj, n):
            try:
                fn = getattr(obj, n)
                return ("ok", fn(*args) if args else fn())
            except Exception as e:
                return ("err:" + str(e)[:80], None)
    return ("missing", None)


def material_info(mat):
    info = {"path": mat.get_path_name()}
    mic = unreal.MaterialInstanceConstant.cast(mat)
    if mic is None:
        info["kind"] = type(mat).__name__
        return info
    info["kind"] = "MaterialInstanceConstant"
    info["parent"] = mic.get_editor_property("parent").get_path_name()
    scalars = {}
    vectors = {}
    textures = {}
    mel = unreal.MaterialEditingLibrary
    for name in mel.get_scalar_parameter_names(mic):
        scalars[name] = round(mel.get_scalar_parameter_value(mic, name), 4)
    for name in mel.get_vector_parameter_names(mic):
        v = mel.get_vector_parameter_value(mic, name)
        vectors[name] = [round(v.r, 4), round(v.g, 4), round(v.b, 4), round(v.a, 4)]
    for name in mel.get_texture_parameter_names(mic):
        t = mel.get_texture_parameter_value(mic, name)
        textures[name] = t.get_path_name() if t else None
    info["scalars"] = scalars
    info["vectors"] = vectors
    info["textures"] = textures
    return info


def component_materials(comp):
    out = []
    n = comp.get_num_materials()
    for i in range(n):
        m = comp.get_material(i)
        out.append(material_info(m) if m else None)
    return out


def dump_light(a):
    d = {
        "class": a.get_class().get_name(),
        "name": a.get_name(),
        "location": v3(a.get_actor_location()),
        "rotation": rot_of(a),
    }
    comp = a.get_component_by_class(unreal.DirectionalLightComponent)
    if comp:
        d["intensity"] = comp.get_editor_property("intensity")
        d["light_color"] = str(comp.get_editor_property("light_color"))
        d["use_temperature"] = comp.get_editor_property("b_use_temperature")
        d["temperature"] = comp.get_editor_property("temperature")
        d["atmosphere_sun_light"] = comp.get_editor_property("b_atmosphere_sun_light")
        d["cast_shadows"] = comp.get_editor_property("cast_shadows")
    return d


def main():
    log("loading map")
    ok = unreal.EditorLoadingAndSavingUtils.load_map(LEVEL_PATH)
    if not ok:
        sys_exit("failed to load map")

    world_obj = unreal.get_editor_subsystem(unreal.UnrealEditorSubsystem).get_game_world()

    actors = unreal.GameplayStatics.get_all_actors_of_class(world_obj, unreal.Actor)

    result = {
        "landscape": [],
        "text_render": [],
        "moving_objects": [],
        "rotating_objects": [],
        "lights": [],
        "materials_seen": {},
        "textures_seen": {},
        "character": {},
        "physics_asset": {},
    }

    mat_cache = {}

    def cache_mat(path):
        if path not in result["materials_seen"]:
            result["materials_seen"][path] = mat_cache[path]

    for a in actors:
        cls = a.get_class().get_name()

        if cls in ("LandscapeStreamingProxy", "LandscapeProxy") or "Landscape" in cls:
            entry = {
                "class": cls,
                "name": a.get_name(),
                "location": v3(a.get_actor_location()),
                "rotation": rot_of(a),
            }
            comps = a.get_components_by_class(unreal.LandscapeHeightfieldCollisionComponent)
            comps_render = a.get_components_by_class(unreal.LandscapeComponent)
            entry["collision_components"] = []
            for c in comps:
                e = {}
                st, size = try_call(c, ["get_component_screen_size"])
                e["screen_size_probe"] = st
                st2, res = try_call(c, ["get_collision_quads"])
                e["collision_quads_probe"] = st2
                for attr in ("component_name",):
                    if hasattr(c, "get_editor_property"):
                        try:
                            e[attr] = c.get_editor_property(attr)
                        except Exception:
                            pass
                b = c.bounds
                e["bounds_origin"] = v3(b.origin)
                e["bounds_extent"] = v3(b.box_extent)
                entry["collision_components"].append(e)
            entry["render_components"] = len(comps_render)
            hm_probe = None
            if comps_render:
                hm = comps_render[0].get_edittool_render_data
                hm_probe = "probe"
            entry["heightmap_api_probe"] = {
                "has_get_heightmap_texture": hasattr(comps_render[0], "get_heightmap_texture")
                if comps_render
                else False,
                "methods_on_component": [
                    m for m in dir(comps_render[0]) if "height" in m.lower()
                ]
                if comps_render
                else [],
            }
            result["landscape"].append(entry)
            continue

        if cls == "TextRenderActor":
            comp = a.get_component_by_class(unreal.TextRenderComponent)
            e = {
                "name": a.get_name(),
                "location": v3(a.get_actor_location()),
                "rotation": rot_of(a),
            }
            if comp:
                e["text"] = comp.get_editor_property("text")
                col = comp.get_editor_property("text_color")
                e["color"] = [col.r, col.g, col.b, col.a]
                e["world_size"] = comp.get_editor_property("world_size")
                e["horizontal_alignment"] = str(
                    comp.get_editor_property("horizontal_alignment")
                )
                e["vertical_alignment"] = str(comp.get_editor_property("vertical_alignment"))
            result["text_render"].append(e)
            continue

        if cls == "B_Als_MovingObject_C":
            e = {
                "name": a.get_name(),
                "location": v3(a.get_actor_location()),
                "rotation": rot_of(a),
                "properties": {},
                "components": [],
            }
            for c in a.get_components_by_class(unreal.SceneComponent):
                e["components"].append(type(c).__name__ + ":" + c.get_name())
            for attr in dir(a):
                low = attr.lower()
                if any(k in low for k in ("move", "target", "speed", "dist", "axis", "time", "period", "offset", "start", "end", "loop")):
                    try:
                        val = a.get_editor_property(attr)
                        e["properties"][attr] = str(val)
                    except Exception:
                        pass
            mesh_comp = a.get_component_by_class(unreal.StaticMeshComponent)
            if mesh_comp:
                e["mesh"] = (
                    mesh_comp.get_editor_property("static_mesh").get_path_name()
                    if mesh_comp.get_editor_property("static_mesh")
                    else None
                )
                e["materials"] = component_materials(mesh_comp)
            result["moving_objects"].append(e)
            continue

        if cls == "B_Als_RotatingObject_C":
            e = {
                "name": a.get_name(),
                "location": v3(a.get_actor_location()),
                "rotation": rot_of(a),
                "properties": {},
            }
            for attr in dir(a):
                low = attr.lower()
                if any(k in low for k in ("rot", "speed", "rate", "axis", "period")):
                    try:
                        val = a.get_editor_property(attr)
                        e["properties"][attr] = str(val)
                    except Exception:
                        pass
            result["rotating_objects"].append(e)
            continue

        if cls in ("DirectionalLight", "DirectionalLightMobile"):
            result["lights"].append(dump_light(a))
            continue

        # generic visible mesh components: collect materials
        for comp in a.get_components_by_class(unreal.StaticMeshComponent):
            sm = comp.get_editor_property("static_mesh")
            mats = component_materials(comp)
            key = (a.get_name(), comp.get_name())
            result.setdefault("object_materials", {})[
                a.get_class().get_name() + "/" + a.get_name()
            ] = mats
            for mi in mats:
                if mi and mi.get("path"):
                    p = mi["path"]
                    if p not in result["materials_seen"]:
                        result["materials_seen"][p] = mi
                    for tn in (mi.get("textures") or {}).values():
                        if tn and tn not in result["textures_seen"]:
                            tex = unreal.load_object(None, tn)
                            dim = None
                            if tex:
                                dim = [tex.get_size_x(), tex.get_size_y()]
                            result["textures_seen"][tn] = {"dims": dim}

    skel = unreal.load_object(None, SKEL_MESH_PATH + ".SKM_Als")
    if skel:
        slots = []
        for i in range(skel.get_num_material_slots()):
            m = skel.get_material(i)
            slots.append(material_info(m))
        result["character"] = {
            "path": SKEL_MESH_PATH,
            "material_slots": slots,
        }

    pa_path = None
    ar = unreal.AssetRegistryHelpers.get_asset_registry()
    assets = ar.get_assets_by_class(unreal.TopLevelAssetPath("/Script/Engine", "PhysicsAsset"), True)
    for adata in assets:
        if "PA_Als" in str(adata.asset_name):
            pa_path = str(adata.package_name) + "." + str(adata.asset_name)
            break
    result["physics_asset"]["registry_path"] = pa_path
    if pa_path:
        obj_path = pa_path.split(".", 1)[0]
        pa = unreal.load_object(None, obj_path)
        if pa:
            info = {"loaded": type(pa).__name__, "bodies": [], "constraints_count": None}
            bodies_probe = try_call(pa, ["skeleton_bodies"])
            info["skeleton_bodies_probe"] = bodies_probe[0]
            bodies = bodies_probe[1]
            if bodies:
                for bs in bodies:
                    be = {}
                    st, bone = try_call(bs, ["get_bone_name"])
                    be["bone"] = str(bone) if st == "ok" else f"{st}:{bone}"
                    st2, pt = try_call(bs, ["get_physics_type"])
                    be["physics_type"] = str(pt) if st2 == "ok" else f"{st2}"
                    st3, flag = try_call(bs, ["get_collision_trace_flag"])
                    be["trace_flag"] = str(flag) if st3 == "ok" else None
                    info["bodies"].append(be)
            cons = try_call(pa, ["constraint_profiles"])
            info["constraints_probe"] = cons[0]
            result["physics_asset"] = info

    out_file = os.path.join(OUT, "scene_extra.json")
    with open(out_file, "w", encoding="utf-8") as f:
        json.dump(result, f, ensure_ascii=False, indent=1)
    log("wrote %s" % out_file)
    log(
        "counts: landscape=%d text=%d moving=%d rotating=%d lights=%d mats=%d tex=%d"
        % (
            len(result["landscape"]),
            len(result["text_render"]),
            len(result["moving_objects"]),
            len(result["rotating_objects"]),
            len(result["lights"]),
            len(result["materials_seen"]),
            len(result["textures_seen"]),
        )
    )


def sys_exit(msg):
    log("FATAL " + msg)
    raise SystemExit(1)


main()
