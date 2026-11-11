import copy
import json
import sys
from pathlib import Path


directory = Path(__file__).resolve().parent
root = directory.parents[3]
sys.path.insert(0, str(root / "tools/import"))
from import_playground_mesh_ray_data import compile_sample, point, traversal
from import_playground_ray_culling import report_bytes


mesh_root = root / "assets/ALSExtras/Environment/Meshes"
rules = json.loads((mesh_root / "playground_mesh_ray_culling.runtime.json").read_text())
meshes = json.loads((mesh_root / "playground_mesh_ray_data.runtime.json").read_text())
source = json.loads(report_bytes(directory / "mesh-data.native.json.gz"))
assert len(meshes) == len(rules) == 12
for sample in source["geometry_query_probe"]["samples"]:
    name, data = compile_sample(sample, rules)
    assert meshes[name] == data

cart = next(sample for sample in source["geometry_query_probe"]["samples"] if sample["input"]["name"] == "Cart")
cart_shape, = [shape for shape in cart["mesh_raycast"]["shapes"] if shape["triangle_mesh"] and shape["complex"]]
contact = json.loads(report_bytes(directory / "contact-control.native.json.gz"))
other = contact["physics_contact_probe"]["samples"][0]["other_geometry"]
assert other["asset_path"].endswith("SM_Als_Cart.SM_Als_Cart")
for field in ("trimesh_core_vertices_cm", "trimesh_core_triangles", "trimesh_core_bvh_face_order",
              "trimesh_core_bvh_face_bounds_cm", "trimesh_core_bvh_nodes"):
    assert other[field] == cart_shape[field]

changed = copy.deepcopy(cart)
shape, = [shape for shape in changed["mesh_raycast"]["shapes"] if shape["triangle_mesh"] and shape["complex"]]
shape["trimesh_core_bvh_face_order"].reverse()
try:
    compile_sample(changed, rules)
except ValueError as error:
    assert "traversal order differs" in str(error)
else:
    raise AssertionError("Changing the native face order must fail the import")

edges = json.loads((directory / "edge-visits.native.json").read_text())
rows = []
for sample in edges["geometry_query_probe"]["samples"]:
    request = sample["input"]["mesh_raycast"]
    name = request["asset_path"].rsplit(".", 1)[1]
    mesh = meshes[name]
    order = traversal(mesh["nodes"], len(mesh["triangles"]))
    rank = {face: index for index, face in enumerate(order)}
    result = sample["mesh_raycast"]
    shape, = [shape for shape in result["shapes"] if shape["triangle_mesh"] and shape["complex"]]
    ray = shape["ray_visits"]
    assert ray["agrees_with_core"]
    visits = ray["visits"]
    indices = [rank[visit["internal_face"]] for visit in visits]
    assert indices == sorted(set(indices))
    for visit in visits:
        triangle = mesh["triangles"][visit["internal_face"]]
        assert [mesh["vertices_cm"][vertex] for vertex in triangle] == [point(vertex) for vertex in visit["vertices_cm"]]
    hits = [visit for visit in visits if visit["hit"]]
    assert hits and ray["selected_internal_face"] == hits[-1]["internal_face"]
    assert result["face_index"] == hits[-1]["external_face"]
    closest = min(hits, key=lambda visit: visit["distance_cm"])
    rows.append({
        "case": sample["input"]["name"],
        "visits": len(visits),
        "accepted_faces": [visit["external_face"] for visit in hits],
        "native_face": result["face_index"],
        "strict_nearest_face": closest["external_face"],
    })
arch = next(row for row in rows if row["case"] == "arch_shared_edge")
assert arch["accepted_faces"] == [50, 231] and arch["native_face"] == 231 and arch["strict_nearest_face"] == 50
assert any(row["native_face"] != row["strict_nearest_face"] for row in rows if row["case"].startswith("fountain_vertex"))
assert all(row["native_face"] == row["strict_nearest_face"] for row in rows if "interior" in row["case"])
print(json.dumps({"cases": rows, "contact_reader_matches": True, "reversed_order_rejected": True}, indent=2))
