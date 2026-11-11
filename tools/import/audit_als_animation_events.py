import argparse
import json
import re
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT))

from uasset_base import UAsset
from uasset_reader import PropReader
from ue_animation_timing import f32


SEQUENCE_PROPERTIES = {
    "SequenceLength",
    "Notifies",
    "AnimNotifyTracks",
    "AuthoredSyncMarkers",
    "SlotAnimTracks",
    "CompositeSections",
}
ANIMATION_CLASSES = {"AnimSequence", "AnimMontage", "AnimComposite"}
MANIFEST_ROW = re.compile(
    r"^\| [^|]+ \| `(?P<source>/ALS/[^`]+)` \| glTF \| `(?P<target>assets/(?:ALS|ALSCamera|ALSExtras)/[^`]+\.gltf)` \|$"
)


def scalar(value, fallback=None):
    if isinstance(value, (str, int, float, bool)) or value is None:
        return value
    return fallback


def numeric(value, fallback=0.0):
    return float(value) if isinstance(value, (int, float)) else fallback


def boolean(value, fallback=False):
    return bool(value) if isinstance(value, bool) else fallback


def enum_value(value):
    if isinstance(value, dict):
        return str(value.get("value", value.get("enum_name", "None")))
    return str(value) if value is not None else "None"


def nested(value):
    if isinstance(value, dict):
        tagged = value.get("__tagged__")
        return tagged if isinstance(tagged, dict) else value
    return {}


def export_path(asset, index):
    parts = []
    current = index + 1
    while current > 0:
        export = asset.exports[current - 1]
        parts.append(export.object_name)
        current = export.outer_index
    return "/".join(reversed(parts))


def export_class(asset, index):
    export = asset.exports[index]
    if export.class_index < 0:
        imported = asset.imports[-export.class_index - 1]
        return imported.object_name if imported.class_name == "Class" else imported.class_name
    return asset.class_of_export(index)


def export_index(asset):
    return {export_path(asset, index): index for index in range(len(asset.exports))}


def detail_for_reference(asset, reader, indexes, reference):
    if not isinstance(reference, str) or not reference:
        return None
    index = indexes.get(reference)
    if index is None:
        return {"object": reference, "class": None, "properties": None}
    properties, ok, error = reader.read_export(index)
    return {
        "object": reference,
        "class": export_class(asset, index),
        "properties": properties if ok else None,
        "properties_error": error,
    }


def manifest_targets(manifest):
    targets = {}
    for line in manifest.read_text(encoding="utf-8").splitlines():
        match = MANIFEST_ROW.match(line)
        if match is None:
            continue
        source_object = match.group("source")
        targets[source_object] = match.group("target")
    return targets


def source_for_file(content_root, path, asset, index, targets):
    relative = path.relative_to(content_root).with_suffix("").as_posix()
    package = "/ALS/" + relative
    source_object = package + "." + asset.exports[index].object_name
    return {
        "source_object": source_object,
        "source_package": package,
        "source_relative": relative,
        "target_gltf": targets.get(source_object),
        "asset_class": export_class(asset, index),
    }


