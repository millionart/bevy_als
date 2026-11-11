import gzip
import json
import math
import sys
from pathlib import Path


def subtract(a, b):
    return tuple(left - right for left, right in zip(a, b))


def dot(a, b):
    return sum(left * right for left, right in zip(a, b))


def cross(a, b):
    ax, ay, az = a
    bx, by, bz = b
    return ay * bz - az * by, az * bx - ax * bz, ax * by - ay * bx


def vector(point):
    return tuple(point[axis] for axis in "xyz")


directory = Path(__file__).resolve().parent
root = directory.parents[3]
sys.path.insert(0, str(root / "tools/import"))
from import_playground_ray_culling import compile_mesh


mesh_root = root / "assets/ALSExtras/Environment/Meshes"
audit = json.loads((directory / "geometry.audit.json").read_text())
installed = json.loads((mesh_root / "playground_mesh_ray_culling.runtime.json").read_text())
assert len(audit) == len(installed) == 12
assert installed == {row["mesh"]: row["cull_back_faces"] for row in audit}
assert all(installed.values()) and sum(row["matching_oriented_triangles"] for row in audit) == 10133
with gzip.open(directory / "Barrel.native.json.gz", "rt") as stream:
    source = json.load(stream)
assert compile_mesh(source, mesh_root) == ("SM_Als_Barrel", True, 644)
setup_flag, = [prop for obj in source["asset_objects"] for prop in obj["properties"]
               if prop["name"] == "bDoubleSidedGeometry"]
setup_flag["exported_text"] = "True"
assert compile_mesh(source, mesh_root) == ("SM_Als_Barrel", False, 644)
source["static_mesh_collision"]["flip_normals"] = not source["static_mesh_collision"]["flip_normals"]
try:
    compile_mesh(source, mesh_root)
except ValueError as error:
    assert "winding differs" in str(error)
else:
    raise AssertionError("Reversing the source winding must fail the import")

native = json.loads((directory / "barrel-ray.native.json").read_text())
rows = []
for sample in native["geometry_query_probe"]["samples"]:
    report = sample["mesh_raycast"]
    shape, = [shape for shape in report["shapes"] if shape["triangle_mesh"] and shape["complex"]]
    assert shape["culls_back_face"] and not report["double_sided_geometry"]
    ray = shape["ray_visits"]
    assert ray["agrees_with_core"]
    direction = vector(ray["direction"])
    start = vector(ray["start_cm"])
    candidates = []
    for visit in ray["visits"]:
        a, b, c = map(vector, visit["vertices_cm"])
        ab, ac = subtract(b, a), subtract(c, a)
        normal = cross(ab, ac)
        assert visit["culled"] == (dot(direction, normal) > 0.0)
        inverse_direction = tuple(-value for value in direction)
        denominator = dot(inverse_direction, normal)
        if abs(denominator) < 1e-8:
            continue
        offset = subtract(start, a)
        time = dot(offset, normal) / denominator
        v = cross(inverse_direction, offset)
        u, w = dot(ac, v) / denominator, -dot(ab, v) / denominator
        if 0.0 <= time <= ray["length_cm"] and u >= -1e-8 and w >= -1e-8 and u + w <= 1.0 + 1e-8:
            candidates.append((time / ray["length_scale"], visit["culled"]))
    selected = min((time for time, culled in candidates if not culled), default=None)
    unculled = min((time for time, _ in candidates), default=None)
    assert report["hit"] == (selected is not None)
    if selected is not None:
        assert math.isclose(report["distance_cm"], selected, abs_tol=1e-4)
    rows.append({"case": sample["input"]["name"], "source_distance_cm": selected,
                 "distance_with_culling_removed_cm": unculled})
assert [row["case"] for row in rows if row["source_distance_cm"] is None and row["distance_with_culling_removed_cm"] is not None] == [
    "inside_unit", "inside_scaled",
]
print(json.dumps({"cases": rows, "mesh_count": len(audit), "matching_triangles": 10133}, indent=2))
