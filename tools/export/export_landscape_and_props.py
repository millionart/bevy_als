import json
import math
import os

import unreal

OUT_DIR = os.path.join(
    os.environ.get("BEVY_ALS_ROOT", os.getcwd()), "tools", "tmp", "export", "playground"
)
LEVEL_PATH = "/ALS/ALSExtras/Levels/L_Als_Playground"

GRID_SIZE = 128
SAMPLE_EXTENT = 5000.0


def log(msg):
    unreal.log_warning("LANDEXPORT %s" % msg)


def v3(v):
    return {"x": round(v.x, 2), "y": round(v.y, 2), "z": round(v.z, 2)}


def main():
    log("loading map")
    unreal.EditorLoadingAndSavingUtils.load_map(LEVEL_PATH)

    actor_sub = unreal.get_editor_subsystem(unreal.EditorActorSubsystem)
    actors = actor_sub.get_all_level_actors()
    log("total actors: %d" % len(actors))

    landscape_proxies = []
    text_actors = []
    moving_objects = []
    dir_lights = []

    for a in actors:
        cls = a.get_class().get_name()
        if cls in ("LandscapeProxy", "LandscapeStreamingProxy"):
            landscape_proxies.append(a)
        elif cls == "TextRenderActor":
            comp = a.get_component_by_class(unreal.TextRenderComponent)
            if comp:
                try:
                    col = comp.get_editor_property("text_render_color")
                except Exception:
                    col = unreal.Color(1.0, 1.0, 1.0, 1.0)
                text_actors.append({
                    "name": a.get_name(),
                    "text": str(comp.get_editor_property("text")),
                    "location": v3(a.get_actor_location()),
                    "rotation_yaw": a.get_actor_rotation().yaw,
                    "world_size": comp.get_editor_property("world_size"),
                    "color": [round(col.r, 3), round(col.g, 3), round(col.b, 3)],
                })
        elif cls == "B_Als_MovingObject_C":
            mesh_comp = a.get_component_by_class(unreal.StaticMeshComponent)
            entry = {
                "name": a.get_name(),
                "location": v3(a.get_actor_location()),
                "properties": {},
            }
            for attr in dir(a):
                low = attr.lower()
                if any(k in low for k in ("move", "target", "speed", "dist", "axis", "time", "period", "offset", "start", "end", "loop", "height")):
                    try:
                        val = a.get_editor_property(attr)
                        if val is not None and not callable(val):
                            entry["properties"][attr] = str(val)
                    except Exception:
                        pass
            if mesh_comp:
                sm = mesh_comp.get_editor_property("static_mesh")
                entry["mesh"] = sm.get_path_name() if sm else None
            moving_objects.append(entry)
        elif cls in ("DirectionalLight",):
            comp = a.get_component_by_class(unreal.DirectionalLightComponent)
            rot = a.get_actor_rotation()
            d = {
                "rotation": {"pitch": round(rot.pitch, 2), "yaw": round(rot.yaw, 2), "roll": round(rot.roll, 2)},
                "intensity": None,
                "color": None,
                "temperature": None,
                "use_temperature": False,
            }
            if comp:
                for prop, key in [("intensity", "intensity"), ("temperature", "temperature"), ("b_use_temperature", "use_temperature")]:
                    try:
                        d[key] = comp.get_editor_property(prop)
                    except Exception:
                        pass
                try:
                    lc = comp.get_editor_property("light_color")
                    d["color"] = [round(lc.r, 3), round(lc.g, 3), round(lc.b, 3)]
                except Exception:
                    pass
            dir_lights.append(d)

    log("found: landscape=%d text=%d moving=%d lights=%d" % (
        len(landscape_proxies), len(text_actors), len(moving_objects), len(dir_lights)))

    landscape_data = []
    for lp in landscape_proxies:
        comps = lp.get_components_by_class(unreal.LandscapeHeightfieldCollisionComponent)
        comps_render = lp.get_components_by_class(unreal.LandscapeComponent)
        proxy_info = {
            "name": lp.get_name(),
            "location": v3(lp.get_actor_location()),
            "num_collision_comps": len(comps),
            "num_render_comps": len(comps_render),
            "component_details": [],
        }
        for c in comps:
            b = c.bounds
            det = {
                "bounds_origin": v3(b.origin),
                "bounds_extent": v3(b.box_extent),
            }
            for attr in ("component_screen_size", "collision_quads"):
                if hasattr(c, attr.replace("_", "_")):
                    try:
                        det[attr] = getattr(c, attr)()
                    except Exception:
                        pass
            for m in dir(c):
                if "quad" in m.lower() or "size" in m.lower() or "resolution" in m.lower():
                    try:
                        val = getattr(c, m)
                        if not callable(val):
                            det[m] = str(val)
                    except Exception:
                        pass
            proxy_info["component_details"].append(det)
        landscape_data.append(proxy_info)

    world = None
    for a in actors:
        pass
    world_ctx = unreal.get_editor_subsystem(unreal.UnrealEditorSubsystem)
    if world_ctx:
        world = world_ctx.get_game_world()

    heights = []
    if landscape_proxies and world:
        log("sampling terrain heights on %dx%d grid..." % (GRID_SIZE, GRID_SIZE))
        step = SAMPLE_EXTENT * 2 / GRID_SIZE
        for zi in range(GRID_SIZE):
            row = []
            for xi in range(GRID_SIZE):
                x = -SAMPLE_EXTENT + xi * step
                y = -SAMPLE_EXTENT + zi * step
                start = unreal.Vector(x, y, 5000.0)
                end = unreal.Vector(x, y, -1000.0)
                hit = unreal.SystemLibrary.line_trace_single(
                    world, start, end,
                    unreal.TraceTypeQuery.WorldStatic, False, [], False)
                if hit:
                    row.append(round(hit.z_pos, 3))
                else:
                    row.append(0.0)
            heights.append(row)
        log("terrain sampling done")

    out = {
        "landscape_proxies": landscape_data,
        "terrain_heights_cm": {
            "grid_size": GRID_SIZE,
            "extent_cm": SAMPLE_EXTENT * 2,
            "origin_x_cm": -SAMPLE_EXTENT,
            "origin_y_cm": -SAMPLE_EXTENT,
            "step_cm": round(step, 2) if landscape_proxies else 0,
            "heights": heights,
        },
        "text_render_actors": text_actors,
        "moving_objects": moving_objects,
        "directional_lights": dir_lights,
    }

    os.makedirs(OUT_DIR, exist_ok=True)
    out_path = os.path.join(OUT_DIR, "scene_full.json")
    with open(out_path, "w", encoding="utf-8") as f:
        json.dump(out, f, ensure_ascii=False)
    log("wrote %s (%d bytes)" % (out_path, os.path.getsize(out_path)))
    log("done")


main()
