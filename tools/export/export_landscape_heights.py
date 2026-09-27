import json
import math
import os

import unreal

OUT_DIR = os.path.join(
    os.environ.get("BEVY_ALS_ROOT", os.getcwd()), "tools", "tmp", "export", "playground"
)
LEVEL_PATH = "/ALS/ALSExtras/Levels/L_Als_Playground"

GRID = 128
HALF_EXTENT_CM = 6000.0


def log(msg):
    unreal.log_warning("HMAP %s" % msg)


def main():
    log("loading %s" % LEVEL_PATH)
    unreal.EditorLoadingAndSavingUtils.load_map(LEVEL_PATH)

    actor_sub = unreal.get_editor_subsystem(unreal.EditorActorSubsystem)
    all_actors = actor_sub.get_all_level_actors()
    log("actors loaded: %d" % len(all_actors))

    landscape_actors = []
    for a in all_actors:
        cn = a.get_class().get_name()
        if "landscape" in cn.lower() or "Landscape" in cn:
            landscape_actors.append((cn, a.get_name()))
    log("landscape actors found: %s" % str(landscape_actors))

    world = None
    for a in all_actors:
        if hasattr(a, "get_world"):
            try:
                w = a.get_world()
                if w:
                    world = w
                    break
            except Exception:
                pass

    if world is None:
        editor_sub = unreal.get_editor_subsystem(unreal.UnrealEditorSubsystem)
        if editor_sub:
            try:
                world = editor_sub.get_editor_world()
            except Exception:
                pass
    if world is None:
        for a in unreal.EditorLevelLibrary.get_all_level_actors():
            try:
                w = a.get_world()
                if w:
                    world = w
                    break
            except Exception:
                pass

    log("world resolved: %s" % (type(world).__name__ if world else "None"))

    log("sampling %dx%d grid over +-%.0fcm..." % (GRID, GRID, HALF_EXTENT_CM))
    step = 2.0 * HALF_EXTENT_CM / GRID
    heights = []
    hits = 0
    misses = 0

    for zi in range(GRID):
        row = []
        y_cm = -HALF_EXTENT_CM + zi * step + step * 0.5
        for xi in range(GRID):
            x_cm = -HALF_EXTENT_CM + xi * step + step * 0.5
            start = unreal.Vector(x_cm, y_cm, 20000.0)
            end = unreal.Vector(x_cm, y_cm, -20000.0)
            try:
                hit = unreal.SystemLibrary.line_trace_single(
                    world,
                    start,
                    end,
                    unreal.TraceTypeQuery.WorldStatic,
                    False,
                    [],
                    False,
                )
                if hit:
                    row.append(round(hit.z_pos, 2))
                    hits += 1
                else:
                    row.append(0.0)
                    misses += 1
            except Exception:
                row.append(0.0)
                misses += 1
        heights.append(row)
        if zi % 16 == 0:
            log("row %d/%d done" % (zi, GRID))

    hmin = min(min(r) for r in heights)
    hmax = max(max(r) for r in heights)
    log("heights: min=%.2f max=%.2f hits=%d misses=%d" % (hmin, hmax, hits, misses))

    text_render = []
    moving_objects = []
    dir_lights = []

    for a in all_actors:
        cls = a.get_class().get_name()
        loc = a.get_actor_location()
        rot = a.get_actor_rotation()

        if cls == "TextRenderActor":
            comp = a.get_component_by_class(unreal.TextRenderComponent)
            e = {
                "name": a.get_name(),
                "location": {"x": round(loc.x, 1), "y": round(loc.y, 1), "z": round(loc.z, 1)},
                "rotation_yaw": round(rot.yaw, 1),
            }
            if comp:
                try:
                    e["text"] = str(comp.get_editor_property("text"))
                except Exception:
                    e["text"] = ""
                try:
                    col = comp.get_editor_property("text_render_color")
                    e["color"] = [round(col.r, 3), round(col.g, 3), round(col.b, 3)]
                except Exception:
                    e["color"] = [1.0, 1.0, 1.0]
                try:
                    e["world_size"] = comp.get_editor_property("world_size")
                except Exception:
                    e["world_size"] = 30.0
            text_render.append(e)

        elif cls == "B_Als_MovingObject_C":
            mesh_comp = a.get_component_by_class(unreal.StaticMeshComponent)
            entry = {
                "name": a.get_name(),
                "location": {"x": round(loc.x, 1), "y": round(loc.y, 1), "z": round(loc.z, 1)},
                "properties": {},
            }
            for attr in dir(a):
                low = attr.lower()
                if any(k in low for k in ("move", "speed", "dist", "axis", "period")):
                    try:
                        val = getattr(a, attr)
                        if not callable(val):
                            entry["properties"][attr] = str(val)
                    except Exception:
                        pass
            if mesh_comp:
                sm = mesh_comp.get_editor_property("static_mesh")
                entry["mesh"] = sm.get_path_name() if sm else None
            moving_objects.append(entry)

        elif cls == "DirectionalLight":
            comp = a.get_component_by_class(unreal.DirectionalLightComponent)
            d = {"rotation": {"pitch": round(rot.pitch, 2), "yaw": round(rot.yaw, 2)}}
            for prop, key in [
                ("intensity", "intensity"),
                ("temperature", "temperature"),
                ("b_use_temperature", "use_temperature"),
            ]:
                try:
                    d[key] = comp.get_editor_property(prop)
                except Exception:
                    pass
            try:
                lc = comp.get_editor_property("light_color")
                d["color"] = [round(lc.r / 255, 4), round(lc.g / 255, 4), round(lc.b / 255, 4)]
            except Exception:
                d["color"] = [1.0, 1.0, 1.0]
            dir_lights.append(d)

    out = {
        "terrain": {
            "grid": GRID,
            "half_extent_cm": HALF_EXTENT_CM,
            "step_cm": round(step, 3),
            "origin_x_cm": -HALF_EXTENT_CM,
            "origin_y_cm": -HALF_EXTENT_CM,
            "heights_cm": heights,
            "min_cm": hmin,
            "max_cm": hmax,
        },
        "text_render_actors": text_render,
        "moving_objects": moving_objects,
        "directional_lights": dir_lights,
        "landscape_classes_found": landscape_actors,
    }

    os.makedirs(OUT_DIR, exist_ok=True)
    path = os.path.join(OUT_DIR, "landscape_heights.json")
    with open(path, "w", encoding="utf-8") as f:
        json.dump(out, f, ensure_ascii=False)
    log("wrote %s (%d bytes)" % (path, os.path.getsize(path)))
    log("done")


main()
