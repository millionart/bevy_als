import argparse
import json
import uuid
from collections import defaultdict
from pathlib import Path


class ValidationError(Exception):
    pass


EPSILON = 0.0001


def quoted(value):
    return json.dumps(value, ensure_ascii=False)


def clip_durations(manifest):
    durations = {}
    for source in manifest.get("clips", []):
        if source.get("source_kind") != "AnimSequence":
            continue
        for segment in source.get("runtime_segments", []):
            clip = segment.get("graph_clip")
            if not isinstance(clip, str):
                raise ValidationError("invalid_sequence_graph_clip")
            duration = float(source.get("sequence_length", 0.0))
            existing = durations.get(clip)
            if existing is not None and abs(existing - duration) > EPSILON:
                raise ValidationError("conflicting_graph_clip_duration:" + clip)
            durations[clip] = duration
    return durations


def segment_for_event(source, event):
    start = float(event["start_time"])
    end = float(event["end_time"])
    matches = []
    for segment in source.get("runtime_segments", []):
        segment_start = float(segment["animation_start_time"])
        segment_end = float(segment["animation_end_time"])
        if start + EPSILON >= segment_start and end <= segment_end + EPSILON:
            matches.append(segment)
    if len(matches) != 1:
        raise ValidationError("event_segment_mapping:" + event["event_id"])
    return matches[0]


def map_time(time, segment):
    return float(segment["start_position"]) + (
        float(time) - float(segment["animation_start_time"])
    ) * float(segment["play_rate"])


def tracks(manifest):
    durations = clip_durations(manifest)
    result = defaultdict(lambda: defaultdict(list))
    seen_events = set()
    for source in manifest.get("clips", []):
        for event in source.get("events", []):
            event_id = event.get("event_id")
            if not isinstance(event_id, str) or not event_id:
                raise ValidationError("invalid_event_id")
            if event_id in seen_events:
                raise ValidationError("duplicate_event_id:" + event_id)
            seen_events.add(event_id)
            segment = segment_for_event(source, event)
            clip = segment.get("graph_clip")
            if not isinstance(clip, str) or clip not in durations:
                raise ValidationError("unresolved_event_graph_clip:" + event_id)
            start = map_time(event["start_time"], segment)
            end = map_time(event["end_time"], segment)
            if start < -EPSILON or end < start - EPSILON or end > durations[clip] + EPSILON:
                raise ValidationError("compiled_event_time_out_of_range:" + event_id)
            source_track = str(event.get("track_index", -1))
            track_name = "als_notify:" + source["source_asset"].lower() + ":" + source_track
            result[clip][track_name].append(
                {
                    "id": str(uuid.uuid5(uuid.NAMESPACE_URL, event_id)),
                    "event_id": event_id,
                    "start_time": max(0.0, start),
                    "end_time": min(durations[clip], max(start, end)),
                }
            )
    return result


def render_tracks(track_map):
    lines = ["    event_tracks: {"]
    for name in sorted(track_map):
        lines.append("        " + quoted(name) + ": (")
        lines.append("            name: " + quoted(name) + ",")
        lines.append("            events: [")
        for event in sorted(track_map[name], key=lambda value: (value["start_time"], value["end_time"], value["event_id"])):
            lines.extend(
                [
                    "                (",
                    "                    id: " + quoted(event["id"]) + ",",
                    "                    value: (",
                    "                        event: StringId(" + quoted(event["event_id"]) + "),",
                    "                        start_time: " + repr(event["start_time"]) + ",",
                    "                        end_time: " + repr(event["end_time"]) + ",",
                    "                    ),",
                    "                ),",
                ]
            )
        lines.append("            ],")
        lines.append("        ),")
    lines.append("    },")
    return "\n".join(lines)


def remove_event_tracks(text):
    marker = "\n    event_tracks:"
    start = text.find(marker)
    if start < 0:
        return text
    if text.find(marker, start + len(marker)) >= 0:
        raise ValidationError("multiple_event_tracks_fields")
    brace = text.find("{", start)
    if brace < 0:
        raise ValidationError("event_tracks_missing_map")
    depth = 0
    end = None
    for index in range(brace, len(text)):
        char = text[index]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                end = index + 1
                break
    if end is None:
        raise ValidationError("event_tracks_unclosed_map")
    if end < len(text) and text[end] == ",":
        end += 1
    if end < len(text) and text[end] == "\n":
        end += 1
    return text[:start] + "\n" + text[end:]


def update_graph_clip(path, block, check):
    raw = path.read_bytes()
    newline = "\r\n" if b"\r\n" in raw else "\n"
    text = raw.decode("utf-8").replace("\r\n", "\n")
    text = remove_event_tracks(text)
    end = text.rfind("\n)")
    if end < 0:
        raise ValidationError("graph_clip_missing_final_delimiter:" + path.as_posix())
    updated = text[:end] + "\n" + block + text[end:]
    encoded = updated.replace("\n", newline).encode("utf-8")
    if check:
        if raw != encoded:
            raise ValidationError("graph_clip_mismatch:" + path.as_posix())
    else:
        path.write_bytes(encoded)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", default=Path("assets/ALS/Animations/als_notify_manifest.json"), type=Path)
    parser.add_argument("--assets-root", default=Path("assets"), type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    manifest_data = json.loads(args.manifest.read_text(encoding="utf-8"))
    compiled = tracks(manifest_data)
    for clip, track_map in sorted(compiled.items()):
        path = args.assets_root / clip
        if not path.is_file():
            raise ValidationError("missing_graph_clip:" + clip)
        update_graph_clip(path, render_tracks(track_map), args.check)
    track_count = sum(len(track_map) for track_map in compiled.values())
    event_count = sum(len(events) for track_map in compiled.values() for events in track_map.values())
    print(json.dumps({"graph_clips": len(compiled), "tracks": track_count, "events": event_count}, sort_keys=True))


if __name__ == "__main__":
    main()
