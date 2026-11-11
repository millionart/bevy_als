import gzip
import json
import re
from pathlib import Path


directory = Path(__file__).resolve().parent
root = directory.parents[3]
manifest = json.loads((root / "assets/ALS/Animations/als_notify_manifest.json").read_bytes())
montages = {clip["source_asset"]: clip for clip in manifest["clips"] if clip["source_kind"] == "AnimMontage"}


def trace(intervals, markers, steps, *, markers_enabled=True, reconcile_enabled=True, legacy=False):
    active = set()
    result = []
    for previous, current in steps:
        phases = []

        def reconcile(position):
            current_active = {index for index, (start, end) in intervals.items() if start < position <= end}
            phases.extend((index, "end") for index in sorted(active - current_active, reverse=True))
            phases.extend((index, "begin") for index in sorted(current_active - active))
            active.clear()
            active.update(current_active)

        last = None
        old = active.copy()
        if not legacy:
            for time, index, phase in markers:
                if previous < time <= current:
                    if reconcile_enabled:
                        reconcile(time)
                    if markers_enabled:
                        phases.append((index, phase))
                        if phase == "begin":
                            active.add(index)
                        else:
                            active.discard(index)
                    last = time
        if last != current:
            reconcile(current)
        phases.extend((index, "active") for index in sorted(active))
        if legacy:
            for index, (start, end) in intervals.items():
                if previous < start <= end <= current and index not in old and index not in active:
                    phases.extend([(index, "begin"), (index, "end")])
        result.append(phases)
    return result


reports = {}
for path in sorted(directory.glob("*.native.json.gz")):
    with gzip.open(path) as stream:
        source = json.load(stream)
    clip = montages.pop(source["asset_path"])
    properties = {item["name"]: item["exported_text"] for item in source["asset_properties"]}
    native_intervals = {}
    for case in source["notify_clock_probe"]["cases"]:
        for event in case["events"]:
            index = event["event_index"]
            pair = (event["trigger_time"], event["end_trigger_time"])
            assert native_intervals.setdefault(index, pair) == pair
    installed = {index: (event["start_time"], event["end_time"]) for index, event in enumerate(clip["events"])}
    assert native_intervals == installed
    raw_markers = re.findall(
        r"\(NotifyIndex=(\d+),TriggerTime=([\d.-]+)(,NotifyEventType=End)?\)",
        properties["BranchingPointMarkers"],
    )
    native_markers = []
    for index, display_time, end in raw_markers:
        index = int(index)
        time = native_intervals[index][bool(end)]
        assert abs(time - float(display_time)) <= 0.0000005
        native_markers.append((time, index, "end" if end else "begin"))
    markers = sorted((time, index, phase) for index, (start, end) in installed.items()
                     for time, phase in [(start, "begin"), (end, "end")])
    assert markers == native_markers
    length = source["notify_clock_probe"]["duration"]
    cases = {"first_full_step": [(0.0, length)]}
    for index, (start, end) in installed.items():
        previous = max(start, 0.0)
        middle = (previous + end) / 2
        cases[f"state_{index}_end"] = [(previous, middle), (middle, end), (end, end), (end, min(length, end + 0.00004))]
        if start > 0.0:
            cases[f"state_{index}_start"] = [(max(0.0, start - 0.01), start), (start, start), (start, middle)]
        else:
            cases[f"state_{index}_first_exact_end"] = [(0.0, end)]
    rows = []
    for name, steps in cases.items():
        reference = trace(native_intervals, native_markers, steps)
        actual = trace(installed, markers, steps)
        assert actual == reference
        baseline = trace(installed, markers, steps, legacy=True)
        ablated = trace(installed, markers, steps, markers_enabled=False)
        no_reconcile = trace(installed, markers, steps, reconcile_enabled=False)
        rows.append({"case": name, "steps": steps, "expected": actual,
                     "legacy_mismatches": sum(a != b for a, b in zip(baseline, reference)),
                     "without_marker_mismatches": sum(a != b for a, b in zip(ablated, reference)),
                     "without_substep_reconciliation_mismatches": sum(a != b for a, b in zip(no_reconcile, reference))})
    assert rows[0]["expected"][0] == [(index, phase) for index in installed for phase in ["begin", "end"]]
    assert trace(installed, markers, [(length, length)]) != rows[0]["expected"]
    reports[clip["source_asset"]] = {"markers": native_markers, "cases": rows}
assert not montages and len(reports) == 9
counts = {field: sum(case[field] for report in reports.values() for case in report["cases"])
          for field in ["legacy_mismatches", "without_marker_mismatches", "without_substep_reconciliation_mismatches"]}
assert all(counts.values())
print(json.dumps({"montages": len(reports), "counts": counts,
                  "first_step_without_previous_position_mismatches": len(reports),
                  "reports": reports}, indent=2))
