import argparse
import copy
import hashlib
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "import"))
from gen_ragdoll import build_profile

parser = argparse.ArgumentParser()
parser.add_argument("source")
parser.add_argument("--output")
args = parser.parse_args()
source_path = Path(args.source)
source = json.loads(source_path.read_text(encoding="utf-8"))
for enabled in (False, True):
    fixture = copy.deepcopy(source)
    body = fixture["physics_asset"]["bodies"][0]
    next(item for item in body["default_instance_properties"] if item["name"] == "bInertiaConditioning")["exported_text"] = str(enabled)
    profile = build_profile(fixture, 3600.0)
    assert profile["bodies"][0]["target"]["inertia_conditioning_enabled"] == enabled
settings = profile["target_inertia_conditioning"]
variables = source["physics_asset"]["constraint_console_variables"]
assert settings["distance_m"] == float(variables["p.Chaos.Solver.InertiaConditioning.Distance"]) * 0.01
assert settings["inverse_inertia_tolerance"] == float(variables["p.Chaos.InertiaConditioning.InvInertiaTolerance"]) * 10000.0
assert settings["extent_tolerance_m"] == float(variables["p.Chaos.InertiaConditioning.ExtentTolerance"]) * 0.01

def axes(value, scale=1.0):
    return [value[key] * scale for key in ("x", "z", "y")]

samples = [{"inverse_mass": item["inverse_mass"],
            "inverse_inertia": axes(item["inverse_inertia"], 10000.0),
            "extents_m": axes(item["extents"], 0.01),
            "maximum_distance_m": item["maximum_distance"] * 0.01,
            "maximum_rotation_ratio": item["maximum_rotation_ratio"],
            "maximum_component_ratio": item["maximum_component_ratio"],
            "inverse_scale": axes(item["inverse_scale"])}
           for item in source["physics_asset"]["inertia_conditioning_fixtures"]]
assert len(samples) == 192
if args.output:
    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps({"format": "native_body_inertia_conditioning_v1",
                                 "source_sha256": hashlib.sha256(source_path.read_bytes()).hexdigest(),
                                 "settings": settings, "samples": samples}, indent=2) + "\n", encoding="utf-8")
print("native Body inertia flags, units and 192 UE algorithm samples PASS")
