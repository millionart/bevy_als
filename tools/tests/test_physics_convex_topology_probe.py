import json
import math
import sys
from pathlib import Path

path = Path(sys.argv[1])
report = json.loads(path.with_name(path.name + ".reader.report.json").read_text(encoding="utf-8-sig"))
assert report["success"], "Native reader did not complete successfully"
samples = json.loads(path.read_text(encoding="utf-8-sig"))["physics_contact_probe"]["samples"]
geometries = {}
scales = set()
for sample in samples:
    geometry = sample["other_geometry"]
    if geometry["kind"] != "convex":
        assert "convex_core_planes" not in geometry
        continue
    vertices = geometry["convex_core_vertices_cm"]
    planes = geometry["convex_core_planes"]
    scale = geometry["convex_core_scale"]
    assert len(vertices) >= 4 and len(planes) >= 4
    for plane in planes:
        indices = plane["vertex_indices"]
        assert len(indices) >= 3 and len(indices) == len(set(indices))
        assert all(isinstance(i, int) and 0 <= i < len(vertices) for i in indices)
        assert abs(sum(plane["normal"][axis] ** 2 for axis in "xyz") - 1) < 1e-5
        assert all(math.isfinite(plane["origin_cm"][axis]) for axis in "xyz")
    for axis in "xyz":
        values = [vertex[axis] * scale[axis] for vertex in vertices]
        assert all(math.isfinite(value) for value in values)
        assert abs(min(values) - geometry["bounds_min_cm"][axis]) < 1e-3
        assert abs(max(values) - geometry["bounds_max_cm"][axis]) < 1e-3
    key = geometry["asset_path"], geometry["shape_index"]
    core = vertices, planes
    if key in geometries:
        assert geometries[key] == core
    geometries[key] = core
    scales.add(tuple(scale[axis] for axis in "xyz"))
assert len(geometries) == 15
assert (1, 1, 1) in scales and (1.5, 1.5, 1.5) in scales
barrels = [planes for (asset, _), (_, planes) in geometries.items() if "Barrel" in asset]
assert len(barrels) == 2 and all(max(len(plane["vertex_indices"]) for plane in planes) == 8 for planes in barrels)
print("physics convex topology probe PASS: fifteen hulls and scaled core identity")
