import json
import sys
from pathlib import Path

path = Path(sys.argv[1])
report = json.loads(path.with_name(path.name + ".reader.report.json").read_text(encoding="utf-8-sig"))
assert report["success"], "Native reader did not complete successfully"
samples = json.loads(path.read_text(encoding="utf-8-sig"))["physics_contact_probe"]["samples"]
meshes = ("SM_Als_Rock", "SM_Als_Stairs_02")
assert {sample["input"]["label"] for sample in samples} == {
    f"{mesh}_{case}" for mesh in meshes for case in ("full", "half", "slice", "outside")
}
for sample in samples:
    geometry = sample["other_geometry"]
    complete = geometry["trimesh_core_bvh_face_order"]
    selected = geometry["trimesh_core_bvh_query_order"]
    rank = {face: index for index, face in enumerate(complete)}
    assert len(selected) == len(set(selected)) and all(face in rank for face in selected)
    assert [rank[face] for face in selected] == sorted(rank[face] for face in selected)
    if sample["input"]["label"].endswith("_full"):
        assert selected == complete
    if sample["input"]["label"].endswith("_outside"):
        assert not selected
print("physics mesh BVH query probe PASS")
