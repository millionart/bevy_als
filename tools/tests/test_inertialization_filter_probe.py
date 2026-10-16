import json
import sys
from pathlib import Path

data = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8-sig"))["inertialization_probe"]
assert data["node_property"] == "AnimGraphNode_Inertialization_1"
assert (data["enabled"], data["ignore_velocity"], data["ignore_deficit"]) == (1, 0, 0)
rows = {row["name"]: row["frames"][-1] for row in data["samples"]}
assert set(rows) == {"filtered_value", "filtered_absent", "filtered_case"}
for name, row in rows.items():
    curves = {key.lower(): value for key, value in row["curves"].items()}
    assert abs(curves["curve"] - 0.5) < 1e-6
    if name == "filtered_absent":
        assert "rotationyawspeed" not in curves
    else:
        assert curves["rotationyawspeed"] == 0.0
print("Selected native inertial curve filter controls PASS")
