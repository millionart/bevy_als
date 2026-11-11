import gzip
import json
from pathlib import Path


directory = Path(__file__).resolve().parent
with gzip.open(directory / "roll.native.json.gz") as stream:
    source = json.load(stream)
notify = next(item for item in source["asset_objects"]
              if item["class_path"].endswith("AlsAnimNotifyState_SetRootMotionScale"))
properties = {item["name"]: item for item in notify["properties"]}
assert properties["NotifyStateBehaviorFlags"]["exported_text"] == "0"
scale = properties["TranslationScale"]["numeric_value"]
rows = source["notify_dispatch_probe"]["montage_identity_rows"]
playbacks = [(11,), (11, 22), (22,), (), (33,)]
assert len(rows) == len(playbacks)
assert [row["queued_count"] for row in rows] == [1, 1, 1, 0, 1]
assert [row["active_state_count"] for row in rows] == [1, 1, 1, 0, 1]
assert rows[0]["notify_instance_id"] == rows[1]["notify_instance_id"] == rows[2]["notify_instance_id"]
assert rows[4]["notify_instance_id"] != rows[0]["notify_instance_id"]


def source_phases():
    previous = {}
    for row in rows:
        old = previous.get("notify_instance_id")
        current = row.get("notify_instance_id")
        events = []
        if old is not None and old != current:
            events.append((previous["montage_instance_id"], "end"))
        if current is not None:
            instance = row["montage_instance_id"]
            if old != current:
                events.append((instance, "begin"))
            events.append((instance, "active"))
        yield events
        previous = row


def target_phases(merge_begin=True, merge_end=True, deduplicate=True):
    previous = ()
    for current in playbacks:
        events = [(instance, "end") for instance in previous if instance not in current]
        for instance in current:
            if instance not in previous:
                events.append((instance, "begin"))
            events.append((instance, "active"))
        selected = []
        seen = set()
        for instance, phase in events:
            if (phase == "begin" and merge_begin and previous) or (phase == "end" and merge_end and current):
                continue
            if deduplicate and phase in seen:
                continue
            seen.add(phase)
            selected.append((instance, phase))
        yield selected
        previous = current


def values(phases):
    value = 1.0
    result = []
    for index, events in enumerate(phases):
        if index == 1:
            value = 2.0
        for _, phase in events:
            if phase == "begin":
                value = scale
            elif phase == "end" and abs(value - scale) <= 1e-8:
                value = 1.0
        result.append(value)
    return result


source_trace = list(source_phases())
fixed = list(target_phases())
legacy = list(target_phases(False, False, False))
assert fixed == source_trace
assert values(fixed) == [scale, 2.0, 2.0, 2.0, scale]
assert values(legacy)[1] == scale
ablations = {}
for name in ["merge_begin", "merge_end", "deduplicate"]:
    trace = list(target_phases(**{name: False}))
    ablations[name] = sum(a != b for a, b in zip(trace, source_trace))
    assert ablations[name] > 0
print(json.dumps({"source": rows, "phases": fixed, "scale": values(fixed),
                  "legacy_scale_after_overlap": values(legacy)[1],
                  "ablation_mismatch_frames": ablations}, indent=2))
