import copy
import json
import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "import"))
from gen_ragdoll import build_profile

source = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
for body_mode, shape_mode, expected in (
    ("QueryAndPhysics", "QueryOnly", "QueryOnly"),
    ("PhysicsOnly", "QueryAndProbe", "ProbeOnly"),
    ("QueryOnly", "PhysicsOnly", "NoCollision"),
    ("ProbeOnly", "QueryAndPhysics", "ProbeOnly"),
    ("QueryAndProbe", "QueryAndPhysics", "QueryAndProbe"),
    ("NoCollision", "QueryAndPhysics", "NoCollision"),
):
    fixture = copy.deepcopy(source)
    body = next(x for x in fixture["physics_asset"]["bodies"] if x["bone_name"] == "spine_03")
    next(p for p in body["default_instance_properties"] if p["name"] == "CollisionEnabled")["exported_text"] = body_mode
    next(p for p in body["shapes"][1]["properties"] if p["name"] == "CollisionEnabled")["exported_text"] = shape_mode
    profile = build_profile(fixture, 3600.0)
    target = next(x for x in profile["bodies"] if x["bone_name"] == "spine_03")
    assert target["shapes"][1]["target"]["collision_enabled"] == expected
    assert math.isclose(sum(shape["target"]["mass_kg"] for shape in target["shapes"]), body["calculated_mass"])
    if any(shape["target"]["collision_enabled"] in ("NoCollision", "ProbeOnly", "QueryAndProbe") for shape in target["shapes"]):
        mass = target["target"]["mass_properties"]
        assert math.isclose(mass["mass_kg"], body["calculated_mass"])
        assert all(math.isfinite(mass["inertia_tensor_kg_m2"][i][i]) and mass["inertia_tensor_kg_m2"][i][i] > 0 for i in range(3))
print("shape collision mode fixtures PASS")
