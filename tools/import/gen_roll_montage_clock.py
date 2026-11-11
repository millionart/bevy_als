import argparse
import json
import re
from pathlib import Path


def branching_points(document):
    properties = {entry["name"]: entry["exported_text"] for entry in document["asset_properties"]}
    field = properties["BranchingPointMarkers"]
    markers = re.findall(r"\(NotifyIndex=(\d+),TriggerTime=([-+0-9.eE]+)(?:,NotifyEventType=(Begin|End))?\)", field)
    if not markers or len(markers) != field.count("NotifyIndex="):
        raise ValueError("Unsupported native montage branching point records")

    events = {}
    for case in document["notify_clock_probe"]["cases"]:
        for event in case["events"]:
            times = event["trigger_time"], event["end_trigger_time"]
            if events.setdefault(event["event_index"], times) != times:
                raise ValueError("Native notify clock samples disagree")

    exact = []
    for index, printed, event_type in markers:
        time = events[int(index)][int(event_type == "End")]
        if abs(time - float(printed)) > 5.0e-7:
            raise ValueError("Native branching point does not match its reflected marker")
        exact.append(time)
    return sorted(set(exact))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("native", type=Path)
    parser.add_argument("--output", type=Path,
                        default=Path(__file__).resolve().parents[2] / "src/als/settings/roll_montage_clock.rs")
    args = parser.parse_args()
    document = json.loads(args.native.read_text(encoding="utf-8-sig"))
    if not document["asset_path"].endswith("/AM_Als_Roll.AM_Als_Roll"):
        raise ValueError("Expected the source Roll montage")
    times = branching_points(document)
    args.output.write_text("[" + ", ".join(repr(time) for time in times) + "]\n", encoding="utf-8")
    print(f"Generated {len(times)} native Roll branching points")


if __name__ == "__main__":
    main()