def event_rows(asset, reader, sequence_index, source):
    properties, ok, error = reader.read_export(sequence_index, SEQUENCE_PROPERTIES)
    result = {
        **source,
        "sequence_length": numeric(properties.get("SequenceLength")),
        "read_ok": ok,
        "read_error": error,
        "notify_tracks": [],
        "notifies": [],
        "markers": [],
        "montage_segments": [],
    }
    if not ok:
        return result
    tracks = properties.get("AnimNotifyTracks", [])
    if isinstance(tracks, list):
        for index, track in enumerate(tracks):
            values = nested(track)
            result["notify_tracks"].append(
                {
                    "index": index,
                    "name": str(values.get("TrackName", "None")),
                }
            )
    indexes = export_index(asset)
    notifies = properties.get("Notifies", [])
    if isinstance(notifies, list):
        for event in notifies:
            values = nested(event)
            start = numeric(values.get("LinkValue"))
            duration = numeric(values.get("Duration"))
            end_link = nested(values.get("EndLink"))
            end = numeric(end_link.get("LinkValue"), f32(start + duration))
            state_ref = scalar(values.get("NotifyStateClass"))
            notify_ref = scalar(values.get("Notify"))
            state = detail_for_reference(asset, reader, indexes, state_ref)
            notify = detail_for_reference(asset, reader, indexes, notify_ref)
            track_index = int(values.get("TrackIndex", -1)) if isinstance(values.get("TrackIndex"), int) else -1
            track_name = next(
                (track["name"] for track in result["notify_tracks"] if track["index"] == track_index),
                "None",
            )
            result["notifies"].append(
                {
                    "id": str(values.get("Guid", "None")),
                    "kind": "state" if state_ref else "point",
                    "name": str(values.get("NotifyName", "None")),
                    "start_time": start,
                    "end_time": end if state_ref else start,
                    "duration": duration,
                    "track_index": track_index,
                    "track_name": track_name,
                    "linked_sequence": scalar(values.get("LinkedSequence")),
                    "linked_montage": scalar(values.get("LinkedMontage")),
                    "trigger_time_offset": numeric(values.get("TriggerTimeOffset")),
                    "end_trigger_time_offset": numeric(values.get("EndTriggerTimeOffset")),
                    "weight_threshold": numeric(values.get("TriggerWeightThreshold")),
                    "filter_type": enum_value(values.get("NotifyFilterType")),
                    "filter_lod": int(values.get("NotifyFilterLOD", 0)),
                    "montage_tick_type": enum_value(values.get("MontageTickType")),
                    "trigger_chance": numeric(values.get("NotifyTriggerChance"), 1.0),
                    "trigger_on_follower": boolean(values.get("bTriggerOnFollower")),
                    "trigger_on_dedicated_server": boolean(values.get("bTriggerOnDedicatedServer"), True),
                    "branching_point": boolean(values.get("bConvertedFromBranchingPoint")),
                    "notify": notify,
                    "notify_state": state,
                }
            )
    markers = properties.get("AuthoredSyncMarkers", [])
    if isinstance(markers, list):
        for marker in markers:
            values = nested(marker)
            result["markers"].append(
                {
                    "id": str(values.get("Guid", "None")),
                    "name": str(values.get("MarkerName", "None")),
                    "time": numeric(values.get("Time")),
                    "track_index": int(values.get("TrackIndex", -1)) if isinstance(values.get("TrackIndex"), int) else -1,
                }
            )
    slot_tracks = properties.get("SlotAnimTracks", [])
    if isinstance(slot_tracks, list):
        for slot_track in slot_tracks:
            track_values = nested(slot_track)
            slot_name = str(track_values.get("SlotName", "None"))
            animation_track = nested(track_values.get("AnimTrack"))
            for segment in animation_track.get("AnimSegments", []):
                values = nested(segment)
                result["montage_segments"].append(
                    {
                        "slot": slot_name,
                        "animation": scalar(values.get("AnimReference")),
                        "start_position": numeric(values.get("StartPos")),
                        "animation_start_time": numeric(values.get("AnimStartTime")),
                        "animation_end_time": numeric(values.get("AnimEndTime")),
                        "play_rate": numeric(values.get("AnimPlayRate"), 1.0),
                        "loop_count": int(values.get("LoopingCount", 1)) if isinstance(values.get("LoopingCount"), int) else 1,
                    }
                )
    return result


def audit(content_root, manifest):
    rows = []
    failures = []
    targets = manifest_targets(manifest)
    seen_targets = set()
    for path in sorted(content_root.rglob("*.uasset"), key=lambda path: path.as_posix().casefold()):
        try:
            asset = UAsset(str(path))
        except Exception as error:
            failures.append({"source_object": "/ALS/" + path.relative_to(content_root).as_posix(), "error": type(error).__name__})
            continue
        reader = PropReader(asset, precise_float_properties=True)
        for index in range(len(asset.exports)):
            source = source_for_file(content_root, path, asset, index, targets)
            if source["target_gltf"]:
                seen_targets.add(source["source_object"])
            if source["asset_class"] not in ANIMATION_CLASSES:
                continue
            row = event_rows(asset, reader, index, source)
            rows.append(row)
    for source_object in sorted(set(targets) - seen_targets):
        failures.append({"source_object": source_object, "error": "missing_exported_animation"})
    return rows, failures


def source_class(event):
    detail = event["notify_state"] if event["kind"] == "state" else event["notify"]
    if isinstance(detail, dict):
        value = detail.get("class")
        if isinstance(value, str) and value:
            return value
    return event["name"]


def summary(rows, failures):
    events = [event for row in rows for event in row["notifies"]]
    markers = [marker for row in rows for marker in row["markers"]]
    segments = [segment for row in rows for segment in row["montage_segments"]]
    classes = Counter(source_class(event) for event in events)
    kinds = Counter(event["kind"] for event in events)
    valid_events = all(
        event["start_time"] >= 0.0
        and event["end_time"] >= event["start_time"]
        and event["end_time"] <= row["sequence_length"] + 0.0001
        and event["id"] != "None"
        for row in rows
        for event in row["notifies"]
    )
    valid_markers = all(
        marker["time"] >= 0.0 and marker["time"] <= row["sequence_length"] + 0.0001
        for row in rows
        for marker in row["markers"]
    )
    valid_references = all(
        (
            event["kind"] == "point"
            and (
                event["notify"] is None
                or isinstance(event["notify"].get("class"), str)
            )
        )
        or (
            event["kind"] == "state"
            and isinstance(event["notify_state"], dict)
            and isinstance(event["notify_state"].get("class"), str)
        )
        for row in rows
        for event in row["notifies"]
    ) and all(
        segment["animation"]
        and segment["animation_end_time"] >= segment["animation_start_time"]
        for row in rows
        for segment in row["montage_segments"]
    )
    return {
        "animation_count": len(rows),
        "exported_animation_count": sum(row["target_gltf"] is not None for row in rows),
        "animation_classes": dict(sorted(Counter(row["asset_class"] for row in rows).items())),
        "failure_count": len(failures),
        "event_count": len(events),
        "point_event_count": kinds["point"],
        "state_event_count": kinds["state"],
        "marker_count": len(markers),
        "montage_segment_count": len(segments),
        "event_classes": dict(sorted(classes.items())),
        "valid_events": valid_events,
        "valid_markers": valid_markers,
        "valid_references": valid_references,
    }


