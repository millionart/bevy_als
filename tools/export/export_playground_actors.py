import json
import math
import os

import unreal

OUT = os.path.join(
    os.environ.get("BEVY_ALS_ROOT", os.getcwd()), "tools", "tmp", "export", "playground"
)
LEVEL_PATH = "/ALS/ALSExtras/Levels/L_Als_Playground"
COMPONENT_CLASSES_OF_INTEREST = {
    "StaticMeshActor",
    "B_Als_StaticObject_C",
    "B_Als_MovingObject_C",
    "B_Als_RotatingObject_C",
}


def log(msg):
    unreal.log_warning("PGPROBE %s" % msg)


def euler_deg_to_quat(pitch_deg, yaw_deg, roll_deg):
    p = math.radians(pitch_deg) / 2.0
    y = math.radians(yaw_deg) / 2.0
    r = math.radians(roll_deg) / 2.0
    sp, cp = math.sin(p), math.cos(p)
    sy, cy = math.sin(y), math.cos(y)
    sr, cr = math.sin(r), math.cos(r)
    qx = (sr, 0.0, 0.0, cr)
    qy = (0.0, -sp, 0.0, cp)
    qz = (0.0, 0.0, sy, cy)

    def mul(a, b):
        ax, ay, az, aw = a
        bx, by, bz, bw = b
        return (
            aw * bx + ax * bw + ay * bz - az * by,
            aw * by - ax * bz + ay * bw + az * bx,
            aw * bz + ax * by - ay * bx + az * bw,
            aw * bw - ax * bx - ay * by - az * bz,
        )

    return mul(mul(qz, qy), qx)


def quat_rotate(q, v):
    x, y, z, w = q
    vx, vy, vz = v
    tx = 2.0 * (y * vz - z * vy)
    ty = 2.0 * (z * vx - x * vz)
    tz = 2.0 * (x * vy - y * vx)
    return (
        vx + w * tx + (y * tz - z * ty),
        vy + w * ty + (z * tx - x * tz),
        vz + w * tz + (x * ty - y * tx),
    )


def vec_add(a, b):
    return [a[0] + b[0], a[1] + b[1], a[2] + b[2]]


def component_world_aabb(loc, rot, scale, local_origin, local_extent):
    q = euler_deg_to_quat(rot.pitch, rot.yaw, rot.roll)
    lo = [float("inf")] * 3
    hi = [float("-inf")] * 3
    for sx in (-1.0, 1.0):
        for sy in (-1.0, 1.0):
            for sz in (-1.0, 1.0):
                corner = (
                    local_origin[0] + sx * local_extent[0],
                    local_origin[1] + sy * local_extent[1],
                    local_origin[2] + sz * local_extent[2],
                )
                scaled = [
                    corner[0] * scale[0],
                    corner[1] * scale[1],
                    corner[2] * scale[2],
                ]
                rotated = quat_rotate(q, scaled)
                world = vec_add(loc, rotated)
                for i in range(3):
                    lo[i] = min(lo[i], world[i])
                    hi[i] = max(hi[i], world[i])
    center = [(lo[i] + hi[i]) / 2.0 for i in range(3)]
    extent = [(hi[i] - lo[i]) / 2.0 for i in range(3)]
    return center, extent


def mesh_local_bounds(comp, mesh):
    try:
        b = mesh.get_bounds()
        return ([b.origin.x, b.origin.y, b.origin.z],
                [b.box_extent.x, b.box_extent.y, b.box_extent.z])
    except Exception:
        pass
    try:
        b = comp.get_editor_property("local_bounds")
        return ([b.origin.x, b.origin.y, b.origin.z],
                [b.box_extent.x, b.box_extent.y, b.box_extent.z])
    except Exception:
        pass
    return None, None


def component_world_scale(comp):
    s = [1.0, 1.0, 1.0]
    c = comp
    while c is not None:
        rs = c.get_editor_property("relative_scale3d")
        s = [s[0] * rs.x, s[1] * rs.y, s[2] * rs.z]
        c = c.get_attach_parent()
    return s


