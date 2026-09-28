import copy
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "import"))
from gen_ragdoll import build_profile

source = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
for joint_disabled in (False, True):
    for asset_disabled in (False, True):
        fixture = copy.deepcopy(source)
        constraint = next(item for item in fixture["physics_asset"]["constraints"] if item["joint_name"] == "clavicle_l")
        profile_fields = next(item for item in constraint["instance_properties"] if item["name"] == "ProfileInstance")["fields"]
        next(item for item in profile_fields if item["name"] == "bDisableCollision")["exported_text"] = str(joint_disabled)
        pairs = fixture["physics_asset"]["collision_disabled_pairs"]
        pairs[:] = [pair for pair in pairs if sorted((pair["body_index_1"], pair["body_index_2"])) != [3, 10]]
        if asset_disabled:
            pairs.append({"body_index_1": 3, "body_index_2": 10, "stored_value": False, "disabled": True})
        converted = build_profile(fixture, 3600.0)
        joint = next(item for item in converted["constraints"] if item["joint_name"] == "clavicle_l")
        assert joint["source_profile"]["collision_disabled"] == joint_disabled
        assert joint["target"]["collision_disabled"] == joint_disabled
        assert ([3, 10] in converted["collision_disabled_pairs"]) == asset_disabled
print("joint collision source modes PASS")
