import json
import math
import sys
from pathlib import Path

path = Path(sys.argv[1])
reader = json.loads(path.with_name(path.name + ".reader.report.json").read_text(encoding="utf-8-sig"))
assert reader["success"], "Native reader did not complete successfully"
samples = json.loads(path.read_text(encoding="utf-8-sig"))["physics_contact_probe"]["samples"]
labels = {sample["input"]["label"]: sample for sample in samples}
assert set(labels) == {"crate_box", "stairs_box", "crate_head", "crate_box_rotated_reflected",
                       "stairs_head_rotated_reflected", "crate_box_outside"}
assert labels["crate_box"]["geometry"]["kind"] == "box"
assert labels["crate_head"]["geometry"]["kind"] == "capsule"
assert labels["crate_box"]["mesh_triangle_query"]["triangles"]
assert labels["stairs_box"]["mesh_triangle_query"]["triangles"]
assert not labels["crate_box_outside"]["mesh_triangle_query"]["triangles"]
for name, sample in labels.items():
    query = sample["mesh_triangle_query"]
    cull = sample["input"]["cull_distance_cm"]
    scale = query["mesh_to_convex"]["scale"]
    if "reflected" in name:
        assert scale["x"] * scale["y"] * scale["z"] < 0
    for axis in "xyz":
        assert math.isclose(query["object_min_cm"][axis], query["convex_min_cm"][axis] - cull, abs_tol=1e-9)
        assert math.isclose(query["object_max_cm"][axis], query["convex_max_cm"][axis] + cull, abs_tol=1e-9)
        assert query["mesh_min_cm"][axis] <= query["mesh_max_cm"][axis]
    rank = {face: index for index, face in enumerate(sample["other_geometry"]["trimesh_core_bvh_face_order"])}
    triangles = query["triangles"]
    faces = [triangle["face_index"] for triangle in triangles]
    assert len(faces) == len(set(faces)) and all(face in rank for face in faces)
    assert [rank[face] for face in faces] == sorted(rank[face] for face in faces)
    for triangle in triangles:
        assert len(triangle["vertex_indices"]) == len(triangle["vertices_cm"]) == 3
        for axis in "xyz":
            values = [vertex[axis] for vertex in triangle["vertices_cm"]]
            assert max(values) >= query["object_min_cm"][axis]
            assert min(values) <= query["object_max_cm"][axis]
print("physics mesh triangle query probe PASS")
