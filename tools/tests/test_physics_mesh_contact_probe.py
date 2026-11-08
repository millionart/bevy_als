import json
import math
import sys
from pathlib import Path

path = Path(sys.argv[1])
report = json.loads(path.with_name(path.name + ".reader.report.json").read_text(encoding="utf-8-sig"))
assert report["success"], "Native reader did not complete successfully"
data = json.loads(path.read_text(encoding="utf-8-sig"))
settings = data["physics_asset"]["constraint_console_variables"]
assert settings["p.Chaos.Collision.ConvexTriMeshMode"] == "1"
for name in ["p.Chaos.Collision.UseCapsuleTriMesh2", "p.Chaos.Collision.EnableMeshManifoldOptimizedLoop", "p.Chaos.Collision.EnableMeshManifoldOptimizedLoopTriMesh", "p.Chaos.MeshContactGenerator.FixContactNormal.FixEnabled"]:
    assert settings[name].lower() in ["1", "true"]
for name in ["p.Chaos.Collision.ConvexTriMeshSortByPhi", "p.Chaos.Collision.ConvexTriMeshSortByDistance"]:
    assert settings[name].lower() in ["0", "false"]
samples = data["physics_contact_probe"]["samples"]
expected = {(mesh, bone, gap) for mesh in ["SM_Als_Barrel", "SM_Als_Rock", "SM_Als_Stairs_01"] for bone in ["head", "foot_l"] for gap in [-0.1, 30]}
assert len(samples) == len(expected)
assert {(s["input"]["mesh_asset_path"].rsplit("/", 1)[1], s["input"]["bone_name"], s["input"]["placement_gap_cm"]) for s in samples} == expected
mesh_orders = {}
for sample in samples:
    source = sample["input"]
    geometry = sample["other_geometry"]
    assert geometry["simulation_enabled"] and geometry["query_enabled"]
    assert geometry["kind"] == ("convex" if "Barrel" in source["mesh_asset_path"] else "trimesh")
    assert (geometry["margin_cm"] > 0) == (geometry["kind"] == "convex")
    assert bool(sample["points"]) == (source["placement_gap_cm"] < 0), source["label"]
    if geometry["kind"] == "trimesh":
        vertices = geometry["trimesh_core_vertices_cm"]
        triangles = geometry["trimesh_core_triangles"]
        order = geometry["trimesh_core_bvh_face_order"]
        assert vertices and triangles
        assert all(len(indices) == 3 and all(0 <= index < len(vertices) for index in indices) for indices in triangles)
        assert sorted(order) == list(range(len(triangles)))
        mesh = source["mesh_asset_path"]
        assert mesh not in mesh_orders or mesh_orders[mesh] == order
        mesh_orders[mesh] = order
    for point in sample["points"]:
        assert point["face_index"] >= 0 if geometry["kind"] == "trimesh" else point["face_index"] == -1
        if geometry["kind"] == "trimesh":
            assert point["face_index"] < len(triangles)
            scale = geometry["trimesh_core_scale"]
            triangle = [[vertices[index][axis] * scale[axis] for axis in "xyz"] for index in triangles[point["face_index"]]]
            edges = [[triangle[index][axis] - triangle[0][axis] for axis in range(3)] for index in [1, 2]]
            face_normal = [edges[0][(axis + 1) % 3] * edges[1][(axis + 2) % 3] - edges[0][(axis + 2) % 3] * edges[1][(axis + 1) % 3] for axis in range(3)]
            plane_error = sum((point["shape_point_1_cm"][axis] - triangle[0][index]) * face_normal[index] for index, axis in enumerate("xyz"))
            assert abs(plane_error) / math.sqrt(sum(value * value for value in face_normal)) < 1e-3
        normal = point["world_normal"]
        assert abs(sum(normal[axis] ** 2 for axis in "xyz") - 1) < 1e-5
        projection = sum((point["world_point_0_cm"][axis] - point["world_point_1_cm"][axis]) * normal[axis] for axis in "xyz")
        assert math.isfinite(point["phi_cm"]) and abs(projection - point["phi_cm"]) < 1e-3
print("physics mesh contact probe PASS")
