import json
import sys
from pathlib import Path

report = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8-sig"))
probe = report["state_machine_entry_probe"]
assert probe["machine_index"] == 0
assert len(probe["samples"]) == 8
for sample in probe["samples"]:
    pose, desired, limit = sample["name"].split(":")
    pose = pose == "True"
    desired = desired == "True"
    limit = int(limit)
    assert sample["max_transitions"] == limit
    frames = sample["frames"]
    assert len(frames) == 3
    initial = desired if limit >= 2 else pose
    assert frames[0]["state"] == (2 if initial else 1)
    assert frames[0]["weights"] == [0, int(not initial), int(initial), 0, 0, 0]
    assert frames[0]["active_transitions"] == 0
    for frame in frames:
        assert frame['updated'] is True
        assert len(frame['active_entries']) == frame['active_transitions']
        assert frame["updates"][0] == []
        assert all(len(updates) <= 1 for updates in frame["updates"])
    assert frames[1]["state"] == (2 if desired else 1)
    if pose != desired and limit == 1:
        assert frames[1]["active_transitions"] == 1
        assert 0 < frames[1]["weights"][1] < 1
        assert 0 < frames[1]["weights"][2] < 1
    else:
        assert all(frame["active_transitions"] == 0 for frame in frames)
print("All eight native entry-budget cases and 24 update frames match.", flush=True)
