import copy
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "import"))
from gen_ragdoll import build_profile

source = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
for cached in (False, True):
    fixture = copy.deepcopy(source)
    constraint = next(item for item in fixture["physics_asset"]["constraints"] if item["joint_name"] == "thigh_l")
    profile = next(item for item in constraint["instance_properties"] if item["name"] == "ProfileInstance")
    flag = next(item for item in profile["fields"] if item["name"] == "bUseLinearJointSolver")
    flag["exported_text"] = str(cached)
    result = build_profile(fixture, 3600.0)
    thigh = next(item for item in result["constraints"] if item["joint_name"] == "thigh_l")
    assert thigh["source_profile"]["linear_joint_solver"] == cached
    assert thigh["target"]["use_linear_solver"] == cached
print("native joint solver selector fixtures PASS")
