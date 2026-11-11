import json
import math
from pathlib import Path


directory = Path(__file__).resolve().parent
native = json.loads((directory / "overlap.native.json").read_text())
rows = []
for sample in native["geometry_query_probe"]["samples"]:
    request = sample["input"]["box_raycast"]
    hit = sample["box_raycast"]
    assert hit["hit"]
    time = hit["distance_cm"]
    raw_point = [hit["position_cm"][axis] for axis in "xyz"]
    raw_normal = [hit["normal"][axis] for axis in "xyz"]
    start = request["start_cm"]
    direction = request["direction"]
    if time == 0:
        assert raw_normal == [0, 0, 0]
        point = start
        normal = [-value / math.hypot(*direction) for value in direction]
    else:
        point = raw_point
        normal = raw_normal
    offsets = []
    for position, slope_cos in [(point, normal[2]), (raw_point, raw_normal[2])]:
        offsets.append(0.0 if slope_cos < math.cos(math.pi / 4) else
                       position[2] * 0.01 + 0.134656 / slope_cos - 0.134656)
    rows.append({"case": sample["input"]["name"], "distance_cm": time, "impact_point_cm": point,
                 "impact_normal": normal, "foot_offset_m": offsets[0], "raw_normal_offset_m": offsets[1]})
assert [row["case"] for row in rows if row["raw_normal_offset_m"] != row["foot_offset_m"]] == [
    "inside", "surface", "inside_oblique",
]
assert math.isclose(rows[0]["foot_offset_m"], 0.5, abs_tol=1e-12)
assert math.isclose(rows[1]["foot_offset_m"], 0.5, abs_tol=1e-12)
assert math.isclose(rows[2]["foot_offset_m"], 0.4, abs_tol=1e-12)
print(json.dumps({"cases": rows}, indent=2))
