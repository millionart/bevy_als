import json
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT / "tools/import"))
from generate_als_notify_manifest import manifest
from ue_animation_timing import f32, native_notify_times


def read(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def precision_changes(old, new, changes, path=()):
    if isinstance(old, dict):
        assert old.keys() == new.keys(), path
        for key in old:
            precision_changes(old[key], new[key], changes, (*path, key))
    elif isinstance(old, list):
        assert len(old) == len(new), path
        for previous, current in zip(old, new):
            precision_changes(previous, current, changes, path)
    elif old != new:
        assert type(old) in (int, float) and type(new) in (int, float), path
        if f32(old) != f32(new):
            changes[".".join(path)] += 1


def compare(previous, current):
    evidence = Path(__file__).resolve().parent
    precise = read(evidence / "notify-source-audit.json")
    assert manifest(precise, ROOT / "assets") == current
    rounded = manifest(read(ROOT / "docs/bf103-notify-source-audit.json"), ROOT / "assets")
    clips = {clip["source_asset"]:clip for clip in current["clips"]}
    rounded = {clip["source_asset"]:clip for clip in rounded["clips"]}
    changes = Counter()
    precision_changes(previous, current, changes)
    native_event_count = rounded_event_failures = 0
    for path in sorted(evidence.glob("*.native.json")):
        document = read(path)
        name = document["asset_path"]
        events = native_notify_times(document)
        assert set(events) == set(range(len(clips[name]["events"])))
        for i, times in events.items():
            clip = clips[name]["events"][i]
            ablated = rounded[name]["events"][i]
            assert (clip["start_time"], clip["end_time"]) == times, (name, i)
            rounded_event_failures += (ablated["start_time"], ablated["end_time"]) != times
            native_event_count += 1
    assert native_event_count == 22
    assert rounded_event_failures > 0
    return {"clips":len(clips), "events":sum(len(c["events"]) for c in clips.values()),
            "changed_single_precision_fields":dict(sorted(changes.items())),
            "native_events_compared":native_event_count, "rounded_source_event_failures":rounded_event_failures}


if __name__ == "__main__":
    previous, current = map(Path, sys.argv[1:])
    print(json.dumps(compare(read(previous), read(current)),indent=2))