def component_entry(comp):
    entry = {
        "name": comp.get_name(),
        "class": comp.get_class().get_name(),
    }
    mesh = None
    try:
        mesh = comp.get_editor_property("static_mesh")
    except Exception:
        mesh = None
    if mesh:
        path = str(mesh.get_path_name())
        entry["mesh"] = path
        entry["engine_basic"] = path.startswith("/Engine/BasicShapes/")
    try:
        loc_v = comp.get_world_location()
        rot = comp.get_world_rotation()
        loc = [float(loc_v.x), float(loc_v.y), float(loc_v.z)]
        scl = component_world_scale(comp)
        entry["location"] = [round(v, 2) for v in loc]
        entry["rotation"] = {
            "pitch": round(rot.pitch, 2),
            "yaw": round(rot.yaw, 2),
            "roll": round(rot.roll, 2),
        }
        entry["scale"] = [round(v, 4) for v in scl]
    except Exception as e:
        entry["world_tf_err"] = str(e)
        return entry
    if mesh:
        lo, ext = mesh_local_bounds(comp, mesh)
        if lo is not None:
            entry["local_box_origin"] = [round(v, 3) for v in lo]
            entry["local_box_extent"] = [round(v, 3) for v in ext]
            try:
                c, e = component_world_aabb(
                    loc, rot, scl,
                    tuple(entry["local_box_origin"]),
                    tuple(entry["local_box_extent"]),
                )
                entry["world_aabb_center"] = [round(v, 2) for v in c]
                entry["world_aabb_extent"] = [round(v, 2) for v in e]
            except Exception as e:
                entry["aabb_err"] = str(e)
    return entry


def actor_entry(a):
    cls = a.get_class().get_name()
    rot = a.get_actor_rotation()
    entry = {
        "class": cls,
        "name": a.get_name(),
        "location": [
            round(a.get_actor_location().x, 2),
            round(a.get_actor_location().y, 2),
            round(a.get_actor_location().z, 2),
        ],
        "rotation_yaw": round(rot.yaw, 2),
        "rotation": {
            "pitch": round(rot.pitch, 2),
            "yaw": round(rot.yaw, 2),
            "roll": round(rot.roll, 2),
        },
        "scale": [
            round(a.get_actor_scale3d().x, 3),
            round(a.get_actor_scale3d().y, 3),
            round(a.get_actor_scale3d().z, 3),
        ],
    }
    try:
        origin, extent = a.get_actor_bounds(False)
        entry["bounds_origin"] = [
            round(origin.x, 2),
            round(origin.y, 2),
            round(origin.z, 2),
        ]
        entry["bounds_extent"] = [
            round(extent.x, 2),
            round(extent.y, 2),
            round(extent.z, 2),
        ]
    except Exception:
        pass
    if isinstance(a, unreal.StaticMeshActor):
        smc = a.static_mesh_component
        mesh = smc.static_mesh if smc else None
        if mesh:
            entry["mesh"] = mesh.get_path_name()
    if cls in COMPONENT_CLASSES_OF_INTEREST:
        comps = []
        try:
            comps = a.get_components_by_class(unreal.StaticMeshComponent)
        except Exception as e:
            entry["components_err"] = str(e)
        rows = []
        for c in comps:
            try:
                rows.append(component_entry(c))
            except Exception as e:
                rows.append({"name": c.get_name(), "err": str(e)})
        entry["components"] = rows
    return entry


def main():
    os.makedirs(OUT, exist_ok=True)

    world = unreal.EditorLoadingAndSavingUtils.load_map(LEVEL_PATH)
    log(
        "load_map -> %s"
        % (world.get_path_name() if hasattr(world, "get_path_name") else world)
    )
    if not hasattr(world, "get_path_name"):
        log("load failed")
        return
    log("world=%s" % world.get_path_name())

    actors = unreal.GameplayStatics.get_all_actors_of_class(world, unreal.Actor)
    log("actor count=%d" % len(actors))

    rows = [actor_entry(a) for a in actors]

    bp_total = 0
    bp_with_real = 0
    bp_with_engine = 0
    bp_without = 0
    for r in rows:
        if r["class"] != "B_Als_StaticObject_C":
            continue
        bp_total += 1
        meshes = [
            c.get("mesh") for c in r.get("components", []) if c.get("mesh")
        ]
        if any(not c.get("engine_basic") for c in r.get("components", []) if c.get("mesh")):
            bp_with_real += 1
        elif meshes:
            bp_with_engine += 1
        else:
            bp_without += 1

    log(
        "STATS static_object total=%d real_mesh=%d engine_basic=%d no_mesh=%d"
        % (bp_total, bp_with_real, bp_with_engine, bp_without)
    )
    sm_total = sum(1 for r in rows if r["class"] == "StaticMeshActor")
    sm_filled = sum(1 for r in rows if r["class"] == "StaticMeshActor" and r.get("mesh"))
    log("STATS staticmeshactor total=%d mesh_filled=%d" % (sm_total, sm_filled))

    out_path = os.path.join(OUT, "playground_actors.json")
    with open(out_path, "w", encoding="utf-8") as f:
        json.dump(rows, f, indent=1)
    log("DONE total=%d -> %s" % (len(rows), out_path))


main()
