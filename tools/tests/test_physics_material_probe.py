import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "import"))
from gen_ragdoll import target_material

probe = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))["physics_material_probe"]
assert probe["override_path"] != probe["shared_path"]
for key, material, friction, restitution in (
    ("overridden_body", "override_path", 0.8, 0.6),
    ("fallback_body", "shared_path", 0.2, 0.1),
):
    body = probe[key]
    assert body["physical_material_path"] == probe[material]
    values = {entry["name"]: entry["exported_text"] for entry in body["physical_material_properties"]}
    assert abs(float(values["Friction"]) - friction) < 1e-6
    assert abs(float(values["Restitution"]) - restitution) < 1e-6
    target = target_material(body["physical_material_properties"])
    assert abs(target["dynamic_friction"] - friction) < 1e-6
    assert abs(target["static_friction"] - friction) < 1e-6
    assert abs(target["restitution"] - restitution) < 1e-6
print("physics material probe PASS")
