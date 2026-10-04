import copy
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "import"))
from gen_ragdoll import build_profile

source = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
for enabled in (False, True):
    fixture = copy.deepcopy(source)
    body = next(item for item in fixture["physics_asset"]["bodies"] if item["bone_name"] == "pelvis")
    next(item for item in body["default_instance_properties"] if item["name"] == "bUseCCD")["exported_text"] = str(enabled)
    profile = build_profile(fixture, 3600.0)
    assert next(item for item in profile["bodies"] if item["bone_name"] == "pelvis")["target"]["ccd_enabled"] == enabled
    cvars = source["physics_asset"]["constraint_console_variables"]
    assert profile["target_ccd"]["enable_threshold_bounds_scale"] == float(cvars["p.Chaos.CCD.EnableThresholdBoundsScale"])
    assert profile["target_ccd"]["allowed_depth_bounds_scale"] == float(cvars["p.Chaos.CCD.AllowedDepthBoundsScale"])
    assert profile["target_ccd"]["axis_threshold_mode"] == int(cvars["p.Chaos.CCD.AxisThresholdMode"])
print("native CCD flag and scalar fixtures PASS")
