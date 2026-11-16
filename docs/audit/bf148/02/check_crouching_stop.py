import gzip
import json
from pathlib import Path
import sys

sys.path.insert(0, "tools")
from validate_animation_visual_evidence import motion_window_issues


path = Path(sys.argv[1])
raw = path.read_bytes()
report = json.loads(gzip.decompress(raw) if path.suffix == ".gz" else raw)
labels = {
    "crouch_stop_right",
    "crouch_quick_stop_default",
    "crouch_quick_stop_left",
    "crouch_quick_stop_right",
}
covered = {
    row["name"]
    for row in report["manual_integration"]["results"]
    if row["covered"]
}
assert labels <= covered and not report["graph_errors"]
windows = [row for row in report["motion_windows"] if row["label"] in labels]
scoped = dict(report, motion_windows=windows, required_motion_windows=sorted(labels))
issues = motion_window_issues(scoped, [], report["pose_samples"], False)
assert not issues, issues
print(json.dumps({"case": report["case"], "windows": sorted(labels), "issues": issues}))
