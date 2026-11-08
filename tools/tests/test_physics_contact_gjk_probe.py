import json
import math
import sys
from pathlib import Path

path = Path(sys.argv[1])
data = json.loads(path.read_text(encoding="utf-8-sig"))
settings = data["physics_asset"]["constraint_console_variables"]
assert all(name in settings for name in ["p.Chaos.Collision.ConvexZeroMargin", "p.Chaos.Collision.MarginFraction", "p.Chaos.Collision.MarginMax", "p.Chaos.Collision.Manifold.MinFaceSearchDistance"])
assert math.isfinite(float(settings["p.Chaos.Collision.Manifold.PlaneContactNormalEpsilon"]))
assert settings["p.Chaos.Collision.UseGJK2"] == "false"
assert math.isclose(float(settings["p.Chaos.Collision.GJKEpsilon"]), 1e-6)
assert math.isclose(float(settings["p.Chaos.Collision.EPAEpsilon"]), 1e-6)
samples = data["physics_contact_probe"]["samples"]
box_count = 0
for sample in samples:
    simplex = sample["gjk_simplex"]
    assert 0 <= len(simplex) <= 4
    for vertex in simplex:
        assert math.isfinite(vertex["weight"])
        for key in ["shape_point_0_cm", "shape_point_1_cm", "world_delta_cm"]:
            assert all(math.isfinite(vertex[key][axis]) for axis in "xyz")
    if sample["geometry"]["kind"] == "box":
        box_count += 1
        assert simplex
        for vertex in simplex:
            for axis in "xyz":
                lower = sample["geometry"]["minimum_cm"][axis] + sample["collision_margin_0_cm"]
                upper = sample["geometry"]["maximum_cm"][axis] - sample["collision_margin_0_cm"]
                assert min(abs(vertex["shape_point_0_cm"][axis] - edge) for edge in [lower, upper]) < 1e-7
        assert math.isclose(sum(v["weight"] for v in simplex), 1, abs_tol=1e-8)
        delta = [sum(v["weight"] * v["world_delta_cm"][axis] for v in simplex) for axis in "xyz"]
        length = math.sqrt(sum(v*v for v in delta))
        assert length > float(settings["p.Chaos.Collision.GJKEpsilon"])
        normal = [v / length for v in delta]
        expected = sample["points"][0]["world_normal"]
        assert max(abs(normal[i] - expected[axis]) for i, axis in enumerate("xyz")) < 1e-7
        assert math.isclose(length - sample["collision_margin_0_cm"] - sample["collision_margin_1_cm"], min(p["phi_cm"] for p in sample["points"]), abs_tol=1e-5)
assert box_count > 0
print("physics GJK probe PASS: settings, core simplex, separation and normal")