def markdown(rows, failures, result):
    lines = [
        "# BF103 ALS Animation Event Source Audit",
        "",
        "This report is generated from local ALS animation uassets listed in the export manifest.",
        "",
        "## Result",
        "",
        f"- Animations audited: {result['animation_count']}",
        f"- Exported animations audited: {result['exported_animation_count']}",
        f"- Asset failures: {result['failure_count']}",
        f"- Point notifies: {result['point_event_count']}",
        f"- Notify states: {result['state_event_count']}",
        f"- Authored sync markers: {result['marker_count']}",
        f"- Montage segments: {result['montage_segment_count']}",
        f"- Event times valid: {result['valid_events']}",
        f"- Marker times valid: {result['valid_markers']}",
        f"- References valid: {result['valid_references']}",
        "",
        "## Event Classes",
        "",
        "| Class | Count |",
        "| --- | ---: |",
    ]
    for name, count in result["event_classes"].items():
        lines.append(f"| {name} | {count} |")
    lines.extend(
        [
            "",
            "## Animation Asset Classes",
            "",
            "| Class | Count |",
            "| --- | ---: |",
        ]
    )
    for name, count in result["animation_classes"].items():
        lines.append(f"| {name} | {count} |")
    lines.extend(
        [
            "",
            "## Animation Events",
            "",
            "| Animation | Target glTF | Kind | Name | Class | Start | End | Weight | Track |",
            "| --- | --- | --- | --- | --- | ---: | ---: | ---: | --- |",
        ]
    )
    for row in rows:
        for event in row["notifies"]:
            lines.append(
                "| {animation} | {target} | {kind} | {name} | {class_name} | {start:.6f} | {end:.6f} | {weight:.6f} | {track} |".format(
                    animation=row["source_object"],
                    target=row["target_gltf"] or "None",
                    kind=event["kind"],
                    name=event["name"],
                    class_name=source_class(event),
                    start=event["start_time"],
                    end=event["end_time"],
                    weight=event["weight_threshold"],
                    track=event["track_name"],
                )
            )
    lines.extend(
        [
            "",
            "## Authored Sync Markers",
            "",
            "| Animation | Target glTF | Marker | Time | Track |",
            "| --- | --- | --- | ---: | ---: |",
        ]
    )
    for row in rows:
        for marker in row["markers"]:
            lines.append(
                f"| {row['source_object']} | {row['target_gltf'] or 'None'} | {marker['name']} | {marker['time']:.6f} | {marker['track_index']} |"
            )
    lines.extend(
        [
            "",
            "## Montage Segments",
            "",
            "| Montage | Slot | Animation | Start position | Animation start | Animation end | Rate | Loops |",
            "| --- | --- | --- | ---: | ---: | ---: | ---: | ---: |",
        ]
    )
    for row in rows:
        for segment in row["montage_segments"]:
            lines.append(
                "| {montage} | {slot} | {animation} | {start:.6f} | {animation_start:.6f} | {animation_end:.6f} | {rate:.6f} | {loops} |".format(
                    montage=row["source_object"],
                    slot=segment["slot"],
                    animation=segment["animation"] or "None",
                    start=segment["start_position"],
                    animation_start=segment["animation_start_time"],
                    animation_end=segment["animation_end_time"],
                    rate=segment["play_rate"],
                    loops=segment["loop_count"],
                )
            )
    if failures:
        lines.extend(
            [
                "",
                "## Failures",
                "",
                "| Source object | Error |",
                "| --- | --- |",
            ]
        )
        for failure in failures:
            lines.append(f"| {failure['source_object']} | {failure['error']} |")
    lines.append("")
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--content-root", required=True, type=Path)
    parser.add_argument("--manifest", default=Path("assets/docs/export-manifest.md"), type=Path)
    parser.add_argument("--json", default=Path("docs/audit/bf144/d121a/notify-source-audit.json"), type=Path)
    parser.add_argument("--report", default=Path("target/notify-source-audit.md"), type=Path)
    args = parser.parse_args()
    rows, failures = audit(args.content_root, args.manifest)
    result = summary(rows, failures)
    document = {"summary": result, "failures": failures, "animations": rows}
    args.json.parent.mkdir(parents=True, exist_ok=True)
    args.json.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n", encoding="utf-8", newline="\n")
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(markdown(rows, failures, result), encoding="utf-8", newline="\n")
    print(json.dumps(result, sort_keys=True))
    return 0 if not failures and result["valid_events"] and result["valid_markers"] and result["valid_references"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
