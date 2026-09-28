import copy
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "import"))
from gen_ragdoll import build_profile

source = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
for iterations in (0, 1, 2):
    fixture = copy.deepcopy(source)
    next(item for item in fixture["physics_asset"]["solver_settings_properties"]
        if item["name"] == "VelocityIterations")["exported_text"] = str(iterations)
    profile = build_profile(fixture, 3600.0)
    assert profile["target_solver"]["velocity_iterations"] == iterations
print("joint velocity iteration conversion PASS")
