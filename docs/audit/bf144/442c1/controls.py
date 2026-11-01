import json
from pathlib import Path
from check import validate

r = Path(__file__).resolve().parent
data = json.loads((r / "runtime-ablated/parameters.json").read_text())
try:
    validate(data)
except AssertionError:
    pass
else:
    raise AssertionError("Removed lifecycle controls were not detected")
assert data["natural_cold"] == []
assert all(v["player_view_yaw"] is not None for v in data["rows"])
result = dict(gate="PASS_REMOVING_COLD_SAMPLING_AND_CAMERA_INVALIDATION_LOSES_SIGNALS", removed_cold_samples=0, removed_controlled_fallback_rows=0)
(r / "controls-check.json").write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps(result), flush=True)
