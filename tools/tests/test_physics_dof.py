import copy
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "import"))
from gen_ragdoll import target_locked_axes

source = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
body = source["physics_asset"]["bodies"][0]
keys = ["bLockYRotation", "bLockZRotation", "bLockXRotation",
        "bLockYTranslation", "bLockZTranslation", "bLockXTranslation"]
for mode in ("Default", "SixDOF", "None"):
    for bits in range(64):
        for overall in (False, True):
            fixture = copy.deepcopy(body)
            values = {item["name"]: item for item in fixture["default_instance_properties"]}
            values["DOFMode"]["exported_text"] = mode
            for index, key in enumerate(keys):
                values[key]["exported_text"] = str(bool(bits & (1 << index)))
            for key in ("bLockTranslation", "bLockRotation"):
                values[key]["exported_text"] = str(overall)
            assert target_locked_axes(fixture, 0) == (0 if mode == "None" else bits)
for mode in ("YZPlane", "XZPlane", "XYPlane", "CustomPlane"):
    fixture = copy.deepcopy(body)
    next(item for item in fixture["default_instance_properties"] if item["name"] == "DOFMode")["exported_text"] = mode
    try:
        target_locked_axes(fixture, 0)
    except ValueError:
        pass
    else:
        raise AssertionError(f"unsupported DOF mode accepted: {mode}")
for project_mode in (1, 2, 3):
    fixture = copy.deepcopy(body)
    next(item for item in fixture["default_instance_properties"] if item["name"] == "DOFMode")["exported_text"] = "Default"
    try:
        target_locked_axes(fixture, project_mode)
    except ValueError:
        pass
    else:
        raise AssertionError(f"unsupported project DOF mode accepted: {project_mode}")
print("native SixDOF mode and 384 individual/overall axis fixtures PASS")
