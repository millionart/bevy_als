import json
import math
import sys
from pathlib import Path

data = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8-sig"))
rows = {row["name"]: row["frames"] for row in data["inertialization_probe"]["samples"]}
assert set(rows) == {"full_half_decay", "small_scale", "short_history", "no_request"}
assert all(len(frames) == 3 for frames in rows.values())
full = rows["full_half_decay"][-1]
assert abs(full["transform"]["translation"]["x"] - 50.0) < 1e-4
assert abs(full["transform"]["rotation"]["y"] - math.sin(math.pi / 8)) < 1e-6
assert abs(full["transform"]["rotation"]["w"] - math.cos(math.pi / 8)) < 1e-6
assert abs(full["curves"]["Curve"] - 0.5) < 1e-6
assert rows["small_scale"][-1]["transform"]["scale"]["x"] == 1.0
assert abs(rows["short_history"][-1]["transform"]["translation"]["x"] - 50.0) < 1e-4
assert rows["no_request"][-1]["transform"]["translation"]["x"] == 0.0
print("Native inertialization pose, curve, threshold, and no-request controls PASS")
